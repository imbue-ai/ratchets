//! Parsing and validation for ratchets.toml configuration files

use crate::error::ConfigError;
use crate::types::{GlobPattern, Language, RuleId, SetId, Severity};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

/// Expected `[ratchets].version` value for the current schema.
///
/// The library only parses configs that match this exact string; everything
/// else is rejected via [`ConfigError::UnsupportedVersion`].
const EXPECTED_CONFIG_VERSION: &str = "2";

/// Main configuration struct for ratchets.toml
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Ratchets metadata
    pub ratchets: RatchetsMeta,

    /// Rule configuration
    #[serde(default)]
    pub rules: RulesConfig,

    /// Output configuration
    #[serde(default)]
    pub output: OutputConfig,

    /// Reusable pattern definitions
    #[serde(default)]
    pub patterns: HashMap<String, Vec<GlobPattern>>,

    /// Enabled ratchets — a mix of single rule IDs and `$set-name` references.
    ///
    /// Each entry is a single TOML string: a leading `$` strips and parses the
    /// remainder as a [`SetId`], otherwise the bare string parses as a
    /// [`RuleId`].
    #[serde(default)]
    pub enabled_ratchets: Vec<RatchetRef>,

    /// Disabled ratchets — same shape as `enabled_ratchets`. Disabled wins over
    /// enabled at resolution time.
    #[serde(default)]
    pub disabled_ratchets: Vec<RatchetRef>,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse configuration from a TOML string
    pub fn parse(s: &str) -> Result<Self, ConfigError> {
        let config: Config = toml::from_str(s)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate version. Anything other than the current `EXPECTED_CONFIG_VERSION`
        // (including the previously valid `"1"`) is rejected via a structured
        // error so the CLI layer can render the embedded upgrade notice.
        if self.ratchets.version != EXPECTED_CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion(
                self.ratchets.version.clone(),
            ));
        }

        // Validate that at least one language is specified
        if self.ratchets.languages.is_empty() {
            return Err(ConfigError::Validation(
                "No languages configured. Add languages to ratchets.toml to start checking."
                    .to_string(),
            ));
        }

        // Validate glob patterns by attempting to compile them with globset
        for pattern in &self.ratchets.include {
            globset::Glob::new(pattern.as_str()).map_err(|e| {
                ConfigError::Validation(format!(
                    "Invalid include glob pattern '{}': {}",
                    pattern.as_str(),
                    e
                ))
            })?;
        }

        for pattern in &self.ratchets.exclude {
            globset::Glob::new(pattern.as_str()).map_err(|e| {
                ConfigError::Validation(format!(
                    "Invalid exclude glob pattern '{}': {}",
                    pattern.as_str(),
                    e
                ))
            })?;
        }

        // Validate rule settings regions (if specified). Post-v2 the only
        // shape allowed under `[rules]` is a settings table, so every entry
        // is iterated directly.
        for (rule_id, settings) in &self.rules.builtin {
            if let Some(regions) = &settings.regions {
                for region in regions {
                    globset::Glob::new(region.as_str()).map_err(|e| {
                        ConfigError::Validation(format!(
                            "Invalid region glob pattern '{}' for rule '{}': {}",
                            region.as_str(),
                            rule_id.as_str(),
                            e
                        ))
                    })?;
                }
            }
        }

        for (rule_id, settings) in &self.rules.custom {
            if let Some(regions) = &settings.regions {
                for region in regions {
                    globset::Glob::new(region.as_str()).map_err(|e| {
                        ConfigError::Validation(format!(
                            "Invalid region glob pattern '{}' for custom rule '{}': {}",
                            region.as_str(),
                            rule_id.as_str(),
                            e
                        ))
                    })?;
                }
            }
        }

        // Validate pattern definitions
        for (pattern_name, patterns) in &self.patterns {
            for pattern in patterns {
                globset::Glob::new(pattern.as_str()).map_err(|e| {
                    ConfigError::Validation(format!(
                        "Invalid glob pattern '{}' in pattern '{}': {}",
                        pattern.as_str(),
                        pattern_name,
                        e
                    ))
                })?;
            }
        }

        Ok(())
    }
}

