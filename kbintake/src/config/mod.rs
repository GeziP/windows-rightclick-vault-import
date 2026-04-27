use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::Target;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app_data_dir: PathBuf,
    pub targets: Vec<Target>,
    pub import: ImportConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routing: Vec<RoutingRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<TemplateConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routing_rules: Vec<RoutingRuleV2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub watch: Vec<WatchConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfig {
    pub max_file_size_mb: u64,
    #[serde(default = "default_inject_frontmatter")]
    pub inject_frontmatter: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingRule {
    pub extensions: Vec<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplateConfig {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subfolder: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "toml::Table::is_empty")]
    pub frontmatter: toml::Table,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingRuleV2 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension: Option<StringList>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name_contains: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size_kb_gt: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size_kb_lt: Option<u64>,
    pub template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default = "default_debounce_secs")]
    pub debounce_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<StringList>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum StringList {
    One(String),
    Many(Vec<String>),
}

impl StringList {
    pub fn values(&self) -> Vec<&str> {
        match self {
            Self::One(value) => vec![value.as_str()],
            Self::Many(values) => values.iter().map(String::as_str).collect(),
        }
    }

    pub fn matches_case_insensitive(&self, candidate: &str) -> bool {
        self.values()
            .iter()
            .any(|value| value.eq_ignore_ascii_case(candidate))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RouteSelection {
    pub target: Target,
    pub matched_rule_template: Option<String>,
}

/// Resolved routing intent for an import operation, combining explicit overrides
/// with automatic routing.
#[derive(Debug, Clone)]
pub struct ImportRoutingIntent {
    pub target: Target,
    pub template_name: Option<String>,
    pub matched_rule_template: Option<String>,
}

impl ConfigValidation {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

impl AppConfig {
    pub fn load_or_init() -> Result<Self> {
        let app_data_dir = default_app_data_dir();
        Self::load_or_init_in(app_data_dir)
    }

    pub fn load_or_init_in(app_data_dir: PathBuf) -> Result<Self> {
        let config_path = app_data_dir.join("config.toml");

        if config_path.exists() {
            let raw = std::fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?;
            let config = toml::from_str(&raw)
                .with_context(|| format!("failed to parse {}", config_path.display()))?;
            return Ok(config);
        }

        std::fs::create_dir_all(&app_data_dir)?;
        let target_root = app_data_dir.join("vault");
        let config = Self {
            app_data_dir,
            targets: vec![Target::new("default", target_root)],
            import: ImportConfig {
                max_file_size_mb: 512,
                inject_frontmatter: true,
                language: None,
            },
            agent: AgentConfig {
                poll_interval_secs: default_poll_interval_secs(),
            },
            routing: Vec::new(),
            templates: Vec::new(),
            routing_rules: Vec::new(),
            watch: Vec::new(),
        };

        config.save()?;
        Ok(config)
    }

    pub fn default_target(&self) -> Result<Target> {
        let target = self
            .targets
            .first()
            .context("no import target configured")?;
        ensure_target_active(target)?;
        Ok(target.clone())
    }

    pub fn target_by_id(&self, target_id: &str) -> Result<Target> {
        let target = self.target_any_by_id(target_id)?;
        ensure_target_active(&target)?;
        Ok(target)
    }

    pub fn target_for_path(&self, path: &std::path::Path) -> Result<Target> {
        if let Some(extension) = path.extension().map(|value| value.to_string_lossy()) {
            let normalized = format!(".{}", extension).to_ascii_lowercase();
            for rule in &self.routing {
                if rule
                    .extensions
                    .iter()
                    .any(|candidate| normalize_extension(candidate) == normalized)
                {
                    return self.target_by_id(&rule.target);
                }
            }
        }
        self.default_target()
    }

    pub fn target_for_path_with_size(
        &self,
        path: &std::path::Path,
        source_size_bytes: u64,
    ) -> Result<Target> {
        Ok(self
            .route_selection_for_path(path, source_size_bytes)?
            .target)
    }

    pub fn route_selection_for_path(
        &self,
        path: &std::path::Path,
        source_size_bytes: u64,
    ) -> Result<RouteSelection> {
        let matched_rule = self.first_matching_routing_rule(path, source_size_bytes);
        let target = if let Some(target) = matched_rule.and_then(|rule| rule.target.as_deref()) {
            self.target_by_id(target)?
        } else {
            self.target_for_path(path)?
        };

        Ok(RouteSelection {
            target,
            matched_rule_template: matched_rule.map(|rule| rule.template.clone()),
        })
    }

    /// Returns the configured UI language, defaulting to `"en"`.
    pub fn language(&self) -> &str {
        self.import.language.as_deref().unwrap_or("en")
    }

    /// Resolve full routing intent for an import, honouring explicit target and template overrides.
    ///
    /// When `explicit_template` is `Some`, routing-rule template selection is bypassed and the
    /// named template is used directly. When `explicit_target` is `Some`, the routing-rule template
    /// is suppressed (the caller explicitly directed the target). Target resolution still follows
    /// the explicit-then-default chain.
    pub fn resolve_import_intent(
        &self,
        path: &std::path::Path,
        source_size_bytes: u64,
        explicit_target: Option<String>,
        explicit_template: Option<String>,
    ) -> Result<ImportRoutingIntent> {
        let is_explicit_target = explicit_target.is_some();

        // Resolve matched rule template first (before any move).
        let routing_rule_template = self
            .first_matching_routing_rule(path, source_size_bytes)
            .filter(|_| !is_explicit_target)
            .map(|rule| rule.template.clone());

        let target = match explicit_target {
            Some(id) => self.target_by_id(&id)?,
            None => {
                let matched_rule = self.first_matching_routing_rule(path, source_size_bytes);
                if let Some(target) = matched_rule.and_then(|rule| rule.target.as_deref()) {
                    self.target_by_id(target)?
                } else {
                    self.target_for_path(path)?
                }
            }
        };

        let template_name = match explicit_template {
            Some(name) => {
                self.templates
                    .iter()
                    .find(|t| t.name == name)
                    .with_context(|| format!("template not found: {name}"))?;
                Some(name)
            }
            None => routing_rule_template.clone(),
        };

        Ok(ImportRoutingIntent {
            target,
            template_name,
            matched_rule_template: routing_rule_template,
        })
    }

    /// Resolve just the template name for a path, honouring an explicit override.
    pub fn resolve_template_name(
        &self,
        path: &std::path::Path,
        source_size_bytes: u64,
        explicit_template: Option<&str>,
    ) -> Option<String> {
        match explicit_template {
            Some(name) => {
                self.templates.iter().find(|t| t.name == name)?;
                Some(name.to_string())
            }
            None => self
                .template_for_path(path, source_size_bytes)
                .map(|t| t.name.clone()),
        }
    }

    pub fn routing_warnings(&self) -> Vec<String> {
        let mut warnings = self
            .routing
            .iter()
            .filter(|rule| self.target_any_by_id(&rule.target).is_err())
            .map(|rule| format!("routing rule references missing target '{}'", rule.target))
            .collect::<Vec<_>>();
        warnings.extend(
            self.routing_rules
                .iter()
                .filter_map(|rule| rule.target.as_deref())
                .filter(|target| self.target_any_by_id(target).is_err())
                .map(|target| format!("routing rule references missing target '{target}'")),
        );
        warnings
    }

    pub fn target_any_by_id(&self, target_id: &str) -> Result<Target> {
        self.targets
            .iter()
            .find(|target| target.target_id == target_id || target.name == target_id)
            .cloned()
            .with_context(|| format!("target not configured: {target_id}"))
    }

    pub fn add_target(&mut self, name: impl Into<String>, root_path: PathBuf) -> Result<Target> {
        let name = validate_target_name(name.into())?;
        if self
            .targets
            .iter()
            .any(|target| target.is_active() && (target.target_id == name || target.name == name))
        {
            bail!("target already configured: {name}");
        }

        let target = Target::new(name, root_path);
        self.targets.push(target.clone());
        Ok(target)
    }

    pub fn set_default_target_by_id(&mut self, target_id: &str) -> Result<Target> {
        let index = self.target_index(target_id)?;
        ensure_target_active(&self.targets[index])?;
        let target = self.targets.remove(index);
        self.targets.insert(0, target.clone());
        Ok(target)
    }

    pub fn rename_target(
        &mut self,
        target_id: impl AsRef<str>,
        new_name: impl Into<String>,
    ) -> Result<Target> {
        let target_id = target_id.as_ref();
        let new_name = validate_target_name(new_name.into())?;
        if self.targets.iter().any(|target| {
            target.is_active()
                && (target.target_id == new_name || target.name == new_name)
                && target.target_id != target_id
                && target.name != target_id
        }) {
            bail!("target already configured: {new_name}");
        }

        let index = self.target_index(target_id)?;
        ensure_target_active(&self.targets[index])?;
        self.targets[index].target_id = new_name.clone();
        self.targets[index].name = new_name;
        Ok(self.targets[index].clone())
    }

    pub fn remove_target(&mut self, target_id: &str) -> Result<Target> {
        let index = self.target_index(target_id)?;
        ensure_target_active(&self.targets[index])?;
        self.targets[index].archive();
        Ok(self.targets[index].clone())
    }

    fn target_index(&self, target_id: &str) -> Result<usize> {
        self.targets
            .iter()
            .position(|target| target.target_id == target_id || target.name == target_id)
            .with_context(|| format!("target not configured: {target_id}"))
    }

    pub fn config_path(&self) -> PathBuf {
        self.app_data_dir.join("config.toml")
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.app_data_dir)?;
        let config_path = self.config_path();
        let encoded = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, encoded)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
        Ok(())
    }

    pub fn set_default_target(
        &mut self,
        name: impl Into<String>,
        root_path: PathBuf,
    ) -> Result<Target> {
        let name = name.into();
        let name = validate_target_name(name)?;

        let target = Target::new(name, root_path);
        if self.targets.is_empty() {
            self.targets.push(target.clone());
        } else {
            self.targets[0] = target.clone();
        }
        Ok(target)
    }

    pub fn validate_semantics(&self) -> ConfigValidation {
        let mut errors = Vec::new();
        let mut warnings = self.routing_warnings();

        if self.targets.is_empty() {
            errors.push("no targets configured".to_string());
        }

        for target in &self.targets {
            if let Some(subfolder) = &target.default_subfolder {
                if subfolder.trim().is_empty() {
                    errors.push(format!(
                        "target '{}' has an empty default_subfolder",
                        target.target_id
                    ));
                }
                if std::path::Path::new(subfolder).is_absolute() {
                    errors.push(format!(
                        "target '{}' default_subfolder must be relative",
                        target.target_id
                    ));
                }
            }
        }

        let mut template_names = std::collections::HashSet::new();
        for template in &self.templates {
            let name = template.name.trim();
            if name.is_empty() {
                errors.push("template name cannot be empty".to_string());
                continue;
            }
            if !template_names.insert(name.to_string()) {
                errors.push(format!("duplicate template name '{name}'"));
            }
        }

        for template in &self.templates {
            if let Some(base_template) = template.base_template.as_deref() {
                if base_template == template.name {
                    errors.push(format!(
                        "template '{}' cannot inherit from itself",
                        template.name
                    ));
                } else if !template_names.contains(base_template) {
                    errors.push(format!(
                        "template '{}' references missing base_template '{}'",
                        template.name, base_template
                    ));
                } else if self
                    .templates
                    .iter()
                    .find(|candidate| candidate.name == base_template)
                    .and_then(|candidate| candidate.base_template.as_deref())
                    .is_some()
                {
                    errors.push(format!(
                        "template '{}' uses nested inheritance through '{}'",
                        template.name, base_template
                    ));
                }
            }
        }

        for rule in &self.routing_rules {
            if !template_names.contains(rule.template.trim()) {
                errors.push(format!(
                    "routing rule references missing template '{}'",
                    rule.template
                ));
            }
            if let Some(extension) = &rule.extension {
                for value in extension.values() {
                    if value.trim().is_empty() {
                        errors.push(format!(
                            "routing rule for template '{}' has an empty extension",
                            rule.template
                        ));
                    }
                }
            }
            if let Some(target) = rule.target.as_deref() {
                if self.target_any_by_id(target).is_err() {
                    errors.push(format!("routing rule references missing target '{target}'"));
                }
            }
            if rule.extension.is_none()
                && rule.source_folder.is_none()
                && rule.file_name_contains.is_none()
                && rule.file_size_kb_gt.is_none()
                && rule.file_size_kb_lt.is_none()
            {
                warnings.push(format!(
                    "routing rule for template '{}' has no match conditions and will match broadly",
                    rule.template
                ));
            }
        }

        ConfigValidation { errors, warnings }
    }

    pub fn template_for_path<'a>(
        &'a self,
        path: &std::path::Path,
        source_size_bytes: u64,
    ) -> Option<&'a TemplateConfig> {
        if let Some(rule) = self.first_matching_routing_rule(path, source_size_bytes) {
            return self
                .templates
                .iter()
                .find(|template| template.name == rule.template);
        }

