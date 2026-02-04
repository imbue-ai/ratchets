//! Parsing and validation for ratchets.toml configuration files

use crate::error::ConfigError;
use crate::types::{GlobPattern, Language, RuleId, Severity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
        // Validate version
        if self.ratchets.version != "1" {
            return Err(ConfigError::Validation(format!(
                "Unsupported configuration version '{}'. Expected '1'",
                self.ratchets.version
            )));
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

        // Validate rule settings regions (if specified)
        for (rule_id, rule_value) in &self.rules.builtin {
            if let RuleValue::Settings(settings) = rule_value
                && let Some(regions) = &settings.regions
            {
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

        for (rule_id, rule_value) in &self.rules.custom {
            if let RuleValue::Settings(settings) = rule_value
                && let Some(regions) = &settings.regions
            {
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
    /// Configuration version (must be "1")
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
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RulesConfig {
    /// Built-in rules (flattened from `[rules]` table, excluding `[rules.custom]`)
    #[serde(flatten)]
    pub builtin: HashMap<RuleId, RuleValue>,

    /// Custom rules from `[rules.custom]` section
    #[serde(default)]
    pub custom: HashMap<RuleId, RuleValue>,
}

/// A rule can be enabled with a boolean or configured with settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleValue {
    /// Simple boolean enable/disable
    Enabled(bool),
    /// Settings table for the rule
    Settings(RuleSettings),
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
version = "1"
languages = ["rust", "typescript", "python"]
include = ["src/**", "tests/**"]
exclude = ["**/generated/**", "**/vendor/**"]

[rules]
no-unwrap = true
no-expect = true
no-todo-comments = { severity = "warning" }
no-fixme-comments = false

[rules.custom]
my-company-rule = true
legacy-api-usage = { regions = ["src/legacy/**"] }

[output]
format = "human"
color = "auto"
"#;

    #[test]
    fn test_valid_config_parsing() {
        let config = Config::parse(VALID_CONFIG).unwrap();

        assert_eq!(config.ratchets.version, "1");
        assert_eq!(config.ratchets.languages.len(), 3);
        assert!(config.ratchets.languages.contains(&Language::Rust));
        assert!(config.ratchets.languages.contains(&Language::TypeScript));
        assert!(config.ratchets.languages.contains(&Language::Python));

        assert_eq!(config.ratchets.include.len(), 2);
        assert_eq!(config.ratchets.exclude.len(), 2);

        // Check built-in rules
        assert_eq!(config.rules.builtin.len(), 4);
        assert_eq!(
            config.rules.builtin.get(&RuleId::new("no-unwrap").unwrap()),
            Some(&RuleValue::Enabled(true))
        );
        assert_eq!(
            config.rules.builtin.get(&RuleId::new("no-expect").unwrap()),
            Some(&RuleValue::Enabled(true))
        );
        assert_eq!(
            config
                .rules
                .builtin
                .get(&RuleId::new("no-fixme-comments").unwrap()),
            Some(&RuleValue::Enabled(false))
        );

        // Check rule with settings
        match config
            .rules
            .builtin
            .get(&RuleId::new("no-todo-comments").unwrap())
        {
            Some(RuleValue::Settings(settings)) => {
                assert_eq!(settings.severity, Some(Severity::Warning));
            }
            _ => panic!("Expected settings for no-todo-comments"),
        }

        // Check custom rules
        assert_eq!(config.rules.custom.len(), 2);
        assert_eq!(
            config
                .rules
                .custom
                .get(&RuleId::new("my-company-rule").unwrap()),
            Some(&RuleValue::Enabled(true))
        );

        // Check output settings
        assert_eq!(config.output.format, OutputFormat::Human);
        assert_eq!(config.output.color, ColorOption::Auto);
    }

    #[test]
    fn test_minimal_config() {
        let minimal = r#"
[ratchets]
version = "1"
languages = ["rust"]
"#;

        let config = Config::parse(minimal).unwrap();
        assert_eq!(config.ratchets.version, "1");
        assert_eq!(config.ratchets.languages.len(), 1);
        assert_eq!(config.ratchets.include.len(), 1); // Default "**/*"
        assert_eq!(config.ratchets.exclude.len(), 0);
        assert_eq!(config.output.format, OutputFormat::Human);
        assert_eq!(config.output.color, ColorOption::Auto);
    }

    #[test]
    fn test_invalid_version() {
        let invalid = r#"
[ratchets]
version = "2"
languages = ["rust"]
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported configuration version")
        );
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
version = "1"
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
version = "1"
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
version = "1"
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
version = "1"
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
    fn test_jsonl_output_format() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]

[output]
format = "jsonl"
color = "never"
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.output.format, OutputFormat::Jsonl);
        assert_eq!(config.output.color, ColorOption::Never);
    }

    #[test]
    fn test_color_always() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]

[output]
color = "always"
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.output.color, ColorOption::Always);
    }

    #[test]
    fn test_rule_with_severity_and_regions() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]