/// Ratchets metadata section
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatchetsMeta {
    /// Configuration schema version (must equal [`EXPECTED_CONFIG_VERSION`],
    /// currently `"2"`). Any other value is rejected via
    /// [`ConfigError::UnsupportedVersion`].
    pub version: String,

    /// Languages to analyze
    #[serde(default)]
    pub languages: Vec<Language>,

    /// File patterns to include
    #[serde(default = "default_include")]
    pub include: Vec<GlobPattern>,

    /// File patterns to exclude
    #[serde(default)]
    pub exclude: Vec<GlobPattern>,
}

fn default_include() -> Vec<GlobPattern> {
    vec![GlobPattern::new("**/*")]
}

/// Rules configuration section
///
/// Each entry maps a rule ID to a settings table. Enable / disable lives in
/// [`Config::enabled_ratchets`] / [`Config::disabled_ratchets`], not here.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RulesConfig {
    /// Built-in rules (flattened from `[rules]` table, excluding `[rules.custom]`)
    #[serde(flatten)]
    pub builtin: HashMap<RuleId, RuleSettings>,

    /// Custom rules from `[rules.custom]` section
    #[serde(default)]
    pub custom: HashMap<RuleId, RuleSettings>,
}

/// Settings for individual rules
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleSettings {
    /// Severity level for this rule
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,

    /// Specific regions (glob patterns) where this rule applies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regions: Option<Vec<GlobPattern>>,
}

/// A single entry in `enabled_ratchets` / `disabled_ratchets`.
///
/// Serialized as a TOML string. A leading `$` strips and parses the remainder
/// as a [`SetId`]; otherwise the bare string parses as a [`RuleId`]. The
/// `@`-prefix remains reserved for the existing `[patterns]` glob-reference
/// mechanism and is rejected here by the underlying `RuleId` validator.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RatchetRef {
    /// A ratchet-set reference (written as `"$set-name"` in TOML).
    Set(SetId),
    /// A single rule reference (written as the bare `"rule-id"` in TOML).
    Rule(RuleId),
}

impl fmt::Display for RatchetRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RatchetRef::Set(id) => write!(f, "${}", id),
            RatchetRef::Rule(id) => write!(f, "{}", id),
        }
    }
}

impl Serialize for RatchetRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RatchetRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        if let Some(rest) = raw.strip_prefix('$') {
            let set_id = SetId::new(rest).ok_or_else(|| {
                de::Error::custom(format!(
                    "invalid ratchet-set reference '{}': expected '$<set-id>' with alphanumeric + '-' + '_'",
                    raw
                ))
            })?;
            Ok(RatchetRef::Set(set_id))
        } else {
            let rule_id = RuleId::new(&raw).ok_or_else(|| {
                de::Error::custom(format!(
                    "invalid ratchet reference '{}': expected a bare rule ID (alphanumeric + '-' + '_') or '$<set-id>'",
                    raw
                ))
            })?;
            Ok(RatchetRef::Rule(rule_id))
        }
    }
}

/// Output configuration section
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output format
    #[serde(default)]
    pub format: OutputFormat,

    /// Color output setting
    #[serde(default)]
    pub color: ColorOption,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::Human,
            color: ColorOption::Auto,
        }
    }
}

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable output
    #[default]
    Human,
    /// JSON Lines format
    Jsonl,
}

/// Color output options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorOption {
    /// Auto-detect based on terminal capabilities
    #[default]
    Auto,
    /// Always use color
    Always,
    /// Never use color
    Never,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = r#"
[ratchets]
version = "2"
languages = ["rust", "typescript", "python"]
include = ["src/**", "tests/**"]
exclude = ["**/generated/**", "**/vendor/**"]

[rules]
no-todo-comments = { severity = "warning" }

[rules.custom]
legacy-api-usage = { regions = ["src/legacy/**"] }