        self.templates.first()
    }

    fn first_matching_routing_rule<'a>(
        &'a self,
        path: &std::path::Path,
        source_size_bytes: u64,
    ) -> Option<&'a RoutingRuleV2> {
        self.routing_rules
            .iter()
            .find(|rule| routing_rule_matches(rule, path, source_size_bytes))
    }
}

fn routing_rule_matches(
    rule: &RoutingRuleV2,
    path: &std::path::Path,
    source_size_bytes: u64,
) -> bool {
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_ext = path
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let source_path = path.to_string_lossy().to_string();
    let file_size_kb = source_size_bytes.div_ceil(1024);

    if let Some(extension) = &rule.extension {
        if !extension.matches_case_insensitive(&file_ext) {
            return false;
        }
    }
    if let Some(source_folder) = &rule.source_folder {
        let normalized_rule = source_folder.replace('/', "\\").to_ascii_lowercase();
        let normalized_path = source_path.to_ascii_lowercase();
        if !wildcard_match(&normalized_rule, &normalized_path) {
            return false;
        }
    }
    if let Some(file_name_contains) = &rule.file_name_contains {
        if !file_name
            .to_ascii_lowercase()
            .contains(&file_name_contains.to_ascii_lowercase())
        {
            return false;
        }
    }
    if let Some(file_size_kb_gt) = rule.file_size_kb_gt {
        if file_size_kb <= file_size_kb_gt {
            return false;
        }
    }
    if let Some(file_size_kb_lt) = rule.file_size_kb_lt {
        if file_size_kb >= file_size_kb_lt {
            return false;
        }
    }

    true
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    let text_chars = text.chars().collect::<Vec<_>>();
    let (mut p, mut t) = (0usize, 0usize);
    let (mut star_idx, mut match_idx) = (None, 0usize);

    while t < text_chars.len() {
        if p < pattern_chars.len() && (pattern_chars[p] == text_chars[t]) {
            p += 1;
            t += 1;
        } else if p < pattern_chars.len() && pattern_chars[p] == '*' {
            star_idx = Some(p);
            match_idx = t;
            p += 1;
        } else if let Some(star) = star_idx {
            p = star + 1;
            match_idx += 1;
            t = match_idx;
        } else {
            return false;
        }
    }

    while p < pattern_chars.len() && pattern_chars[p] == '*' {
        p += 1;
    }

    p == pattern_chars.len()
}

