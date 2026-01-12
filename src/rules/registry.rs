#![forbid(unsafe_code)]

//! Rule registry for managing and loading rules
//!
//! The RuleRegistry is responsible for:
//! - Loading built-in regex rules from builtin-ratchets/
//! - Loading custom regex rules from ratchets/regex/
//! - Filtering rules based on configuration
//! - Providing access to rules by ID

use crate::config::ratchet_toml::{RuleValue, RulesConfig};
use crate::error::RuleError;
use crate::rules::{RegexRule, Rule};
use crate::types::RuleId;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Registry for storing and managing all rules
///
/// The registry holds all loaded rules, keyed by their unique RuleId.
/// Rules are stored as trait objects to allow for different rule implementations.
pub struct RuleRegistry {
    rules: HashMap<RuleId, Box<dyn Rule>>,
}

impl RuleRegistry {
    /// Create a new empty RuleRegistry
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Load built-in regex rules from a directory
    ///
    /// This method scans the specified directory for `.toml` files and attempts
    /// to load each as a RegexRule. If the directory doesn't exist, a warning
    /// is logged but the operation succeeds.
    ///
    /// # Arguments
    ///
    /// * `builtin_dir` - Path to the builtin-ratchets/regex/ directory
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if:
    /// - A TOML file cannot be parsed
    /// - A rule definition is invalid
    /// - There is an I/O error reading a file
    pub fn load_builtin_regex_rules(&mut self, builtin_dir: &Path) -> Result<(), RuleError> {
        self.load_regex_rules_from_dir(builtin_dir)
    }

    /// Load custom regex rules from a directory
    ///
    /// This method scans the specified directory for `.toml` files and attempts
    /// to load each as a RegexRule. If the directory doesn't exist, a warning
    /// is logged but the operation succeeds.
    ///
    /// # Arguments
    ///
    /// * `custom_dir` - Path to the ratchets/regex/ directory
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if:
    /// - A TOML file cannot be parsed
    /// - A rule definition is invalid
    /// - There is an I/O error reading a file
    pub fn load_custom_regex_rules(&mut self, custom_dir: &Path) -> Result<(), RuleError> {
        self.load_regex_rules_from_dir(custom_dir)
    }

    /// Internal helper to load regex rules from a directory
    ///
    /// Scans for .toml files and loads them as RegexRules.
    fn load_regex_rules_from_dir(&mut self, dir: &Path) -> Result<(), RuleError> {
        // Check if directory exists
        if !dir.exists() {
            // Log warning but don't fail - missing directories are OK
            eprintln!("Warning: Rule directory does not exist: {}", dir.display());
            return Ok(());
        }

        if !dir.is_dir() {
            return Err(RuleError::InvalidDefinition(format!(
                "Path is not a directory: {}",
                dir.display()
            )));
        }

        // Read all entries in the directory
        let entries = fs::read_dir(dir).map_err(|e| {
            RuleError::InvalidDefinition(format!(
                "Failed to read directory {}: {}",
                dir.display(),
                e
            ))
        })?;

        // Process each .toml file
        for entry in entries {
            let entry = entry.map_err(|e| {
                RuleError::InvalidDefinition(format!(
                    "Failed to read directory entry in {}: {}",
                    dir.display(),
                    e
                ))
            })?;

            let path = entry.path();

            // Skip non-files
            if !path.is_file() {
                continue;
            }

            // Only process .toml files
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }

            // Load the rule
            let rule = RegexRule::from_path(&path)?;
            let rule_id = rule.id().clone();

            // Check for duplicate rule IDs
            if self.rules.contains_key(&rule_id) {
                return Err(RuleError::InvalidDefinition(format!(
                    "Duplicate rule ID '{}' in file {}",
                    rule_id.as_str(),
                    path.display()
                )));
            }

            // Add rule to registry
            self.rules.insert(rule_id, Box::new(rule));
        }