[output]
format = "human"
color = "auto"
"#;

    #[test]
    fn test_valid_config_parsing() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::parse(VALID_CONFIG)?;

        assert_eq!(config.ratchets.version, "2");
        assert_eq!(config.ratchets.languages.len(), 3);
        assert!(config.ratchets.languages.contains(&Language::Rust));
        assert!(config.ratchets.languages.contains(&Language::TypeScript));
        assert!(config.ratchets.languages.contains(&Language::Python));

        assert_eq!(config.ratchets.include.len(), 2);
        assert_eq!(config.ratchets.exclude.len(), 2);

        // Only settings tables remain under [rules]; the boolean shorthand is
        // not accepted.
        assert_eq!(config.rules.builtin.len(), 1);
        let no_todo_settings = config
            .rules
            .builtin
            .get(&RuleId::new("no-todo-comments").ok_or("invalid rule id")?)
            .ok_or("no-todo-comments settings should be present")?;
        assert_eq!(no_todo_settings.severity, Some(Severity::Warning));

        // Check custom rules
        assert_eq!(config.rules.custom.len(), 1);
        let legacy_settings = config
            .rules
            .custom
            .get(&RuleId::new("legacy-api-usage").ok_or("invalid rule id")?)
            .ok_or("legacy-api-usage settings should be present")?;
        assert_eq!(
            legacy_settings
                .regions
                .as_ref()
                .ok_or("regions should be present")?
                .len(),
            1
        );

        // Check output settings
        assert_eq!(config.output.format, OutputFormat::Human);
        assert_eq!(config.output.color, ColorOption::Auto);
        Ok(())
    }

    #[test]
    fn test_minimal_config() -> Result<(), Box<dyn std::error::Error>> {
        let minimal = r#"
[ratchets]
version = "2"
languages = ["rust"]
"#;

        let config = Config::parse(minimal)?;
        assert_eq!(config.ratchets.version, "2");
        assert_eq!(config.ratchets.languages.len(), 1);
        assert_eq!(config.ratchets.include.len(), 1); // Default "**/*"
        assert_eq!(config.ratchets.exclude.len(), 0);
        assert_eq!(config.output.format, OutputFormat::Human);
        assert_eq!(config.output.color, ColorOption::Auto);
        // New fields default to empty vectors when omitted.
        assert!(config.enabled_ratchets.is_empty());
        assert!(config.disabled_ratchets.is_empty());
        Ok(())
    }

    #[test]
    fn test_invalid_version_v1_rejected() -> Result<(), Box<dyn std::error::Error>> {
        // The legacy v1 schema is now a hard error: the structured
        // `UnsupportedVersion` variant lets the CLI layer render the upgrade
        // notice instead of a generic validation string.
        let invalid = r#"
[ratchets]
version = "1"
languages = ["rust"]
"#;

        let err = Config::parse(invalid).expect_err("v1 config must be rejected");
        match err {
            ConfigError::UnsupportedVersion(ref v) => assert_eq!(v, "1"),
            other => return Err(format!("expected UnsupportedVersion, got {:?}", other).into()),
        }
        assert!(
            err.to_string()
                .contains("Unsupported configuration version")
        );
        Ok(())
    }

    #[test]
    fn test_invalid_version_unknown_rejected() {
        // Any non-`"2"` value should route to `UnsupportedVersion` — not just
        // the previously valid `"1"`.
        let invalid = r#"
[ratchets]
version = "99"
languages = ["rust"]
"#;

        let err = Config::parse(invalid).expect_err("unknown version must be rejected");
        assert!(matches!(err, ConfigError::UnsupportedVersion(ref v) if v == "99"));
    }

    #[test]
    fn test_missing_version() {
        let invalid = r#"
[ratchets]
languages = ["rust"]
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_languages() {
        let invalid = r#"
[ratchets]
version = "2"
languages = []
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(
            "No languages configured. Add languages to ratchets.toml to start checking."
        ));
    }

    #[test]
    fn test_invalid_glob_pattern_include() {
        let invalid = r#"
[ratchets]
version = "2"
languages = ["rust"]
include = ["[invalid"]
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid include glob pattern")
        );
    }

    #[test]
    fn test_invalid_glob_pattern_exclude() {
        let invalid = r#"
[ratchets]
version = "2"
languages = ["rust"]
exclude = ["[invalid"]
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid exclude glob pattern")
        );
    }

    #[test]
    fn test_invalid_rule_region_glob() {
        let invalid = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules]
