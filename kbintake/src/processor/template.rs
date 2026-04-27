use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};

use crate::config::TemplateConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTemplate {
    pub name: String,
    pub subfolder: Option<String>,
    pub tags: Vec<String>,
    pub frontmatter: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRenderContext {
    pub source_path: String,
    pub source_name: String,
    pub file_ext: Option<String>,
    pub file_size_bytes: u64,
    pub imported_at: DateTime<Utc>,
    pub sha256: String,
    pub target_name: String,
    pub batch_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedTemplate {
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

pub fn render_template(
    template: &ResolvedTemplate,
    context: &TemplateRenderContext,
) -> RenderedTemplate {
    RenderedTemplate {
        name: template.name.clone(),
        subfolder: template
            .subfolder
            .as_deref()
            .map(|value| render_string(value, context)),
        tags: template
            .tags
            .iter()
            .map(|tag| render_string(tag, context))
            .collect(),
        frontmatter: render_table(&template.frontmatter, context),
    }
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

fn render_table(table: &toml::Table, context: &TemplateRenderContext) -> toml::Table {
    table
        .iter()
        .map(|(key, value)| (key.clone(), render_value(value, context)))
        .collect()
}

fn render_value(value: &toml::Value, context: &TemplateRenderContext) -> toml::Value {
    match value {
        toml::Value::String(inner) => toml::Value::String(render_string(inner, context)),
        toml::Value::Array(items) => toml::Value::Array(
            items
                .iter()
                .map(|item| render_value(item, context))
                .collect(),
        ),
        toml::Value::Table(table) => toml::Value::Table(render_table(table, context)),
        _ => value.clone(),
    }
}

fn render_string(input: &str, context: &TemplateRenderContext) -> String {
    let mut rendered = render_conditionals(input, context);
    for (name, value) in template_variables(context) {
        let needle = format!("{{{{{name}}}}}");
        rendered = rendered.replace(&needle, &value);
    }
    rendered
}

fn render_conditionals(input: &str, context: &TemplateRenderContext) -> String {
    let mut rendered = String::new();
    let mut cursor = 0usize;

    while let Some(relative_start) = input[cursor..].find("{{#if ") {
        let start = cursor + relative_start;
        rendered.push_str(&input[cursor..start]);

        let Some((end, condition, then_branch, else_branch)) = parse_if_block(&input[start..])
        else {
            rendered.push_str(&input[start..]);
            return rendered;
        };

        let selected = if evaluate_condition(condition, context) {
            then_branch
        } else {
            else_branch.unwrap_or("")
        };
        rendered.push_str(&render_conditionals(selected, context));
        cursor = start + end;
    }

    rendered.push_str(&input[cursor..]);
    rendered
}

fn parse_if_block(input: &str) -> Option<(usize, &str, &str, Option<&str>)> {
    let header_end = input.find("}}")?;
    let header = &input["{{#if ".len()..header_end];
    let body_start = header_end + 2;
    let mut cursor = body_start;
    let mut depth = 1usize;
    let mut else_range = None;

    while cursor < input.len() {
        let rest = &input[cursor..];
        let next_if = rest.find("{{#if ");
        let next_else = rest.find("{{#else}}");
        let next_end = rest.find("{{/if}}");

        let (offset, token) = [
            next_if.map(|value| (value, TokenKind::If)),
            next_else.map(|value| (value, TokenKind::Else)),
            next_end.map(|value| (value, TokenKind::End)),
        ]
        .into_iter()
        .flatten()
        .min_by_key(|(value, _)| *value)?;

        cursor += offset;
        match token {
            TokenKind::If => {
                depth += 1;
                cursor += "{{#if ".len();
            }
            TokenKind::Else => {
                if depth == 1 && else_range.is_none() {
                    else_range = Some((cursor, cursor + "{{#else}}".len()));
                }
                cursor += "{{#else}}".len();
            }
            TokenKind::End => {
                depth -= 1;
                if depth == 0 {
                    let then_start = body_start;
                    let end_start = cursor;
                    let then_branch = if let Some((else_start, _)) = else_range {
                        &input[then_start..else_start]
                    } else {
                        &input[then_start..end_start]
                    };
                    let else_branch = else_range.map(|(_, else_end)| &input[else_end..end_start]);
                    let block_end = cursor + "{{/if}}".len();
                    return Some((block_end, header.trim(), then_branch, else_branch));
                }
                cursor += "{{/if}}".len();
            }
        }
    }

    None
}

fn evaluate_condition(expression: &str, context: &TemplateRenderContext) -> bool {
    let Ok(tokens) = tokenize(expression) else {
        return false;
    };
    let mut parser = ConditionParser::new(tokens, context);
    parser
        .parse_expression()
        .is_some_and(|value| if parser.is_at_end() { value } else { false })
}

fn tokenize(expression: &str) -> Result<Vec<ConditionToken>> {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }

        let token = match ch {
            '(' => {
                index += 1;
                ConditionToken::LParen
            }
            ')' => {
                index += 1;
                ConditionToken::RParen
            }
            '&' if chars.get(index + 1) == Some(&'&') => {
                index += 2;
                ConditionToken::And
            }
            '|' if chars.get(index + 1) == Some(&'|') => {
                index += 2;
                ConditionToken::Or
            }
            '=' if chars.get(index + 1) == Some(&'=') => {
                index += 2;
                ConditionToken::Eq
            }
            '!' if chars.get(index + 1) == Some(&'=') => {
                index += 2;
                ConditionToken::Ne
            }
            '>' if chars.get(index + 1) == Some(&'=') => {
                index += 2;
                ConditionToken::Ge
            }
            '<' if chars.get(index + 1) == Some(&'=') => {
                index += 2;
                ConditionToken::Le
            }
            '>' => {
                index += 1;
                ConditionToken::Gt
            }
            '<' => {
                index += 1;
                ConditionToken::Lt
            }
            '"' => {
                index += 1;
                let start = index;
                while index < chars.len() && chars[index] != '"' {
                    index += 1;
                }
                if index >= chars.len() {
                    bail!("unterminated string literal");
                }
                let value = chars[start..index].iter().collect::<String>();
                index += 1;
                ConditionToken::String(value)
            }
            _ if ch.is_ascii_digit() => {
                let start = index;
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                }
                ConditionToken::Number(chars[start..index].iter().collect::<String>())
            }
            _ => {
                let start = index;
                while index < chars.len()
                    && !chars[index].is_whitespace()
                    && !matches!(chars[index], '(' | ')' | '>' | '<' | '=' | '!' | '&' | '|')
                {
                    index += 1;
                }
                let word = chars[start..index].iter().collect::<String>();
                match word.as_str() {
                    "contains" => ConditionToken::Contains,
                    _ => ConditionToken::Identifier(word),
                }
            }
        };
        tokens.push(token);
    }

    Ok(tokens)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    If,
    Else,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConditionToken {
    Identifier(String),
    String(String),
    Number(String),
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    And,
    Or,
    LParen,
    RParen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConditionValue {
    String(String),
    Number(i64),
}

struct ConditionParser<'a> {
    tokens: Vec<ConditionToken>,
    current: usize,
    context: &'a TemplateRenderContext,
}

impl<'a> ConditionParser<'a> {
    fn new(tokens: Vec<ConditionToken>, context: &'a TemplateRenderContext) -> Self {
        Self {
            tokens,
            current: 0,
            context,
        }
    }

    fn parse_expression(&mut self) -> Option<bool> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Option<bool> {
        let mut value = self.parse_and()?;
        while self.matches(|token| matches!(token, ConditionToken::Or)) {
            value = value || self.parse_and()?;
        }
        Some(value)
    }

    fn parse_and(&mut self) -> Option<bool> {
        let mut value = self.parse_primary_bool()?;
        while self.matches(|token| matches!(token, ConditionToken::And)) {
            value = value && self.parse_primary_bool()?;
        }
        Some(value)
    }

    fn parse_primary_bool(&mut self) -> Option<bool> {
        if self.matches(|token| matches!(token, ConditionToken::LParen)) {
            let value = self.parse_expression()?;
            self.consume(|token| matches!(token, ConditionToken::RParen))?;
            return Some(value);
        }

        let left = self.parse_value()?;
        let operator = self.advance()?.clone();
        let right = self.parse_value()?;
        compare_values(&left, &operator, &right)
    }

    fn parse_value(&mut self) -> Option<ConditionValue> {
        match self.advance()?.clone() {
            ConditionToken::Identifier(name) => Some(resolve_identifier(&name, self.context)),
            ConditionToken::String(value) => Some(ConditionValue::String(value)),
            ConditionToken::Number(value) => value.parse::<i64>().ok().map(ConditionValue::Number),
            _ => None,
        }
    }

    fn matches(&mut self, predicate: impl Fn(&ConditionToken) -> bool) -> bool {
        if self.peek().is_some_and(predicate) {
            self.current += 1;
            true
        } else {
            false
        }
    }

    fn consume(&mut self, predicate: impl Fn(&ConditionToken) -> bool) -> Option<()> {
        if self.peek().is_some_and(predicate) {
            self.current += 1;
            Some(())
        } else {
            None
        }
    }

    fn advance(&mut self) -> Option<&ConditionToken> {
        let token = self.tokens.get(self.current)?;
        self.current += 1;
        Some(token)
    }

    fn peek(&self) -> Option<&ConditionToken> {
        self.tokens.get(self.current)
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
    }
}

fn resolve_identifier(name: &str, context: &TemplateRenderContext) -> ConditionValue {
    match name {
        "file_name" => ConditionValue::String(source_stem(&context.source_name)),
        "file_ext" => ConditionValue::String(
            context
                .file_ext
                .clone()
                .unwrap_or_default()
                .to_ascii_lowercase(),
        ),
        "file_size_kb" => ConditionValue::Number(file_size_kb(context.file_size_bytes) as i64),
        "imported_at" => ConditionValue::String(context.imported_at.to_rfc3339()),
        "imported_at_date" => ConditionValue::String(
            context
                .imported_at
                .date_naive()
                .format("%Y-%m-%d")
                .to_string(),
        ),
        "source_path" => ConditionValue::String(context.source_path.clone()),
        "sha256" => ConditionValue::String(context.sha256.clone()),
        "target_name" => ConditionValue::String(context.target_name.clone()),
        "batch_id" => ConditionValue::String(context.batch_id.clone()),
        _ => ConditionValue::String(String::new()),
    }
}

fn compare_values(
    left: &ConditionValue,
    operator: &ConditionToken,
    right: &ConditionValue,
) -> Option<bool> {
    match operator {
        ConditionToken::Eq => Some(equals_values(left, right)),
        ConditionToken::Ne => Some(!equals_values(left, right)),
        ConditionToken::Gt => compare_numbers(left, right).map(|(l, r)| l > r),
        ConditionToken::Ge => compare_numbers(left, right).map(|(l, r)| l >= r),
        ConditionToken::Lt => compare_numbers(left, right).map(|(l, r)| l < r),
        ConditionToken::Le => compare_numbers(left, right).map(|(l, r)| l <= r),
        ConditionToken::Contains => Some(string_value(left).contains(&string_value(right))),
        _ => None,
    }
}

fn equals_values(left: &ConditionValue, right: &ConditionValue) -> bool {
    match (left, right) {
        (ConditionValue::Number(left), ConditionValue::Number(right)) => left == right,
        _ => string_value(left) == string_value(right),
    }
}

fn compare_numbers(left: &ConditionValue, right: &ConditionValue) -> Option<(i64, i64)> {
    match (left, right) {
        (ConditionValue::Number(left), ConditionValue::Number(right)) => Some((*left, *right)),
        _ => None,
    }
}

fn string_value(value: &ConditionValue) -> String {
    match value {
        ConditionValue::String(value) => value.clone(),
        ConditionValue::Number(value) => value.to_string(),
    }
}

fn template_variables(context: &TemplateRenderContext) -> [(&'static str, String); 9] {
    [
        ("file_name", source_stem(&context.source_name)),
        (
            "file_ext",
            context
                .file_ext
                .clone()
                .unwrap_or_default()
                .to_ascii_lowercase(),
        ),
        (
            "file_size_kb",
            file_size_kb(context.file_size_bytes).to_string(),
        ),
        ("imported_at", context.imported_at.to_rfc3339()),
        (
            "imported_at_date",
            context
                .imported_at
                .date_naive()
                .format("%Y-%m-%d")
                .to_string(),
        ),
        ("source_path", context.source_path.clone()),
        ("sha256", context.sha256.clone()),
        ("target_name", context.target_name.clone()),
        ("batch_id", context.batch_id.clone()),
    ]
}

fn source_stem(source_name: &str) -> String {
    std::path::Path::new(source_name)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| source_name.to_string())
}

fn file_size_kb(size_bytes: u64) -> u64 {
    size_bytes.div_ceil(1024)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use toml::Table;

    use super::{
        default_template, render_template, resolve_template, RenderedTemplate,
        TemplateRenderContext,
    };
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

    fn render_context() -> TemplateRenderContext {
        TemplateRenderContext {
            source_path: r"C:\Users\dev\Downloads\attention.pdf".to_string(),
            source_name: "attention.pdf".to_string(),
            file_ext: Some("PDF".to_string()),
            file_size_bytes: 2_560,
            imported_at: Utc.with_ymd_and_hms(2026, 4, 27, 1, 2, 3).unwrap(),
            sha256: "abc123ff".to_string(),
            target_name: "research".to_string(),
            batch_id: "batch-123".to_string(),
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

    #[test]
    fn renders_template_strings_using_builtin_variables() {
        let mut resolved = resolve_template(&[template("capture")], "capture").unwrap();
        resolved.subfolder = Some("papers/{{imported_at_date}}".to_string());
        resolved.tags = vec!["{{file_ext}}".to_string(), "{{target_name}}".to_string()];
        resolved.frontmatter.insert(
            "title".to_string(),
            toml::Value::String("{{file_name}}".to_string()),
        );
        resolved.frontmatter.insert(
            "source".to_string(),
            toml::Value::String("{{source_path}}".to_string()),
        );
        resolved.frontmatter.insert(
            "size_kb".to_string(),
            toml::Value::String("{{file_size_kb}}".to_string()),
        );
        resolved.frontmatter.insert(
            "imported".to_string(),
            toml::Value::String("{{imported_at}}".to_string()),
        );
        resolved.frontmatter.insert(
            "hash".to_string(),
            toml::Value::String("{{sha256}}".to_string()),
        );
        resolved.frontmatter.insert(
            "batch".to_string(),
            toml::Value::String("{{batch_id}}".to_string()),
        );

        let rendered = render_template(&resolved, &render_context());

        assert_eq!(rendered.subfolder.as_deref(), Some("papers/2026-04-27"));
        assert_eq!(rendered.tags, vec!["pdf", "research"]);
        assert_eq!(rendered.frontmatter["title"].as_str(), Some("attention"));
        assert_eq!(
            rendered.frontmatter["source"].as_str(),
            Some(r"C:\Users\dev\Downloads\attention.pdf")
        );
        assert_eq!(rendered.frontmatter["size_kb"].as_str(), Some("3"));
        assert_eq!(
            rendered.frontmatter["imported"].as_str(),
            Some("2026-04-27T01:02:03+00:00")
        );
        assert_eq!(rendered.frontmatter["hash"].as_str(), Some("abc123ff"));
        assert_eq!(rendered.frontmatter["batch"].as_str(), Some("batch-123"));
    }

    #[test]
    fn renders_nested_frontmatter_values() {
        let mut resolved = resolve_template(&[template("capture")], "capture").unwrap();
        resolved.frontmatter.insert(
            "tags".to_string(),
            toml::Value::Array(vec![
                toml::Value::String("{{file_ext}}".to_string()),
                toml::Value::String("{{target_name}}".to_string()),
            ]),
        );
        resolved.frontmatter.insert(
            "meta".to_string(),
            toml::Value::Table(
                [(
                    "path".to_string(),
                    toml::Value::String("{{source_path}}".to_string()),
                )]
                .into_iter()
                .collect(),
            ),
        );

        let rendered = render_template(&resolved, &render_context());

        assert_eq!(
            rendered.frontmatter["tags"].as_array().unwrap(),
            &vec![
                toml::Value::String("pdf".to_string()),
                toml::Value::String("research".to_string())
            ]
        );
        assert_eq!(
            rendered.frontmatter["meta"]["path"].as_str(),
            Some(r"C:\Users\dev\Downloads\attention.pdf")
        );
    }

    #[test]
    fn renders_if_else_blocks() {
        let mut resolved = resolve_template(&[template("capture")], "capture").unwrap();
        resolved.frontmatter.insert(
            "type".to_string(),
            toml::Value::String("{{#if file_ext == \"pdf\"}}paper{{#else}}note{{/if}}".to_string()),
        );

        let rendered = render_template(&resolved, &render_context());

        assert_eq!(rendered.frontmatter["type"].as_str(), Some("paper"));
    }

    #[test]
    fn renders_conditionals_with_numeric_and_contains_operators() {
        let mut resolved = resolve_template(&[template("capture")], "capture").unwrap();
        resolved.frontmatter.insert(
            "size_label".to_string(),
            toml::Value::String(
                "{{#if file_size_kb >= 3 && file_name contains \"tention\"}}large{{#else}}small{{/if}}"
                    .to_string(),
            ),
        );

        let rendered = render_template(&resolved, &render_context());

        assert_eq!(rendered.frontmatter["size_label"].as_str(), Some("large"));
    }

    #[test]
    fn renders_nested_if_blocks() {
        let mut resolved = resolve_template(&[template("capture")], "capture").unwrap();
        resolved.frontmatter.insert(
            "status".to_string(),
            toml::Value::String(
                "{{#if file_ext == \"pdf\"}}{{#if file_size_kb > 2}}big-pdf{{#else}}small-pdf{{/if}}{{#else}}other{{/if}}"
                    .to_string(),
            ),
        );

        let rendered = render_template(&resolved, &render_context());

        assert_eq!(rendered.frontmatter["status"].as_str(), Some("big-pdf"));
    }

    #[test]
    fn malformed_if_expression_renders_else_branch() {
        let mut resolved = resolve_template(&[template("capture")], "capture").unwrap();
        resolved.frontmatter.insert(
            "status".to_string(),
            toml::Value::String("{{#if file_size_kb > }}good{{#else}}fallback{{/if}}".to_string()),
        );

        let rendered = render_template(&resolved, &render_context());

        assert_eq!(rendered.frontmatter["status"].as_str(), Some("fallback"));
    }

    #[test]
    fn leaves_unknown_variables_unchanged() {
        let resolved = RenderedTemplate {
            name: "capture".to_string(),
            subfolder: None,
            tags: Vec::new(),
            frontmatter: Table::new(),
        };
        let unresolved = super::render_string("{{unknown}}", &render_context());

        assert_eq!(resolved.name, "capture");
        assert_eq!(unresolved, "{{unknown}}");
    }
}