        Ok(())
    }

    /// Filter rules based on configuration
    ///
    /// This method removes rules that are disabled in the configuration.
    /// Rules are enabled by default unless explicitly disabled.
    ///
    /// # Arguments
    ///
    /// * `config` - The rules configuration from ratchet.toml
    pub fn filter_by_config(&mut self, config: &RulesConfig) {
        // Collect rule IDs to remove
        let mut to_remove = Vec::new();

        for rule_id in self.rules.keys() {
            // Check if rule is in builtin config
            if let Some(rule_value) = config.builtin.get(rule_id) {
                if !is_rule_enabled(rule_value) {
                    to_remove.push(rule_id.clone());
                }
                continue;
            }

            // Check if rule is in custom config
            if let Some(rule_value) = config.custom.get(rule_id) {
                if !is_rule_enabled(rule_value) {
                    to_remove.push(rule_id.clone());
                }
                continue;
            }

            // Rule not in config - keep it enabled by default
        }

        // Remove disabled rules
        for rule_id in to_remove {
            self.rules.remove(&rule_id);
        }
    }

    /// Get a rule by its ID
    ///
    /// Returns `None` if the rule is not found in the registry.
    ///
    /// # Arguments
    ///
    /// * `id` - The rule ID to look up
    pub fn get_rule(&self, id: &RuleId) -> Option<&dyn Rule> {
        self.rules.get(id).map(|boxed| boxed.as_ref())
    }

    /// Iterate over all rules in the registry
    ///
    /// Returns an iterator over references to all rules.
    pub fn iter_rules(&self) -> impl Iterator<Item = &dyn Rule> {
        self.rules.values().map(|boxed| boxed.as_ref())
    }

    /// Get the number of rules in the registry
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to determine if a rule is enabled
fn is_rule_enabled(rule_value: &RuleValue) -> bool {
    match rule_value {
        RuleValue::Enabled(enabled) => *enabled,
        RuleValue::Settings(_) => true, // If settings are provided, rule is enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper to create a test TOML file
    fn create_test_rule_file(dir: &Path, filename: &str, rule_id: &str) -> PathBuf {
        let toml_content = format!(
            r#"
[rule]
id = "{}"
description = "Test rule"
severity = "warning"

[match]
pattern = "TODO"
"#,
            rule_id
        );

        let file_path = dir.join(filename);
        fs::write(&file_path, toml_content).unwrap();
        file_path
    }

    #[test]
    fn test_new_registry() {
        let registry = RuleRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_default_registry() {
        let registry = RuleRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_load_builtin_regex_rules_missing_dir() {
        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_regex_rules(Path::new("/nonexistent/path"));
        assert!(result.is_ok()); // Should succeed with warning
        assert!(registry.is_empty());
    }

    #[test]
    fn test_load_custom_regex_rules_missing_dir() {
        let mut registry = RuleRegistry::new();
        let result = registry.load_custom_regex_rules(Path::new("/nonexistent/path"));
        assert!(result.is_ok()); // Should succeed with warning
        assert!(registry.is_empty());
    }

    #[test]
    fn test_load_single_rule() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "test-rule.toml", "test-rule");

        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_regex_rules(temp_dir.path());
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1);

        let rule_id = RuleId::new("test-rule").unwrap();
        assert!(registry.get_rule(&rule_id).is_some());
    }

    #[test]
    fn test_load_multiple_rules() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule1.toml", "rule-1");
        create_test_rule_file(temp_dir.path(), "rule2.toml", "rule-2");
        create_test_rule_file(temp_dir.path(), "rule3.toml", "rule-3");

        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_regex_rules(temp_dir.path());
        assert!(result.is_ok());
        assert_eq!(registry.len(), 3);

        assert!(registry.get_rule(&RuleId::new("rule-1").unwrap()).is_some());
        assert!(registry.get_rule(&RuleId::new("rule-2").unwrap()).is_some());
        assert!(registry.get_rule(&RuleId::new("rule-3").unwrap()).is_some());
    }

    #[test]
    fn test_load_duplicate_rule_id() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule1.toml", "duplicate-rule");
        create_test_rule_file(temp_dir.path(), "rule2.toml", "duplicate-rule");

        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_regex_rules(temp_dir.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Duplicate rule ID")
        );
    }

    #[test]
    fn test_load_ignores_non_toml_files() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule.toml", "valid-rule");

        // Create non-TOML files
        fs::write(temp_dir.path().join("readme.md"), "# Readme").unwrap();
        fs::write(temp_dir.path().join("data.json"), "{}").unwrap();
        fs::write(temp_dir.path().join("script.sh"), "#!/bin/bash").unwrap();

        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_regex_rules(temp_dir.path());
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1); // Only the .toml file should be loaded
    }

    #[test]
    fn test_load_ignores_subdirectories() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule.toml", "root-rule");

        // Create subdirectory with a rule file
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        create_test_rule_file(&subdir, "subrule.toml", "sub-rule");

        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_regex_rules(temp_dir.path());
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1); // Should not recurse into subdirectories
    }

    #[test]
    fn test_get_rule_existing() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "test.toml", "test-rule");

        let mut registry = RuleRegistry::new();
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();

        let rule_id = RuleId::new("test-rule").unwrap();
        let rule = registry.get_rule(&rule_id);
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().id(), &rule_id);
    }

    #[test]
    fn test_get_rule_nonexistent() {
        let registry = RuleRegistry::new();
        let rule_id = RuleId::new("nonexistent").unwrap();
        assert!(registry.get_rule(&rule_id).is_none());
    }

    #[test]
    fn test_iter_rules_empty() {
        let registry = RuleRegistry::new();
        let count = registry.iter_rules().count();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_iter_rules_multiple() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule1.toml", "rule-1");
        create_test_rule_file(temp_dir.path(), "rule2.toml", "rule-2");
        create_test_rule_file(temp_dir.path(), "rule3.toml", "rule-3");

        let mut registry = RuleRegistry::new();
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();

        let count = registry.iter_rules().count();
        assert_eq!(count, 3);

        // Verify we can iterate and access rule properties
        let rule_ids: Vec<String> = registry
            .iter_rules()
            .map(|rule| rule.id().as_str().to_string())
            .collect();
        assert!(rule_ids.contains(&"rule-1".to_string()));
        assert!(rule_ids.contains(&"rule-2".to_string()));
        assert!(rule_ids.contains(&"rule-3".to_string()));
    }

    #[test]
    fn test_filter_by_config_no_config() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule1.toml", "rule-1");
        create_test_rule_file(temp_dir.path(), "rule2.toml", "rule-2");

        let mut registry = RuleRegistry::new();
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();

        let config = RulesConfig::default();
        registry.filter_by_config(&config);

        // All rules should remain (enabled by default)
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_filter_by_config_explicitly_enabled() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule1.toml", "rule-1");

        let mut registry = RuleRegistry::new();
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();

        let mut config = RulesConfig::default();
        config
            .builtin
            .insert(RuleId::new("rule-1").unwrap(), RuleValue::Enabled(true));
        registry.filter_by_config(&config);

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_filter_by_config_explicitly_disabled() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule1.toml", "rule-1");
        create_test_rule_file(temp_dir.path(), "rule2.toml", "rule-2");

        let mut registry = RuleRegistry::new();
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();

        let mut config = RulesConfig::default();
        config
            .builtin
            .insert(RuleId::new("rule-1").unwrap(), RuleValue::Enabled(false));
        registry.filter_by_config(&config);

        // rule-1 should be removed, rule-2 should remain
        assert_eq!(registry.len(), 1);
        assert!(registry.get_rule(&RuleId::new("rule-1").unwrap()).is_none());
        assert!(registry.get_rule(&RuleId::new("rule-2").unwrap()).is_some());
    }

    #[test]
    fn test_filter_by_config_with_settings() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule1.toml", "rule-1");

        let mut registry = RuleRegistry::new();
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();

        let mut config = RulesConfig::default();
        config.builtin.insert(
            RuleId::new("rule-1").unwrap(),
            RuleValue::Settings(crate::config::ratchet_toml::RuleSettings {
                severity: Some(crate::types::Severity::Error),
                regions: None,
            }),
        );
        registry.filter_by_config(&config);

        // Rule with settings should be enabled
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_filter_by_config_custom_rules() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "custom1.toml", "custom-1");
        create_test_rule_file(temp_dir.path(), "custom2.toml", "custom-2");

        let mut registry = RuleRegistry::new();
        registry.load_custom_regex_rules(temp_dir.path()).unwrap();

        let mut config = RulesConfig::default();
        config
            .custom
            .insert(RuleId::new("custom-1").unwrap(), RuleValue::Enabled(false));
        registry.filter_by_config(&config);

        // custom-1 should be removed, custom-2 should remain
        assert_eq!(registry.len(), 1);
        assert!(
            registry
                .get_rule(&RuleId::new("custom-1").unwrap())
                .is_none()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("custom-2").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_filter_by_config_mixed_rules() {
        let builtin_dir = TempDir::new().unwrap();
        let custom_dir = TempDir::new().unwrap();

        create_test_rule_file(builtin_dir.path(), "builtin.toml", "builtin-rule");
        create_test_rule_file(custom_dir.path(), "custom.toml", "custom-rule");

        let mut registry = RuleRegistry::new();
        registry
            .load_builtin_regex_rules(builtin_dir.path())
            .unwrap();
        registry.load_custom_regex_rules(custom_dir.path()).unwrap();

        let mut config = RulesConfig::default();
        config.builtin.insert(
            RuleId::new("builtin-rule").unwrap(),
            RuleValue::Enabled(false),
        );
        registry.filter_by_config(&config);

        // builtin-rule should be removed, custom-rule should remain
        assert_eq!(registry.len(), 1);
        assert!(
            registry
                .get_rule(&RuleId::new("builtin-rule").unwrap())
                .is_none()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("custom-rule").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_is_rule_enabled() {
        assert!(is_rule_enabled(&RuleValue::Enabled(true)));
        assert!(!is_rule_enabled(&RuleValue::Enabled(false)));

        let settings = crate::config::ratchet_toml::RuleSettings {
            severity: Some(crate::types::Severity::Warning),
            regions: None,
        };
        assert!(is_rule_enabled(&RuleValue::Settings(settings)));
    }

    #[test]
    fn test_load_both_builtin_and_custom() {
        let builtin_dir = TempDir::new().unwrap();
        let custom_dir = TempDir::new().unwrap();

        create_test_rule_file(builtin_dir.path(), "builtin1.toml", "builtin-1");
        create_test_rule_file(builtin_dir.path(), "builtin2.toml", "builtin-2");
        create_test_rule_file(custom_dir.path(), "custom1.toml", "custom-1");

        let mut registry = RuleRegistry::new();
        registry
            .load_builtin_regex_rules(builtin_dir.path())
            .unwrap();
        registry.load_custom_regex_rules(custom_dir.path()).unwrap();

        assert_eq!(registry.len(), 3);
        assert!(
            registry
                .get_rule(&RuleId::new("builtin-1").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("builtin-2").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("custom-1").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut registry = RuleRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        // Load one rule
        let temp_dir1 = TempDir::new().unwrap();
        create_test_rule_file(temp_dir1.path(), "rule1.toml", "rule-1");
        registry.load_builtin_regex_rules(temp_dir1.path()).unwrap();

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        // Load multiple rules from a different directory
        let temp_dir2 = TempDir::new().unwrap();
        create_test_rule_file(temp_dir2.path(), "rule2.toml", "rule-2");
        create_test_rule_file(temp_dir2.path(), "rule3.toml", "rule-3");
        registry.load_builtin_regex_rules(temp_dir2.path()).unwrap();

        assert_eq!(registry.len(), 3);
    }

    #[test]
    #[ignore] // Only run manually - depends on project structure
    fn test_load_actual_builtin_rules() {
        use std::path::PathBuf;

        let mut registry = RuleRegistry::new();
        let builtin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("builtin-ratchets")
            .join("regex");

        if builtin_dir.exists() {
            let result = registry.load_builtin_regex_rules(&builtin_dir);
            assert!(
                result.is_ok(),
                "Failed to load built-in rules: {:?}",
                result
            );

            // We should have at least the two built-in rules we know exist
            assert!(registry.len() >= 2);

            // Verify specific rules exist
            assert!(
                registry
                    .get_rule(&RuleId::new("no-todo-comments").unwrap())
                    .is_some()
            );
            assert!(
                registry
                    .get_rule(&RuleId::new("no-fixme-comments").unwrap())
                    .is_some()
            );
        }
    }
}