no-unwrap = { regions = ["[invalid"] }
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid region glob pattern")
        );
    }

    #[test]
    fn test_jsonl_output_format() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]

[output]
format = "jsonl"
color = "never"
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.output.format, OutputFormat::Jsonl);
        assert_eq!(config.output.color, ColorOption::Never);
        Ok(())
    }

    #[test]
    fn test_color_always() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]

[output]
color = "always"
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.output.color, ColorOption::Always);
        Ok(())
    }

    #[test]
    fn test_rule_with_severity_and_regions() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules]
my-rule = { severity = "error", regions = ["src/**", "tests/**"] }
"#;

        let config = Config::parse(config_str)?;
        let settings = config
            .rules
            .builtin
            .get(&RuleId::new("my-rule").ok_or("invalid rule id")?)
            .ok_or("my-rule settings should be present")?;
        assert_eq!(settings.severity, Some(Severity::Error));
        assert_eq!(
            settings
                .regions
                .as_ref()
                .ok_or("regions should be present")?
                .len(),
            2
        );
        Ok(())
    }

    #[test]
    fn test_custom_rules_with_settings() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules.custom]
custom-rule-2 = { severity = "warning" }
custom-rule-3 = { regions = ["src/legacy/**"] }
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.rules.custom.len(), 2);

        let s2 = config
            .rules
            .custom
            .get(&RuleId::new("custom-rule-2").ok_or("invalid rule id")?)
            .ok_or("custom-rule-2 settings should be present")?;
        assert_eq!(s2.severity, Some(Severity::Warning));

        let s3 = config
            .rules
            .custom
            .get(&RuleId::new("custom-rule-3").ok_or("invalid rule id")?)
            .ok_or("custom-rule-3 settings should be present")?;
        assert_eq!(
            s3.regions
                .as_ref()
                .ok_or("regions should be present")?
                .len(),
            1
        );
        Ok(())
    }

    #[test]
    fn test_multiple_languages() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust", "typescript", "javascript", "python", "go"]
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.ratchets.languages.len(), 5);
        assert!(config.ratchets.languages.contains(&Language::Rust));
        assert!(config.ratchets.languages.contains(&Language::TypeScript));
        assert!(config.ratchets.languages.contains(&Language::JavaScript));
        assert!(config.ratchets.languages.contains(&Language::Python));
        assert!(config.ratchets.languages.contains(&Language::Go));
        Ok(())
    }

    #[test]
    fn test_invalid_language() {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust", "invalid"]
"#;

        let result = Config::parse(config_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::parse(VALID_CONFIG)?;
        let serialized = toml::to_string(&config)?;
        let deserialized = Config::parse(&serialized)?;

        // Compare key fields (order may differ in serialization)
        assert_eq!(config.ratchets.version, deserialized.ratchets.version);
        assert_eq!(config.ratchets.languages, deserialized.ratchets.languages);
        assert_eq!(config.output.format, deserialized.output.format);
        assert_eq!(config.output.color, deserialized.output.color);
        Ok(())
    }

    #[test]
    fn test_missing_languages_field() {
        let invalid = r#"
[ratchets]
version = "2"
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(
            "No languages configured. Add languages to ratchets.toml to start checking."
        ));
    }

    #[test]
    fn test_empty_include_patterns() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]
include = []
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.ratchets.include.len(), 0);
        Ok(())
    }

    #[test]
    fn test_multiple_glob_patterns() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]