[rules]
my-rule = { severity = "error", regions = ["src/**", "tests/**"] }
"#;

        let config = Config::parse(config_str).unwrap();
        match config.rules.builtin.get(&RuleId::new("my-rule").unwrap()) {
            Some(RuleValue::Settings(settings)) => {
                assert_eq!(settings.severity, Some(Severity::Error));
                assert_eq!(settings.regions.as_ref().unwrap().len(), 2);
            }
            _ => panic!("Expected settings for my-rule"),
        }
    }

    #[test]
    fn test_custom_rules_with_settings() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]

[rules.custom]
custom-rule-1 = true
custom-rule-2 = { severity = "warning" }
custom-rule-3 = { regions = ["src/legacy/**"] }
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.rules.custom.len(), 3);

        assert_eq!(
            config
                .rules
                .custom
                .get(&RuleId::new("custom-rule-1").unwrap()),
            Some(&RuleValue::Enabled(true))
        );

        match config
            .rules
            .custom
            .get(&RuleId::new("custom-rule-2").unwrap())
        {
            Some(RuleValue::Settings(settings)) => {
                assert_eq!(settings.severity, Some(Severity::Warning));
            }
            _ => panic!("Expected settings for custom-rule-2"),
        }

        match config
            .rules
            .custom
            .get(&RuleId::new("custom-rule-3").unwrap())
        {
            Some(RuleValue::Settings(settings)) => {
                assert_eq!(settings.regions.as_ref().unwrap().len(), 1);
            }
            _ => panic!("Expected settings for custom-rule-3"),
        }
    }

    #[test]
    fn test_multiple_languages() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust", "typescript", "javascript", "python", "go"]
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.ratchets.languages.len(), 5);
        assert!(config.ratchets.languages.contains(&Language::Rust));
        assert!(config.ratchets.languages.contains(&Language::TypeScript));
        assert!(config.ratchets.languages.contains(&Language::JavaScript));
        assert!(config.ratchets.languages.contains(&Language::Python));
        assert!(config.ratchets.languages.contains(&Language::Go));
    }

    #[test]
    fn test_invalid_language() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust", "invalid"]
"#;

        let result = Config::parse(config_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_round_trip() {
        let config = Config::parse(VALID_CONFIG).unwrap();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized = Config::parse(&serialized).unwrap();

        // Compare key fields (order may differ in serialization)
        assert_eq!(config.ratchets.version, deserialized.ratchets.version);
        assert_eq!(config.ratchets.languages, deserialized.ratchets.languages);
        assert_eq!(config.output.format, deserialized.output.format);
        assert_eq!(config.output.color, deserialized.output.color);
    }

    #[test]
    fn test_missing_languages_field() {
        let invalid = r#"
[ratchets]
version = "1"
"#;

        let result = Config::parse(invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(
            "No languages configured. Add languages to ratchets.toml to start checking."
        ));
    }

    #[test]
    fn test_empty_include_patterns() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = []
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.ratchets.include.len(), 0);
    }

    #[test]
    fn test_multiple_glob_patterns() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["src/**/*.rs", "tests/**/*.rs", "benches/**/*.rs"]
