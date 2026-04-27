use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Result};

use crate::config::TemplateConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTemplate {
    pub name: String,
    pub subfolder: Option<String>,
    pub tags: Vec<String>,
    pub frontmatter: toml::Table,
}

pub fn resolve_template(
    templates: &[TemplateConfig],
    template_name: &str,
) -> Result<ResolvedTemplate> {
    let by_name = templates_by_name(templates)?;
    let template = by_name
        .get(template_name)
        .with_context(|| format!("template not configured: {template_name}"))?;

    resolve_from_map(&by_name, template)
}

pub fn default_template(templates: &[TemplateConfig]) -> Option<&TemplateConfig> {
    templates.first()
}

fn resolve_from_map(
    by_name: &HashMap<&str, &TemplateConfig>,
    template: &TemplateConfig,
) -> Result<ResolvedTemplate> {
    let Some(base_name) = template.base_template.as_deref() else {
        return Ok(ResolvedTemplate {
            name: template.name.clone(),
            subfolder: normalized_optional_string(template.subfolder.as_deref()),
            tags: dedupe_tags(&template.tags),
            frontmatter: template.frontmatter.clone(),
        });
    };

    if base_name == template.name {
        bail!("template '{}' cannot inherit from itself", template.name);
    }

    let base = by_name.get(base_name).with_context(|| {
        format!(
            "template '{}' references missing base_template '{}'",
            template.name, base_name
        )
    })?;
    if base.base_template.is_some() {
        bail!(
            "template '{}' uses nested inheritance through '{}'",
            template.name,
            base_name
        );
    }

    let mut resolved = resolve_from_map(by_name, base)?;
    resolved.name = template.name.clone();
    if template.subfolder.is_some() {
        resolved.subfolder = normalized_optional_string(template.subfolder.as_deref());
    }
    resolved.tags = merge_tags(&resolved.tags, &template.tags);
    merge_frontmatter(&mut resolved.frontmatter, &template.frontmatter);
    Ok(resolved)
}

fn templates_by_name(templates: &[TemplateConfig]) -> Result<HashMap<&str, &TemplateConfig>> {
    let mut by_name = HashMap::new();
    for template in templates {
        let name = template.name.trim();
        if name.is_empty() {
            bail!("template name cannot be empty");
        }
        if by_name.insert(name, template).is_some() {
            bail!("duplicate template name '{name}'");
        }
    }
    Ok(by_name)
}

fn normalized_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn merge_frontmatter(target: &mut toml::Table, overrides: &toml::Table) {
    for (key, value) in overrides {
        target.insert(key.clone(), value.clone());
    }
}

fn merge_tags(parent: &[String], child: &[String]) -> Vec<String> {
    let mut merged = parent.to_vec();
    let mut seen = parent
        .iter()
        .map(|tag| tag.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    for tag in child {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_ascii_lowercase()) {
            merged.push(trimmed.to_string());
        }
    }
    merged
}

fn dedupe_tags(tags: &[String]) -> Vec<String> {
    merge_tags(&[], tags)
}

#[cfg(test)]
mod tests {
    use toml::Table;

    use super::{default_template, resolve_template};
    use crate::config::TemplateConfig;

    fn template(name: &str) -> TemplateConfig {
        TemplateConfig {
            name: name.to_string(),
            base_template: None,
            subfolder: None,
            tags: Vec::new(),
            frontmatter: Table::new(),
        }
    }

    #[test]
    fn resolves_standalone_template() {
        let mut template = template("quick-capture");
        template.subfolder = Some("inbox".to_string());
        template.tags = vec!["inbox".to_string(), "Inbox".to_string()];
        template.frontmatter.insert(
            "type".to_string(),
            toml::Value::String("capture".to_string()),
        );

        let resolved = resolve_template(&[template], "quick-capture").unwrap();

        assert_eq!(resolved.name, "quick-capture");
        assert_eq!(resolved.subfolder.as_deref(), Some("inbox"));
        assert_eq!(resolved.tags, vec!["inbox"]);
        assert_eq!(resolved.frontmatter["type"].as_str(), Some("capture"));
    }

    #[test]
    fn inherits_and_overrides_parent_template() {
        let mut parent = template("base");
        parent.subfolder = Some("references".to_string());
        parent.tags = vec!["imported".to_string(), "reference".to_string()];
        parent.frontmatter.insert(
            "status".to_string(),
            toml::Value::String("unprocessed".to_string()),
        );
        parent.frontmatter.insert(
            "source".to_string(),
            toml::Value::String("{{source_path}}".to_string()),
        );

        let mut child = template("paper");
        child.base_template = Some("base".to_string());
        child.subfolder = Some("papers".to_string());
        child.tags = vec!["reference".to_string(), "research".to_string()];
        child.frontmatter.insert(
            "status".to_string(),
            toml::Value::String("to-read".to_string()),
        );
        child
            .frontmatter
            .insert("type".to_string(), toml::Value::String("paper".to_string()));

        let resolved = resolve_template(&[parent, child], "paper").unwrap();

        assert_eq!(resolved.subfolder.as_deref(), Some("papers"));
        assert_eq!(resolved.tags, vec!["imported", "reference", "research"]);
        assert_eq!(resolved.frontmatter["status"].as_str(), Some("to-read"));
        assert_eq!(
            resolved.frontmatter["source"].as_str(),
            Some("{{source_path}}")
        );
        assert_eq!(resolved.frontmatter["type"].as_str(), Some("paper"));
    }

    #[test]
    fn keeps_parent_subfolder_when_child_omits_subfolder() {
        let mut parent = template("base");
        parent.subfolder = Some("references".to_string());
        let mut child = template("paper");
        child.base_template = Some("base".to_string());

        let resolved = resolve_template(&[parent, child], "paper").unwrap();

        assert_eq!(resolved.subfolder.as_deref(), Some("references"));
    }

    #[test]
    fn rejects_missing_template() {
        let err = resolve_template(&[], "missing").unwrap_err();

        assert!(err.to_string().contains("template not configured"));
    }

    #[test]
    fn rejects_duplicate_template_names() {
        let err = resolve_template(&[template("dup"), template("dup")], "dup").unwrap_err();

        assert!(err.to_string().contains("duplicate template name"));
    }

    #[test]
    fn rejects_missing_base_template() {
        let mut child = template("child");
        child.base_template = Some("missing".to_string());

        let err = resolve_template(&[child], "child").unwrap_err();

        assert!(err.to_string().contains("missing base_template"));
    }

    #[test]
    fn rejects_nested_inheritance() {
        let grandparent = template("grandparent");
        let mut parent = template("parent");
        parent.base_template = Some("grandparent".to_string());
        let mut child = template("child");
        child.base_template = Some("parent".to_string());

        let err = resolve_template(&[grandparent, parent, child], "child").unwrap_err();

        assert!(err.to_string().contains("nested inheritance"));
    }

    #[test]
    fn default_template_returns_first_template() {
        let first = template("first");
        let second = template("second");

        assert_eq!(
            default_template(&[first, second]).map(|template| template.name.as_str()),
            Some("first")
        );
    }
}