include = ["src/**/*.rs", "tests/**/*.rs", "benches/**/*.rs"]
exclude = ["**/target/**", "**/generated/**", "**/*.bak"]
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.ratchets.include.len(), 3);
        assert_eq!(config.ratchets.exclude.len(), 3);
        Ok(())
    }

    #[test]
    fn test_rule_boolean_shorthand_rejected() {
        // `[rules].rule-id = false` (and `= true`) was the v1 enable/disable
        // shorthand. In v2 enable/disable lives in `enabled_ratchets` /
        // `disabled_ratchets`, and `[rules]` only accepts settings tables, so
        // the boolean form is a TOML parse error.
        let cfg_false = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules]
no-unwrap = false
"#;
        assert!(matches!(
            Config::parse(cfg_false).expect_err("boolean shorthand must fail"),
            ConfigError::Parse(_)
        ));

        let cfg_true = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules]
no-unwrap = true
"#;
        assert!(matches!(
            Config::parse(cfg_true).expect_err("boolean shorthand must fail"),
            ConfigError::Parse(_)
        ));
    }

    #[test]
    fn test_rule_with_only_severity() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules]
my-rule = { severity = "info" }
"#;

        let config = Config::parse(config_str)?;
        let settings = config
            .rules
            .builtin
            .get(&RuleId::new("my-rule").ok_or("invalid rule id")?)
            .ok_or("my-rule settings should be present")?;
        assert_eq!(settings.severity, Some(Severity::Info));
        assert!(settings.regions.is_none());
        Ok(())
    }

    #[test]
    fn test_rule_with_only_regions() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules]
my-rule = { regions = ["src/**"] }
"#;

        let config = Config::parse(config_str)?;
        let settings = config
            .rules
            .builtin
            .get(&RuleId::new("my-rule").ok_or("invalid rule id")?)
            .ok_or("my-rule settings should be present")?;
        assert!(settings.severity.is_none());
        assert_eq!(
            settings
                .regions
                .as_ref()
                .ok_or("regions should be present")?
                .len(),
            1
        );
        Ok(())
    }

    #[test]
    fn test_output_format_default() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.output.format, OutputFormat::Human);
        Ok(())
    }

    #[test]
    fn test_color_option_default() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.output.color, ColorOption::Auto);
        Ok(())
    }

    #[test]
    fn test_version_must_be_string() {
        let invalid = r#"
[ratchets]
version = 1
languages = ["rust"]
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_rule_combinations() -> Result<(), Box<dyn std::error::Error>> {
        // Post-v2 every `[rules]` entry is a settings table — the boolean
        // shorthand is gone, and enable/disable lives in
        // `enabled_ratchets` / `disabled_ratchets`.
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust", "python"]

[rules]
rule-3 = { severity = "error" }
rule-4 = { regions = ["src/**"] }
rule-5 = { severity = "warning", regions = ["tests/**"] }

[rules.custom]
custom-2 = { severity = "info" }
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.rules.builtin.len(), 3);
        assert_eq!(config.rules.custom.len(), 1);
        Ok(())
    }

    #[test]
    fn test_invalid_output_format() {
        let invalid = r#"
[ratchets]
version = "2"
languages = ["rust"]

[output]
format = "xml"
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_color_option() {
        let invalid = r#"
[ratchets]
version = "2"
languages = ["rust"]

[output]
color = "sometimes"
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_severity() {
        let invalid = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules]
my-rule = { severity = "critical" }
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_rule_with_invalid_region_glob() {
        let invalid = r#"
[ratchets]
version = "2"
languages = ["rust"]

[rules.custom]
my-rule = { regions = ["[invalid"] }
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid region glob pattern")
        );
    }

    #[test]
    fn test_all_supported_languages() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust", "typescript", "javascript", "python", "go"]
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.ratchets.languages.len(), 5);
        Ok(())
    }

    #[test]
    fn test_single_language() -> Result<(), Box<dyn std::error::Error>> {
        for lang in &["rust", "typescript", "javascript", "python", "go"] {
            let config_str = format!(
                r#"
[ratchets]
version = "2"
languages = ["{}"]
"#,
                lang
            );

            let config = Config::parse(&config_str)?;
            assert_eq!(config.ratchets.languages.len(), 1);
        }
        Ok(())
    }

    #[test]
    fn test_patterns_section() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]