fn default_inject_frontmatter() -> bool {
    true
}

pub fn default_app_data_dir() -> PathBuf {
    std::env::var_os("KBINTAKE_APP_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("kbintake")
        })
}

fn default_poll_interval_secs() -> u64 {
    5
}

fn default_debounce_secs() -> u64 {
    2
}

fn ensure_target_active(target: &Target) -> Result<()> {
    if !target.is_active() {
        bail!("Target '{}' is archived and cannot be used.", target.name);
    }
    Ok(())
}

fn validate_target_name(name: String) -> Result<String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        bail!("target name cannot be empty");
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        bail!("target name may only contain letters, numbers, '-' and '_'");
    }
    Ok(name)
}

fn normalize_extension(extension: &str) -> String {
    let trimmed = extension.trim().to_ascii_lowercase();
    if trimmed.starts_with('.') {
        trimmed
    } else {
        format!(".{trimmed}")
    }
}

pub fn validate_target_root(root_path: &std::path::Path) -> Result<()> {
    if root_path.exists() && !root_path.is_dir() {
        bail!(
            "target path exists but is not a directory: {}",
            root_path.display()
        );
    }

    std::fs::create_dir_all(root_path)
        .with_context(|| format!("failed to create target directory {}", root_path.display()))?;

    let probe_path = root_path.join(format!(".kbintake-write-test-{}", Uuid::new_v4()));
    std::fs::write(&probe_path, b"ok")
        .with_context(|| format!("failed to write target probe {}", probe_path.display()))?;
    std::fs::remove_file(&probe_path)
        .with_context(|| format!("failed to remove target probe {}", probe_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        validate_target_root, AppConfig, RoutingRule, RoutingRuleV2, StringList, TemplateConfig,
    };

    #[test]
    fn saves_and_reloads_updated_default_target() {
        let temp = tempfile::tempdir().unwrap();
        let app_data_dir = temp.path().join("appdata");
        let target_root = temp.path().join("vaults").join("main");

        let mut config = AppConfig::load_or_init_in(app_data_dir.clone()).unwrap();
        let target = config
            .set_default_target("main", target_root.clone())
            .unwrap();
        config.save().unwrap();

        let reloaded = AppConfig::load_or_init_in(app_data_dir).unwrap();
        assert_eq!(target.name, "main");
        assert_eq!(reloaded.targets[0].name, "main");
        assert_eq!(reloaded.targets[0].root_path, target_root);
    }

    #[test]
    fn rejects_empty_target_name() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();

        let err = config
            .set_default_target(" ", temp.path().join("vault"))
            .unwrap_err();

        assert!(err.to_string().contains("target name cannot be empty"));
    }

    #[test]
    fn looks_up_target_by_id() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .set_default_target("archive", temp.path().join("archive"))
            .unwrap();

        assert_eq!(config.target_by_id("archive").unwrap().name, "archive");
        assert!(config
            .target_by_id("missing")
            .unwrap_err()
            .to_string()
            .contains("target not configured"));
    }

    #[test]
    fn rename_target_updates_id_and_name() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();

        let target = config.rename_target("archive", "notes").unwrap();

        assert_eq!(target.target_id, "notes");
        assert_eq!(target.name, "notes");
        assert!(config.target_by_id("notes").is_ok());
        assert!(config.target_by_id("archive").is_err());
    }

    #[test]
    fn rename_target_rejects_duplicate_name() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();

        let err = config.rename_target("archive", "default").unwrap_err();

        assert!(err.to_string().contains("target already configured"));
    }

    #[test]
    fn rename_target_rejects_invalid_name() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();

        let err = config.rename_target("default", "bad name").unwrap_err();

        assert!(err.to_string().contains("may only contain"));
    }

    #[test]
    fn remove_non_default_target_archives_and_preserves_default() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();

        let removed = config.remove_target("archive").unwrap();

        assert_eq!(removed.target_id, "archive");
        assert_eq!(removed.status, "archived");
        assert_eq!(config.targets.len(), 2);
        assert_eq!(config.targets[0].target_id, "default");
    }

    #[test]
    fn remove_default_target_archives_without_promoting_next_target() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();

        let removed = config.remove_target("default").unwrap();

        assert_eq!(removed.target_id, "default");
        assert_eq!(removed.status, "archived");
        assert_eq!(config.targets.len(), 2);
        assert_eq!(config.targets[0].target_id, "default");
        assert!(config
            .default_target()
            .unwrap_err()
            .to_string()
            .contains("archived"));
    }

    #[test]
    fn remove_target_allows_archiving_last_active_target() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();

        let removed = config.remove_target("default").unwrap();

        assert_eq!(removed.status, "archived");
    }

    #[test]
    fn add_target_rejects_duplicates() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();

        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();
        let err = config
            .add_target("archive", temp.path().join("other"))
            .unwrap_err();

        assert!(err.to_string().contains("target already configured"));
    }

    #[test]
    fn set_default_target_by_id_reorders_targets() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();

        let target = config.set_default_target_by_id("archive").unwrap();

        assert_eq!(target.target_id, "archive");
        assert_eq!(config.targets[0].target_id, "archive");
        assert_eq!(config.targets[1].target_id, "default");
    }

    #[test]
    fn target_for_path_uses_case_insensitive_routing_rule() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();
        config.routing.push(RoutingRule {
            extensions: vec![".PDF".to_string()],
            target: "archive".to_string(),
        });

        let target = config
            .target_for_path(std::path::Path::new("report.pdf"))
            .unwrap();

        assert_eq!(target.target_id, "archive");
    }

    #[test]
    fn target_for_path_falls_back_to_default_without_matching_rule() {
        let temp = tempfile::tempdir().unwrap();
        let config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();

        let target = config
            .target_for_path(std::path::Path::new("note.md"))
            .unwrap();

        assert_eq!(target.target_id, "default");
    }

    #[test]
    fn routing_warnings_report_missing_targets() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config.routing.push(RoutingRule {
            extensions: vec![".pdf".to_string()],
            target: "missing".to_string(),
        });

        let warnings = config.routing_warnings();

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("missing"));
    }

    #[test]
    fn loads_v2_templates_routing_rules_and_default_subfolder() {
        let temp = tempfile::tempdir().unwrap();
        let app_data_dir = temp.path().join("appdata");
        fs::create_dir_all(&app_data_dir).unwrap();
        let vault = temp.path().join("vault");
        let raw = format!(
            r#"
app_data_dir = "{}"

[[targets]]
target_id = "default"
name = "default"
root_path = "{}"
default_subfolder = "inbox"
status = "active"

[import]
max_file_size_mb = 512
inject_frontmatter = true

[agent]
poll_interval_secs = 5

[[templates]]
name = "research-paper"
subfolder = "references"
tags = ["research"]
[templates.frontmatter]
type = "paper"
source = "{{{{source_path}}}}"

[[routing_rules]]
extension = ["pdf", "md"]
template = "research-paper"
target = "default"
"#,
            app_data_dir.display().to_string().replace('\\', "\\\\"),
            vault.display().to_string().replace('\\', "\\\\")
        );
        fs::write(app_data_dir.join("config.toml"), raw).unwrap();

        let config = AppConfig::load_or_init_in(app_data_dir).unwrap();

        assert_eq!(
            config.targets[0].default_subfolder.as_deref(),
            Some("inbox")
        );
        assert_eq!(config.templates[0].name, "research-paper");
        assert_eq!(
            config.templates[0].frontmatter["type"].as_str(),
            Some("paper")
        );
        assert!(config.validate_semantics().is_valid());
        assert_eq!(
            config.routing_rules[0].extension,
            Some(StringList::Many(vec!["pdf".to_string(), "md".to_string()]))
        );
    }

    #[test]
    fn loads_v1_routing_without_v2_sections() {
        let temp = tempfile::tempdir().unwrap();
        let app_data_dir = temp.path().join("appdata");
        fs::create_dir_all(&app_data_dir).unwrap();
        let vault = temp.path().join("vault");
        let archive = temp.path().join("archive");
        let raw = format!(
            r#"
app_data_dir = "{}"

[[targets]]
target_id = "default"
name = "default"
root_path = "{}"
status = "active"

[[targets]]
target_id = "archive"
name = "archive"
root_path = "{}"
status = "active"

[import]
max_file_size_mb = 512
inject_frontmatter = true

[agent]
poll_interval_secs = 5

[[routing]]
extensions = [".pdf"]
target = "archive"
"#,
            app_data_dir.display().to_string().replace('\\', "\\\\"),
            vault.display().to_string().replace('\\', "\\\\"),
            archive.display().to_string().replace('\\', "\\\\")
        );
        fs::write(app_data_dir.join("config.toml"), raw).unwrap();

        let config = AppConfig::load_or_init_in(app_data_dir).unwrap();
        let target = config
            .target_for_path(std::path::Path::new("paper.pdf"))
            .unwrap();

        assert_eq!(target.target_id, "archive");
        assert!(config.templates.is_empty());
        assert!(config.routing_rules.is_empty());
    }

    #[test]
    fn validate_semantics_rejects_missing_template_references() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config.routing_rules.push(RoutingRuleV2 {
            extension: Some(StringList::One("pdf".to_string())),
            source_folder: None,
            file_name_contains: None,
            file_size_kb_gt: None,
            file_size_kb_lt: None,
            template: "missing".to_string(),
            target: Some("default".to_string()),
        });

        let validation = config.validate_semantics();

        assert!(!validation.is_valid());
        assert!(validation
            .errors
            .iter()
            .any(|error| error.contains("missing template")));
    }

    #[test]
    fn template_for_path_prefers_first_matching_routing_rule() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config.templates.push(TemplateConfig {
            name: "default-template".to_string(),
            base_template: None,
            subfolder: None,
            tags: Vec::new(),
            frontmatter: toml::Table::new(),
        });
        config.templates.push(TemplateConfig {
            name: "pdf-template".to_string(),
            base_template: None,
            subfolder: None,
            tags: Vec::new(),
            frontmatter: toml::Table::new(),
        });
        config.templates.push(TemplateConfig {
            name: "meeting-template".to_string(),
            base_template: None,
            subfolder: None,
            tags: Vec::new(),
            frontmatter: toml::Table::new(),
        });
        config.routing_rules.push(RoutingRuleV2 {
            extension: Some(StringList::One("pdf".to_string())),
            source_folder: None,
            file_name_contains: None,
            file_size_kb_gt: None,
            file_size_kb_lt: None,
            template: "pdf-template".to_string(),
            target: None,
        });
        config.routing_rules.push(RoutingRuleV2 {
            extension: None,
            source_folder: None,
            file_name_contains: Some("meeting".to_string()),
            file_size_kb_gt: None,
            file_size_kb_lt: None,
            template: "meeting-template".to_string(),
            target: None,
        });

        let template = config
            .template_for_path(std::path::Path::new("team-meeting.pdf"), 10 * 1024)
            .unwrap();

        assert_eq!(template.name, "pdf-template");
    }

    #[test]
    fn template_for_path_falls_back_to_first_template_without_match() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config.templates.push(TemplateConfig {
            name: "default-template".to_string(),
            base_template: None,
            subfolder: None,
            tags: Vec::new(),
            frontmatter: toml::Table::new(),
        });
        config.templates.push(TemplateConfig {
            name: "large-image-template".to_string(),
            base_template: None,
            subfolder: None,
            tags: Vec::new(),
            frontmatter: toml::Table::new(),
        });
        config.routing_rules.push(RoutingRuleV2 {
            extension: Some(StringList::One("png".to_string())),
            source_folder: None,
            file_name_contains: None,
            file_size_kb_gt: Some(500),
            file_size_kb_lt: None,
            template: "large-image-template".to_string(),
            target: None,
        });

        let template = config
            .template_for_path(std::path::Path::new("tiny.png"), 10 * 1024)
            .unwrap();

        assert_eq!(template.name, "default-template");
    }

    #[test]
    fn target_for_path_with_size_prefers_matching_v2_rule_target() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();
        config.routing_rules.push(RoutingRuleV2 {
            extension: Some(StringList::One("pdf".to_string())),
            source_folder: None,
            file_name_contains: None,
            file_size_kb_gt: None,
            file_size_kb_lt: None,
            template: "missing-template-is-ignored-for-target-selection".to_string(),
            target: Some("archive".to_string()),
        });

        let target = config
            .target_for_path_with_size(std::path::Path::new("paper.pdf"), 3 * 1024)
            .unwrap();

        assert_eq!(target.target_id, "archive");
    }

    #[test]
    fn target_for_path_with_size_falls_back_to_v1_routing_when_v2_rule_has_no_target() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config
            .add_target("archive", temp.path().join("archive"))
            .unwrap();
        config.routing.push(RoutingRule {
            extensions: vec![".pdf".to_string()],
            target: "archive".to_string(),
        });
        config.routing_rules.push(RoutingRuleV2 {
            extension: Some(StringList::One("pdf".to_string())),
            source_folder: None,
            file_name_contains: None,
            file_size_kb_gt: None,
            file_size_kb_lt: None,
            template: "capture".to_string(),
            target: None,
        });

        let target = config
            .target_for_path_with_size(std::path::Path::new("paper.pdf"), 3 * 1024)
            .unwrap();

        assert_eq!(target.target_id, "archive");
    }

    #[test]
    fn route_selection_returns_matched_v2_template_name() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();
        config.routing_rules.push(RoutingRuleV2 {
            extension: Some(StringList::One("pdf".to_string())),
            source_folder: None,
            file_name_contains: None,
            file_size_kb_gt: None,
            file_size_kb_lt: None,
            template: "research-paper".to_string(),
            target: None,
        });

        let selection = config
            .route_selection_for_path(std::path::Path::new("paper.pdf"), 3 * 1024)
            .unwrap();

        assert_eq!(selection.target.target_id, "default");
        assert_eq!(
            selection.matched_rule_template.as_deref(),
            Some("research-paper")
        );
    }

    #[test]
    fn target_name_allows_cli_friendly_characters_only() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_or_init_in(temp.path().join("appdata")).unwrap();

        let err = config
            .add_target("bad target", temp.path().join("bad"))
            .unwrap_err();

        assert!(err.to_string().contains("may only contain"));
    }

    #[test]
    fn validate_target_root_creates_writable_directory() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("new-vault");

        validate_target_root(&target).unwrap();

        assert!(target.is_dir());
        assert!(fs::read_dir(target).unwrap().next().is_none());
    }

    #[test]
    fn validate_target_root_rejects_file_path() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("not-a-dir");
        fs::write(&target, "file").unwrap();

        let err = validate_target_root(&target).unwrap_err();

        assert!(err.to_string().contains("not a directory"));
    }
}