exclude = ["**/target/**", "**/generated/**", "**/*.bak"]
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.ratchets.include.len(), 3);
        assert_eq!(config.ratchets.exclude.len(), 3);
    }

    #[test]
    fn test_rule_disabled_explicitly() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]

[rules]
no-unwrap = false
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(
            config.rules.builtin.get(&RuleId::new("no-unwrap").unwrap()),
            Some(&RuleValue::Enabled(false))
        );
    }

    #[test]
    fn test_rule_with_only_severity() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]

[rules]
my-rule = { severity = "info" }
"#;

        let config = Config::parse(config_str).unwrap();
        match config.rules.builtin.get(&RuleId::new("my-rule").unwrap()) {
            Some(RuleValue::Settings(settings)) => {
                assert_eq!(settings.severity, Some(Severity::Info));
                assert!(settings.regions.is_none());
            }
            _ => panic!("Expected settings for my-rule"),
        }
    }

    #[test]
    fn test_rule_with_only_regions() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]

[rules]
my-rule = { regions = ["src/**"] }
"#;

        let config = Config::parse(config_str).unwrap();
        match config.rules.builtin.get(&RuleId::new("my-rule").unwrap()) {
            Some(RuleValue::Settings(settings)) => {
                assert!(settings.severity.is_none());
                assert_eq!(settings.regions.as_ref().unwrap().len(), 1);
            }
            _ => panic!("Expected settings for my-rule"),
        }
    }

    #[test]
    fn test_output_format_default() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.output.format, OutputFormat::Human);
    }

    #[test]
    fn test_color_option_default() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.output.color, ColorOption::Auto);
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
    fn test_complex_rule_combinations() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust", "python"]

[rules]
rule-1 = true
rule-2 = false
rule-3 = { severity = "error" }
rule-4 = { regions = ["src/**"] }
rule-5 = { severity = "warning", regions = ["tests/**"] }

[rules.custom]
custom-1 = true
custom-2 = { severity = "info" }
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.rules.builtin.len(), 5);
        assert_eq!(config.rules.custom.len(), 2);
    }

    #[test]
    fn test_invalid_output_format() {
        let invalid = r#"
[ratchets]
version = "1"
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
version = "1"
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
version = "1"
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
version = "1"
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
    fn test_all_supported_languages() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust", "typescript", "javascript", "python", "go"]
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.ratchets.languages.len(), 5);
    }

    #[test]
    fn test_single_language() {
        for lang in &["rust", "typescript", "javascript", "python", "go"] {
            let config_str = format!(
                r#"
[ratchets]
version = "1"
languages = ["{}"]
"#,
                lang
            );

            let config = Config::parse(&config_str).unwrap();
            assert_eq!(config.ratchets.languages.len(), 1);
        }
    }

    #[test]
    fn test_patterns_section() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]

[patterns]
python_tests = ["**/test_*.py", "**/*_test.py", "**/tests/**"]
rust_tests = ["**/tests/**", "**/benches/**"]
generated = ["**/generated/**", "**/vendor/**"]
"#;

        let config = Config::parse(config_str).unwrap();
        assert_eq!(config.patterns.len(), 3);
        assert_eq!(config.patterns.get("python_tests").unwrap().len(), 3);
        assert_eq!(config.patterns.get("rust_tests").unwrap().len(), 2);
        assert_eq!(config.patterns.get("generated").unwrap().len(), 2);
    }

    #[test]
    fn test_patterns_section_empty() {
        let config_str = r#"
[ratchets]
version = "1"
languages = ["rust"]
"#;

        let config = Config::parse(config_str).unwrap();
        assert!(config.patterns.is_empty());
    }

    #[test]
    fn test_patterns_section_invalid_glob() {
        let config_str = r#"
[ratchets]
version = "1"
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