[patterns]
python_tests = ["**/test_*.py", "**/*_test.py", "**/tests/**"]
rust_tests = ["**/tests/**", "**/benches/**"]
generated = ["**/generated/**", "**/vendor/**"]
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.patterns.len(), 3);
        assert_eq!(
            config
                .patterns
                .get("python_tests")
                .ok_or("python_tests pattern should be present")?
                .len(),
            3
        );
        assert_eq!(
            config
                .patterns
                .get("rust_tests")
                .ok_or("rust_tests pattern should be present")?
                .len(),
            2
        );
        assert_eq!(
            config
                .patterns
                .get("generated")
                .ok_or("generated pattern should be present")?
                .len(),
            2
        );
        Ok(())
    }

    #[test]
    fn test_patterns_section_empty() -> Result<(), Box<dyn std::error::Error>> {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]
"#;

        let config = Config::parse(config_str)?;
        assert!(config.patterns.is_empty());
        Ok(())
    }

    #[test]
    fn test_enabled_and_disabled_ratchets_parsed() -> Result<(), Box<dyn std::error::Error>> {
        // The deserializer accepts bare rule IDs and `$set-name` references.
        // Both arrays live at the root of `ratchets.toml`, not inside the
        // `[ratchets]` table, so they appear before the first table header.
        let config_str = r#"
enabled_ratchets = ["$common-starter", "no-unwrap"]
disabled_ratchets = ["no-todo-comments", "$strict-extras"]

[ratchets]
version = "2"
languages = ["rust"]
"#;

        let config = Config::parse(config_str)?;
        assert_eq!(config.enabled_ratchets.len(), 2);
        assert_eq!(config.disabled_ratchets.len(), 2);

        assert!(matches!(
            &config.enabled_ratchets[0],
            RatchetRef::Set(id) if id.as_str() == "common-starter"
        ));
        assert!(matches!(
            &config.enabled_ratchets[1],
            RatchetRef::Rule(id) if id.as_str() == "no-unwrap"
        ));
        assert!(matches!(
            &config.disabled_ratchets[0],
            RatchetRef::Rule(id) if id.as_str() == "no-todo-comments"
        ));
        assert!(matches!(
            &config.disabled_ratchets[1],
            RatchetRef::Set(id) if id.as_str() == "strict-extras"
        ));
        Ok(())
    }

    #[test]
    fn test_ratchet_ref_invalid_set_id_rejected() {
        // `$` followed by an invalid identifier must fail at parse time.
        let config_str = r#"
enabled_ratchets = ["$bad set"]

[ratchets]
version = "2"
languages = ["rust"]
"#;
        let err = Config::parse(config_str).expect_err("invalid set id must fail");
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn test_ratchet_ref_invalid_rule_id_rejected() {
        let config_str = r#"
enabled_ratchets = ["bad rule"]

[ratchets]
version = "2"
languages = ["rust"]
"#;
        let err = Config::parse(config_str).expect_err("invalid rule id must fail");
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn test_ratchet_ref_round_trip_serializes_with_dollar_prefix()
    -> Result<(), Box<dyn std::error::Error>> {
        let set_ref = RatchetRef::Set(SetId::new("common-starter").ok_or("invalid set id")?);
        let rule_ref = RatchetRef::Rule(RuleId::new("no-unwrap").ok_or("invalid rule id")?);
        assert_eq!(serde_json::to_string(&set_ref)?, "\"$common-starter\"");
        assert_eq!(serde_json::to_string(&rule_ref)?, "\"no-unwrap\"");
        Ok(())
    }

    #[test]
    fn test_patterns_section_invalid_glob() {
        let config_str = r#"
[ratchets]
version = "2"
languages = ["rust"]

[patterns]
bad_pattern = ["[invalid"]
"#;

        let result = Config::parse(config_str);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid glob pattern")
        );
    }
}
