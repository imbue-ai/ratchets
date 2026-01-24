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
use crate::rules::{AstRule, RegexRule, Rule, RuleContext};
use crate::types::{GlobPattern, RuleId};
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

    /// Load built-in regex rules from embedded resources
    ///
    /// This method loads built-in regex rules that are embedded in the binary
    /// at compile time. This ensures the binary is self-contained.
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if:
    /// - A TOML file cannot be parsed
    /// - A rule definition is invalid
    pub fn load_embedded_builtin_regex_rules(&mut self) -> Result<(), RuleError> {
        let rules = crate::rules::load_builtin_regex_rules()?;

        for (rule_id, rule) in rules {
            // Check for duplicate rule IDs
            if self.rules.contains_key(&rule_id) {
                return Err(RuleError::InvalidDefinition(format!(
                    "Duplicate rule ID '{}' in embedded builtin rules",
                    rule_id.as_str(),
                )));
            }

            self.rules.insert(rule_id, rule);
        }

        Ok(())
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
        // Built-in rules don't use pattern references, so we pass None
        // Don't warn on override - filesystem builtins overriding embedded is expected
        self.load_regex_rules_from_dir(builtin_dir, None, false)
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
    /// * `ctx` - Optional pattern context for resolving pattern references
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if:
    /// - A TOML file cannot be parsed
    /// - A rule definition is invalid
    /// - There is an I/O error reading a file
    pub fn load_custom_regex_rules(
        &mut self,
        custom_dir: &Path,
        ctx: Option<&RuleContext>,
    ) -> Result<(), RuleError> {
        // Warn on override - custom rules overriding builtins is unexpected
        self.load_regex_rules_from_dir(custom_dir, ctx, true)
    }

    /// Internal helper to load regex rules from a directory
    ///
    /// Scans for .toml files and loads them as RegexRules.
    /// If a rule with the same ID already exists, it will be replaced (allowing overrides).
    ///
    /// # Arguments
    ///
    /// * `dir` - Directory to load rules from
    /// * `ctx` - Optional pattern context for resolving pattern references
    /// * `warn_on_override` - Whether to warn when overriding an existing rule
    fn load_regex_rules_from_dir(
        &mut self,
        dir: &Path,
        ctx: Option<&RuleContext>,
        warn_on_override: bool,
    ) -> Result<(), RuleError> {
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
            let content = std::fs::read_to_string(&path).map_err(|e| {
                RuleError::InvalidDefinition(format!("Failed to read file {:?}: {}", path, e))
            })?;
            let rule = RegexRule::from_toml_with_context(&content, ctx)?;
            let rule_id = rule.id().clone();

            // Allow overriding existing rules (for filesystem to override embedded)
            // Warn if requested (e.g., when custom rules override builtins)
            if warn_on_override && self.rules.contains_key(&rule_id) {
                eprintln!(
                    "Warning: Overriding rule '{}' with version from {}",
                    rule_id.as_str(),
                    path.display()
                );
            }

            // Add/replace rule in registry (HashMap insert replaces existing key)
            self.rules.insert(rule_id, Box::new(rule));
        }

        Ok(())
    }

    /// Load built-in AST rules from embedded resources
    ///
    /// This method loads built-in AST rules that are embedded in the binary
    /// at compile time. This ensures the binary is self-contained.
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if:
    /// - A TOML file cannot be parsed
    /// - A rule definition is invalid
    /// - A tree-sitter query is invalid
    pub fn load_embedded_builtin_ast_rules(&mut self) -> Result<(), RuleError> {
        let rules = crate::rules::load_builtin_ast_rules()?;

        for (rule_id, rule) in rules {
            // Check for duplicate rule IDs (both within AST rules and with regex rules)
            if self.rules.contains_key(&rule_id) {
                return Err(RuleError::InvalidDefinition(format!(
                    "Duplicate rule ID '{}' in embedded builtin AST rules",
                    rule_id.as_str(),
                )));
            }

            self.rules.insert(rule_id, rule);
        }

        Ok(())
    }

    /// Load built-in AST rules from a directory
    ///
    /// This method scans the specified directory for language subdirectories
    /// (e.g., builtin-ratchets/rust/ast/, builtin-ratchets/python/ast/) and
    /// loads all `.toml` files as AstRules. If the directory doesn't exist,
    /// a warning is logged but the operation succeeds.
    ///
    /// # Arguments
    ///
    /// * `builtin_dir` - Path to the builtin-ratchets/{language}/ast/ directory
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if:
    /// - A TOML file cannot be parsed
    /// - A rule definition is invalid
    /// - A tree-sitter query is invalid
    /// - There is an I/O error reading a file
    pub fn load_builtin_ast_rules(&mut self, builtin_dir: &Path) -> Result<(), RuleError> {
        // Check if directory exists
        if !builtin_dir.exists() {
            // Log warning but don't fail - missing directories are OK
            eprintln!(
                "Warning: AST rule directory does not exist: {}",
                builtin_dir.display()
            );
            return Ok(());
        }

        if !builtin_dir.is_dir() {
            return Err(RuleError::InvalidDefinition(format!(
                "Path is not a directory: {}",
                builtin_dir.display()
            )));
        }

        // Create a default pattern context for builtin rules
        let mut patterns = HashMap::new();

        // Define python_tests pattern for Python AST rules
        #[cfg(feature = "lang-python")]
        {
            patterns.insert(
                "python_tests".to_string(),
                vec![
                    GlobPattern::new("**/test_*.py".to_string()),
                    GlobPattern::new("**/*_test.py".to_string()),
                    GlobPattern::new("**/tests/**".to_string()),
                ],
            );
        }

        let rule_context = RuleContext { patterns };

        // Read all entries in the directory (these should be language directories like rust/, python/, etc.)
        let entries = fs::read_dir(builtin_dir).map_err(|e| {
            RuleError::InvalidDefinition(format!(
                "Failed to read directory {}: {}",
                builtin_dir.display(),
                e
            ))
        })?;

        // Process each language directory
        for entry in entries {
            let entry = entry.map_err(|e| {
                RuleError::InvalidDefinition(format!(
                    "Failed to read directory entry in {}: {}",
                    builtin_dir.display(),
                    e
                ))
            })?;

            let lang_path = entry.path();

            // Only process subdirectories
            if !lang_path.is_dir() {
                continue;
            }

            // Look for an ast/ subdirectory within the language directory
            let ast_path = lang_path.join("ast");
            if ast_path.exists() && ast_path.is_dir() {
                // Load all AST rules from this language's ast subdirectory
                // Don't warn on override - filesystem builtins overriding embedded is expected
                self.load_ast_rules_from_dir(&ast_path, Some(&rule_context), false)?;
            }
        }

        Ok(())
    }

    /// Load custom AST rules from a directory
    ///
    /// This method scans the specified directory for `.toml` files and attempts
    /// to load each as an AstRule. Custom AST rules are stored in a flat
    /// directory structure (ratchets/ast/*.toml), not per-language subdirectories.
    /// If the directory doesn't exist, a warning is logged but the operation succeeds.
    ///
    /// # Arguments
    ///
    /// * `custom_dir` - Path to the ratchets/ast/ directory
    /// * `ctx` - Optional pattern context for resolving pattern references
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if:
    /// - A TOML file cannot be parsed
    /// - A rule definition is invalid
    /// - A tree-sitter query is invalid
    /// - There is an I/O error reading a file
    pub fn load_custom_ast_rules(
        &mut self,
        custom_dir: &Path,
        ctx: Option<&RuleContext>,
    ) -> Result<(), RuleError> {
        // Warn on override - custom rules overriding builtins is unexpected
        self.load_ast_rules_from_dir(custom_dir, ctx, true)
    }

    /// Internal helper to load AST rules from a directory
    ///
    /// Scans for .toml files and loads them as AstRules.
    /// If a rule with the same ID already exists, it will be replaced (allowing overrides).
    ///
    /// # Arguments
    ///
    /// * `dir` - Directory to load rules from
    /// * `ctx` - Optional pattern context for resolving pattern references
    /// * `warn_on_override` - Whether to warn when overriding an existing rule
    fn load_ast_rules_from_dir(
        &mut self,
        dir: &Path,
        ctx: Option<&RuleContext>,
        warn_on_override: bool,
    ) -> Result<(), RuleError> {
        // Check if directory exists
        if !dir.exists() {
            // Log warning but don't fail - missing directories are OK
            eprintln!(
                "Warning: AST rule directory does not exist: {}",
                dir.display()
            );
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
            let content = std::fs::read_to_string(&path).map_err(|e| {
                RuleError::InvalidDefinition(format!("Failed to read file {:?}: {}", path, e))
            })?;
            let rule = AstRule::from_toml_with_context(&content, ctx)?;
            let rule_id = rule.id().clone();

            // Allow overriding existing rules (for filesystem to override embedded)
            // Warn if requested (e.g., when custom rules override builtins)
            if warn_on_override && self.rules.contains_key(&rule_id) {
                eprintln!(
                    "Warning: Overriding rule '{}' with version from {}",
                    rule_id.as_str(),
                    path.display()
                );
            }

            // Add/replace rule in registry (HashMap insert replaces existing key)
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

    /// Filter rules to only keep those matching the configured languages.
    /// Rules with no language restriction are always kept.
    ///
    /// # Arguments
    ///
    /// * `languages` - The list of languages to filter by
    pub fn filter_by_languages(&mut self, languages: &[crate::types::Language]) {
        // If no languages specified, keep all rules
        if languages.is_empty() {
            return;
        }

        // Collect rule IDs to remove
        let to_remove: Vec<RuleId> = self
            .rules
            .iter()
            .filter_map(|(id, rule)| {
                let rule_langs = rule.languages();
                // Keep if rule has no language restriction
                if rule_langs.is_empty() {
                    return None;
                }
                // Keep if any of the rule's languages are in the config
                if rule_langs.iter().any(|l| languages.contains(l)) {
                    return None;
                }
                // Otherwise, remove
                Some(id.clone())
            })
            .collect();

        // Remove filtered rules
        for id in to_remove {
            self.rules.remove(&id);
        }
    }

    /// Build a fully configured rule registry from the given config.
    ///
    /// This is the ONLY function that should be used to create a rule registry
    /// for normal operation. It loads rules in the correct order:
    /// 1. Embedded builtin rules (compiled into binary)
    /// 2. Filesystem builtin rules (from builtin-ratchets/ - for overrides/development)
    /// 3. Custom rules (from ratchets/ - user-defined rules)
    /// 4. Filters by config (removes disabled rules)
    /// 5. Filters by language (removes rules for unconfigured languages)
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration containing patterns and rule settings
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if any rule loading step fails
    pub fn build_from_config(
        config: &crate::config::ratchet_toml::Config,
    ) -> Result<Self, RuleError> {
        let mut registry = Self::new();

        // Create RuleContext from config patterns
        let rule_context = RuleContext::new(config.patterns.clone());

        // Step 1: Load embedded builtin rules (always available)
        registry.load_embedded_builtin_regex_rules()?;
        registry.load_embedded_builtin_ast_rules()?;

        // Step 2: Load filesystem builtin rules (for overrides or development)
        // These silently override embedded rules if present
        let builtin_regex_dir = std::path::PathBuf::from("builtin-ratchets")
            .join("common")
            .join("regex");
        if builtin_regex_dir.exists() {
            registry.load_builtin_regex_rules(&builtin_regex_dir)?;
        }

        let builtin_ratchets_dir = std::path::PathBuf::from("builtin-ratchets");
        if builtin_ratchets_dir.exists() {
            registry.load_builtin_ast_rules(&builtin_ratchets_dir)?;
        }

        // Step 3: Load custom rules (user-defined)
        // These warn if they override builtin rules
        let custom_regex_dir = std::path::PathBuf::from("ratchets").join("regex");
        if custom_regex_dir.exists() {
            registry.load_custom_regex_rules(&custom_regex_dir, Some(&rule_context))?;
        }

        let custom_ast_dir = std::path::PathBuf::from("ratchets").join("ast");
        if custom_ast_dir.exists() {
            registry.load_custom_ast_rules(&custom_ast_dir, Some(&rule_context))?;
        }

        // Step 4: Filter by config (remove disabled rules)
        registry.filter_by_config(&config.rules);

        // Step 5: Filter by language (remove rules for unconfigured languages)
        registry.filter_by_languages(&config.ratchet.languages);

        Ok(registry)
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
        let result = registry.load_custom_regex_rules(Path::new("/nonexistent/path"), None);
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
        // Should succeed now - later rules override earlier ones
        assert!(result.is_ok());
        // Should have exactly 1 rule (the second one overrode the first)
        assert_eq!(registry.len(), 1);
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
        registry
            .load_custom_regex_rules(temp_dir.path(), None)
            .unwrap();

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
        registry
            .load_custom_regex_rules(custom_dir.path(), None)
            .unwrap();

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
        registry
            .load_custom_regex_rules(custom_dir.path(), None)
            .unwrap();

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

    // AST Rule Loading Tests

    // Helper to create a test AST rule TOML file
    fn create_test_ast_rule_file(dir: &Path, filename: &str, rule_id: &str) -> PathBuf {
        let toml_content = format!(
            r#"
[rule]
id = "{}"
description = "Test AST rule"
severity = "error"

[match]
language = "rust"
query = "(identifier) @violation"
"#,
            rule_id
        );

        let file_path = dir.join(filename);
        fs::write(&file_path, toml_content).unwrap();
        file_path
    }

    #[test]
    fn test_load_builtin_ast_rules_missing_dir() {
        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_ast_rules(Path::new("/nonexistent/ast/path"));
        assert!(result.is_ok()); // Should succeed with warning
        assert!(registry.is_empty());
    }

    #[test]
    fn test_load_custom_ast_rules_missing_dir() {
        let mut registry = RuleRegistry::new();
        let result = registry.load_custom_ast_rules(Path::new("/nonexistent/ast/path"), None);
        assert!(result.is_ok()); // Should succeed with warning
        assert!(registry.is_empty());
    }

    #[test]
    fn test_load_single_ast_rule() {
        let temp_dir = TempDir::new().unwrap();
        create_test_ast_rule_file(temp_dir.path(), "test-ast-rule.toml", "test-ast-rule");

        let mut registry = RuleRegistry::new();
        let result = registry.load_custom_ast_rules(temp_dir.path(), None);
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1);

        let rule_id = RuleId::new("test-ast-rule").unwrap();
        assert!(registry.get_rule(&rule_id).is_some());
    }

    #[test]
    fn test_load_multiple_ast_rules() {
        let temp_dir = TempDir::new().unwrap();
        create_test_ast_rule_file(temp_dir.path(), "rule1.toml", "ast-rule-1");
        create_test_ast_rule_file(temp_dir.path(), "rule2.toml", "ast-rule-2");
        create_test_ast_rule_file(temp_dir.path(), "rule3.toml", "ast-rule-3");

        let mut registry = RuleRegistry::new();
        let result = registry.load_custom_ast_rules(temp_dir.path(), None);
        assert!(result.is_ok());
        assert_eq!(registry.len(), 3);

        assert!(
            registry
                .get_rule(&RuleId::new("ast-rule-1").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("ast-rule-2").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("ast-rule-3").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_load_builtin_ast_rules_per_language() {
        let temp_dir = TempDir::new().unwrap();

        // Create language subdirectories with ast/ subdirectories
        let rust_ast_dir = temp_dir.path().join("rust").join("ast");
        let python_ast_dir = temp_dir.path().join("python").join("ast");
        fs::create_dir_all(&rust_ast_dir).unwrap();
        fs::create_dir_all(&python_ast_dir).unwrap();

        // Create rules in each language's ast directory
        create_test_ast_rule_file(&rust_ast_dir, "rust-rule.toml", "rust-rule");
        create_test_ast_rule_file(&python_ast_dir, "python-rule.toml", "python-rule");

        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_ast_rules(temp_dir.path());
        assert!(result.is_ok());
        assert_eq!(registry.len(), 2);

        assert!(
            registry
                .get_rule(&RuleId::new("rust-rule").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("python-rule").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_load_builtin_ast_rules_ignores_files_in_root() {
        let temp_dir = TempDir::new().unwrap();

        // Create a rule file in the root (should be ignored)
        create_test_ast_rule_file(temp_dir.path(), "root-rule.toml", "root-rule");

        // Create language subdirectory with ast/ subdirectory and a rule
        let rust_ast_dir = temp_dir.path().join("rust").join("ast");
        fs::create_dir_all(&rust_ast_dir).unwrap();
        create_test_ast_rule_file(&rust_ast_dir, "rust-rule.toml", "rust-rule");

        let mut registry = RuleRegistry::new();
        let result = registry.load_builtin_ast_rules(temp_dir.path());
        assert!(result.is_ok());

        // Should only load from language/ast/ subdirectories, not root files
        assert_eq!(registry.len(), 1);
        assert!(
            registry
                .get_rule(&RuleId::new("rust-rule").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("root-rule").unwrap())
                .is_none()
        );
    }

    #[test]
    fn test_load_ast_rule_duplicate_id() {
        let temp_dir = TempDir::new().unwrap();
        create_test_ast_rule_file(temp_dir.path(), "rule1.toml", "duplicate-ast-rule");
        create_test_ast_rule_file(temp_dir.path(), "rule2.toml", "duplicate-ast-rule");

        let mut registry = RuleRegistry::new();
        let result = registry.load_custom_ast_rules(temp_dir.path(), None);
        // Should succeed now - later rules override earlier ones
        assert!(result.is_ok());
        // Should have exactly 1 rule (the second one overrode the first)
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_load_ast_and_regex_rules_duplicate_id() {
        let regex_dir = TempDir::new().unwrap();
        let ast_dir = TempDir::new().unwrap();

        // Create rules with the same ID in both directories
        create_test_rule_file(regex_dir.path(), "rule.toml", "shared-rule");
        create_test_ast_rule_file(ast_dir.path(), "rule.toml", "shared-rule");

        let mut registry = RuleRegistry::new();

        // Load regex rule first
        let result = registry.load_builtin_regex_rules(regex_dir.path());
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1);

        // Load AST rule with same ID - should succeed and override
        let result = registry.load_custom_ast_rules(ast_dir.path(), None);
        assert!(result.is_ok());
        // Should still have exactly 1 rule (AST rule overrode regex rule)
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_load_ast_rules_invalid_query() {
        let temp_dir = TempDir::new().unwrap();

        // Create an AST rule with invalid query syntax
        let toml_content = r#"
[rule]
id = "bad-query"
description = "Rule with invalid query"
severity = "error"

[match]
language = "rust"
query = "(unclosed_paren"
"#;
        let file_path = temp_dir.path().join("bad.toml");
        fs::write(&file_path, toml_content).unwrap();

        let mut registry = RuleRegistry::new();
        let result = registry.load_custom_ast_rules(temp_dir.path(), None);

        // Should fail due to invalid query
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RuleError::InvalidQuery(_)));
    }

    #[test]
    fn test_load_both_regex_and_ast_rules() {
        let regex_dir = TempDir::new().unwrap();
        let ast_dir = TempDir::new().unwrap();

        create_test_rule_file(regex_dir.path(), "regex1.toml", "regex-1");
        create_test_rule_file(regex_dir.path(), "regex2.toml", "regex-2");
        create_test_ast_rule_file(ast_dir.path(), "ast1.toml", "ast-1");
        create_test_ast_rule_file(ast_dir.path(), "ast2.toml", "ast-2");

        let mut registry = RuleRegistry::new();
        registry.load_builtin_regex_rules(regex_dir.path()).unwrap();
        registry
            .load_custom_ast_rules(ast_dir.path(), None)
            .unwrap();

        assert_eq!(registry.len(), 4);
        assert!(
            registry
                .get_rule(&RuleId::new("regex-1").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("regex-2").unwrap())
                .is_some()
        );
        assert!(registry.get_rule(&RuleId::new("ast-1").unwrap()).is_some());
        assert!(registry.get_rule(&RuleId::new("ast-2").unwrap()).is_some());
    }

    #[test]
    fn test_load_ast_rules_ignores_non_toml_files() {
        let temp_dir = TempDir::new().unwrap();
        create_test_ast_rule_file(temp_dir.path(), "valid.toml", "valid-ast-rule");

        // Create non-TOML files
        fs::write(temp_dir.path().join("readme.md"), "# Readme").unwrap();
        fs::write(temp_dir.path().join("data.json"), "{}").unwrap();

        let mut registry = RuleRegistry::new();
        let result = registry.load_custom_ast_rules(temp_dir.path(), None);
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1); // Only the .toml file should be loaded
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    #[ignore] // Only run manually - depends on project structure
    fn test_load_actual_builtin_ast_rules() {
        use std::path::PathBuf;

        let mut registry = RuleRegistry::new();
        let builtin_ast_rust_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("builtin-ratchets")
            .join("ast")
            .join("rust");

        if builtin_ast_rust_dir.exists() {
            // Load only Rust AST rules to avoid issues with other languages
            let result = registry.load_custom_ast_rules(&builtin_ast_rust_dir, None);
            assert!(
                result.is_ok(),
                "Failed to load built-in Rust AST rules: {:?}",
                result
            );

            // We should have the three Rust built-in AST rules
            assert_eq!(registry.len(), 3);

            // Verify specific Rust AST rules exist
            assert!(
                registry
                    .get_rule(&RuleId::new("no-unwrap").unwrap())
                    .is_some(),
                "no-unwrap rule not found"
            );
            assert!(
                registry
                    .get_rule(&RuleId::new("no-expect").unwrap())
                    .is_some(),
                "no-expect rule not found"
            );
            assert!(
                registry
                    .get_rule(&RuleId::new("no-panic").unwrap())
                    .is_some(),
                "no-panic rule not found"
            );
        }
    }

    #[test]
    fn test_no_duplicate_rule_ids_after_override() {
        // Test that when a filesystem rule overrides an embedded rule,
        // there's exactly one rule with that ID in the registry
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "override-rule.toml", "test-override");

        let mut registry = RuleRegistry::new();

        // Load the rule once
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();
        assert_eq!(registry.len(), 1);

        // Load again with same rule ID (simulating filesystem override of embedded)
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();

        // Should still have exactly 1 rule, not 2
        assert_eq!(registry.len(), 1);

        // Verify the rule is accessible
        let rule_id = RuleId::new("test-override").unwrap();
        assert!(registry.get_rule(&rule_id).is_some());
    }

    #[test]
    fn test_override_warnings_controlled_by_parameter() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "warning-test.toml", "warning-test");

        let mut registry = RuleRegistry::new();

        // Load builtin (no warning expected since it's first load)
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();
        assert_eq!(registry.len(), 1);

        // Load builtin again (no warning expected - warn_on_override=false)
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();
        assert_eq!(registry.len(), 1);

        // Load custom (warning expected - warn_on_override=true, but we can't easily test stderr)
        registry
            .load_custom_regex_rules(temp_dir.path(), None)
            .unwrap();
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_complete_loading_sequence_no_duplicates() {
        // Simulate the complete loading sequence from build_rule_registry in check.rs
        // This tests that embedded -> filesystem builtin -> custom loading produces
        // exactly one rule per ID

        let builtin_dir = TempDir::new().unwrap();
        let custom_dir = TempDir::new().unwrap();

        // Create a rule in both builtin and custom dirs with the same ID
        create_test_rule_file(builtin_dir.path(), "shared-rule.toml", "shared-rule");
        create_test_rule_file(custom_dir.path(), "shared-rule.toml", "shared-rule");

        // Also create unique rules in each directory
        create_test_rule_file(builtin_dir.path(), "builtin-only.toml", "builtin-only");
        create_test_rule_file(custom_dir.path(), "custom-only.toml", "custom-only");

        let mut registry = RuleRegistry::new();

        // Step 1: Load embedded builtin rules (simulated by loading from builtin_dir first)
        registry
            .load_builtin_regex_rules(builtin_dir.path())
            .unwrap();
        assert_eq!(registry.len(), 2); // shared-rule and builtin-only

        // Step 2: Load filesystem builtin rules (same directory, should not duplicate)
        registry
            .load_builtin_regex_rules(builtin_dir.path())
            .unwrap();
        assert_eq!(registry.len(), 2); // Still 2, no duplicates

        // Step 3: Load custom rules (may override builtin)
        registry
            .load_custom_regex_rules(custom_dir.path(), None)
            .unwrap();
        assert_eq!(registry.len(), 3); // shared-rule (overridden), builtin-only, custom-only

        // Verify all rules are accessible
        assert!(
            registry
                .get_rule(&RuleId::new("shared-rule").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("builtin-only").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get_rule(&RuleId::new("custom-only").unwrap())
                .is_some()
        );

        // Verify no duplicate IDs by checking that iter_rules count equals len
        let iter_count = registry.iter_rules().count();
        assert_eq!(iter_count, registry.len());
    }

    #[test]
    #[cfg(feature = "lang-rust")]
    fn test_build_from_config_loads_embedded_rules() {
        use crate::config::ratchet_toml::{Config, OutputConfig, RatchetMeta, RulesConfig};
        use crate::types::GlobPattern;
        use std::collections::HashMap;

        // Create a minimal config (without requiring ratchet.toml file)
        let config = Config {
            ratchet: RatchetMeta {
                version: "1".to_string(),
                languages: vec![crate::types::Language::Rust],
                include: vec![GlobPattern::new("**/*.rs".to_string())],
                exclude: vec![],
            },
            rules: RulesConfig {
                builtin: HashMap::new(),
                custom: HashMap::new(),
            },
            output: OutputConfig::default(),
            patterns: HashMap::new(),
        };

        // Build registry from config
        let registry = RuleRegistry::build_from_config(&config).unwrap();

        // Should have loaded embedded builtin rules
        // At minimum, we should have the Rust AST rules: no-unwrap, no-panic, no-expect
        assert!(registry.len() >= 3);

        // Verify specific embedded rules are present
        let no_unwrap = RuleId::new("no-unwrap").unwrap();
        let no_panic = RuleId::new("no-panic").unwrap();
        let no_expect = RuleId::new("no-expect").unwrap();

        assert!(
            registry.get_rule(&no_unwrap).is_some(),
            "no-unwrap rule should be loaded from embedded rules"
        );
        assert!(
            registry.get_rule(&no_panic).is_some(),
            "no-panic rule should be loaded from embedded rules"
        );
        assert!(
            registry.get_rule(&no_expect).is_some(),
            "no-expect rule should be loaded from embedded rules"
        );
    }

    #[test]
    fn test_build_from_config_respects_disabled_rules() {
        use crate::config::ratchet_toml::{
            Config, OutputConfig, RatchetMeta, RuleValue, RulesConfig,
        };
        use crate::types::GlobPattern;
        use std::collections::HashMap;

        // Create config with a disabled rule
        let mut builtin_rules = HashMap::new();
        builtin_rules.insert(
            RuleId::new("no-todo-comments").unwrap(),
            RuleValue::Enabled(false),
        );

        let config = Config {
            ratchet: RatchetMeta {
                version: "1".to_string(),
                languages: vec![crate::types::Language::Rust],
                include: vec![GlobPattern::new("**/*".to_string())],
                exclude: vec![],
            },
            rules: RulesConfig {
                builtin: builtin_rules,
                custom: HashMap::new(),
            },
            output: OutputConfig::default(),
            patterns: HashMap::new(),
        };

        // Build registry from config
        let registry = RuleRegistry::build_from_config(&config).unwrap();

        // The no-todo-comments rule should be filtered out
        let no_todo_comments = RuleId::new("no-todo-comments").unwrap();
        assert!(
            registry.get_rule(&no_todo_comments).is_none(),
            "no-todo-comments should be filtered out when disabled in config"
        );
    }

    #[test]
    fn test_filter_by_languages_empty_languages() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rule_file(temp_dir.path(), "rule1.toml", "rule-1");
        create_test_rule_file(temp_dir.path(), "rule2.toml", "rule-2");

        let mut registry = RuleRegistry::new();
        registry.load_builtin_regex_rules(temp_dir.path()).unwrap();
        assert_eq!(registry.len(), 2);

        // Empty languages list should keep all rules
        registry.filter_by_languages(&[]);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    #[cfg(feature = "lang-rust")]
    fn test_filter_by_languages_removes_non_matching_rules() {
        use crate::config::ratchet_toml::{Config, OutputConfig, RatchetMeta, RulesConfig};
        use crate::types::{GlobPattern, Language};
        use std::collections::HashMap;

        // Create config with only Rust language
        let config = Config {
            ratchet: RatchetMeta {
                version: "1".to_string(),
                languages: vec![Language::Rust],
                include: vec![GlobPattern::new("**/*".to_string())],
                exclude: vec![],
            },
            rules: RulesConfig {
                builtin: HashMap::new(),
                custom: HashMap::new(),
            },
            output: OutputConfig::default(),
            patterns: HashMap::new(),
        };

        // Build registry from config
        let registry = RuleRegistry::build_from_config(&config).unwrap();

        // Verify that only Rust rules and language-agnostic rules are present
        for rule in registry.iter_rules() {
            let rule_langs = rule.languages();
            // Rule should either have no languages (language-agnostic)
            // or include Rust
            assert!(
                rule_langs.is_empty() || rule_langs.contains(&Language::Rust),
                "Rule '{}' with languages {:?} should not be present when only Rust is configured",
                rule.id().as_str(),
                rule_langs
            );
        }

        // Verify specific Rust rules are present
        let no_unwrap = RuleId::new("no-unwrap").unwrap();
        let no_panic = RuleId::new("no-panic").unwrap();
        let no_expect = RuleId::new("no-expect").unwrap();

        assert!(
            registry.get_rule(&no_unwrap).is_some(),
            "no-unwrap (Rust rule) should be present"
        );
        assert!(
            registry.get_rule(&no_panic).is_some(),
            "no-panic (Rust rule) should be present"
        );
        assert!(
            registry.get_rule(&no_expect).is_some(),
            "no-expect (Rust rule) should be present"
        );

        // Verify Python-specific rules are NOT present
        let no_args_in_docstrings = RuleId::new("no-args-in-docstrings").unwrap();
        assert!(
            registry.get_rule(&no_args_in_docstrings).is_none(),
            "no-args-in-docstrings (Python rule) should be filtered out"
        );

        // Verify TypeScript-specific rules are NOT present
        let no_any = RuleId::new("no-any").unwrap();
        assert!(
            registry.get_rule(&no_any).is_none(),
            "no-any (TypeScript rule) should be filtered out"
        );

        // Verify language-agnostic rules are present
        let no_todo_comments = RuleId::new("no-todo-comments").unwrap();
        let no_fixme_comments = RuleId::new("no-fixme-comments").unwrap();
        assert!(
            registry.get_rule(&no_todo_comments).is_some(),
            "no-todo-comments (language-agnostic) should be present"
        );
        assert!(
            registry.get_rule(&no_fixme_comments).is_some(),
            "no-fixme-comments (language-agnostic) should be present"
        );
    }

    #[test]
    #[cfg(all(feature = "lang-rust", feature = "lang-python"))]
    fn test_filter_by_languages_keeps_multiple_languages() {
        use crate::config::ratchet_toml::{Config, OutputConfig, RatchetMeta, RulesConfig};
        use crate::types::{GlobPattern, Language};
        use std::collections::HashMap;

        // Create config with Rust and Python languages
        let config = Config {
            ratchet: RatchetMeta {
                version: "1".to_string(),
                languages: vec![Language::Rust, Language::Python],
                include: vec![GlobPattern::new("**/*".to_string())],
                exclude: vec![],
            },
            rules: RulesConfig {
                builtin: HashMap::new(),
                custom: HashMap::new(),
            },
            output: OutputConfig::default(),
            patterns: HashMap::new(),
        };

        // Build registry from config
        let registry = RuleRegistry::build_from_config(&config).unwrap();

        // Verify Rust rules are present
        let no_unwrap = RuleId::new("no-unwrap").unwrap();
        assert!(
            registry.get_rule(&no_unwrap).is_some(),
            "no-unwrap (Rust rule) should be present"
        );

        // Verify Python rules are present
        let no_args_in_docstrings = RuleId::new("no-args-in-docstrings").unwrap();
        assert!(
            registry.get_rule(&no_args_in_docstrings).is_some(),
            "no-args-in-docstrings (Python rule) should be present"
        );

        // Verify TypeScript rules are NOT present
        let no_any = RuleId::new("no-any").unwrap();
        assert!(
            registry.get_rule(&no_any).is_none(),
            "no-any (TypeScript rule) should be filtered out"
        );
    }

    #[test]
    fn test_filter_by_languages_keeps_language_agnostic_rules() {
        use crate::config::ratchet_toml::{Config, OutputConfig, RatchetMeta, RulesConfig};
        use crate::types::{GlobPattern, Language};
        use std::collections::HashMap;

        // Create config with only Rust language
        let config = Config {
            ratchet: RatchetMeta {
                version: "1".to_string(),
                languages: vec![Language::Rust],
                include: vec![GlobPattern::new("**/*".to_string())],
                exclude: vec![],
            },
            rules: RulesConfig {
                builtin: HashMap::new(),
                custom: HashMap::new(),
            },
            output: OutputConfig::default(),
            patterns: HashMap::new(),
        };

        // Build registry from config
        let registry = RuleRegistry::build_from_config(&config).unwrap();

        // Language-agnostic rules (those with empty languages list) should always be present
        let no_todo_comments = RuleId::new("no-todo-comments").unwrap();
        let no_fixme_comments = RuleId::new("no-fixme-comments").unwrap();

        assert!(
            registry.get_rule(&no_todo_comments).is_some(),
            "no-todo-comments (language-agnostic) should always be present"
        );
        assert!(
            registry.get_rule(&no_fixme_comments).is_some(),
            "no-fixme-comments (language-agnostic) should always be present"
        );
    }
}
