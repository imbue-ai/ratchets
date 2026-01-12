//! ratchet-counts.toml parsing and management
//!
//! This module handles the violation budget tracking system. It parses
//! ratchet-counts.toml, resolves region inheritance, and provides methods
//! for querying and mutating counts.

use crate::error::ConfigError;
use crate::types::{RegionPath, RuleId};
use std::collections::HashMap;
use std::path::Path;

/// Manages violation budgets for all rules across all regions
///
/// CountsManager stores a mapping from rule IDs to their region trees,
/// which define the violation budgets for each region.
#[derive(Debug, Clone)]
pub struct CountsManager {
    counts: HashMap<RuleId, RegionTree>,
}

/// Hierarchical storage of violation counts per region for a single rule
///
/// Each rule has a RegionTree that stores:
/// - A root count (default 0) that applies to all regions unless overridden
/// - Explicit overrides for specific region paths
///
/// Regions inherit from their parent unless they have an explicit override.
#[derive(Debug, Clone)]
pub struct RegionTree {
    root_count: u64,
    overrides: HashMap<RegionPath, u64>,
}

impl RegionTree {
    /// Creates a new RegionTree with default root count of 0
    pub fn new() -> Self {
        RegionTree {
            root_count: 0,
            overrides: HashMap::new(),
        }
    }

    /// Creates a new RegionTree with a specific root count
    pub fn with_root_count(count: u64) -> Self {
        RegionTree {
            root_count: count,
            overrides: HashMap::new(),
        }
    }

    /// Gets the budget for a specific file path using inheritance
    ///
    /// Algorithm:
    /// 1. Start with the file's directory
    /// 2. Check if there's an override for that path
    /// 3. If not, go up to parent directory
    /// 4. Repeat until reaching root or finding an override
    /// 5. If no override found, return root_count (default 0)
    pub fn get_budget(&self, file_path: &Path) -> u64 {
        // Normalize the file path to a region path
        let path_str = file_path.to_string_lossy().to_string();

        // Start with the parent directory of the file
        let mut current_path = Path::new(&path_str);

        // If it's a file, get its parent directory first
        if current_path.is_file() || !current_path.ends_with("/") {
            current_path = current_path.parent().unwrap_or(Path::new("."));
        }

        loop {
            let region = RegionPath::new(current_path.to_string_lossy().to_string());

            // Check if there's an explicit override for this region
            if let Some(&count) = self.overrides.get(&region) {
                return count;
            }

            // Try to go up to the parent
            if let Some(parent) = current_path.parent() {
                if parent == Path::new("") || parent == current_path {
                    // We've reached the root
                    break;
                }
                current_path = parent;
            } else {
                // No parent, we're at the root
                break;
            }
        }

        // No override found, check if root "." is explicitly set
        if let Some(&count) = self.overrides.get(&RegionPath::new(".")) {
            return count;
        }

        // Return the root count (default 0)
        self.root_count
    }

    /// Sets the count for a specific region
    pub fn set_count(&mut self, region: &RegionPath, count: u64) {
        if region.as_str() == "." {
            self.root_count = count;
        }
        self.overrides.insert(region.clone(), count);
    }
}

impl Default for RegionTree {
    fn default() -> Self {
        Self::new()
    }
}

impl CountsManager {
    /// Creates a new empty CountsManager
    pub fn new() -> Self {
        CountsManager {
            counts: HashMap::new(),
        }
    }

    /// Parses a CountsManager from TOML string
    ///
    /// Expected format:
    /// ```toml
    /// [rule-id]
    /// "." = 0
    /// "src/legacy" = 15
    /// ```
    pub fn parse(s: &str) -> Result<Self, ConfigError> {
        // Parse the TOML into a raw map
        let parsed: toml::Value = toml::from_str(s)?;

        let mut counts = HashMap::new();

        if let toml::Value::Table(table) = parsed {
            for (rule_id_str, value) in table {
                // Validate and create RuleId
                let rule_id =
                    RuleId::new(&rule_id_str).ok_or_else(|| ConfigError::InvalidValue {
                        field: "rule-id".to_string(),
                        message: format!("Invalid rule ID: {}", rule_id_str),
                    })?;

                // Parse the region counts for this rule
                let mut tree = RegionTree::new();

                if let toml::Value::Table(regions) = value {
                    for (region_str, count_value) in regions {
                        let region = RegionPath::new(region_str);

                        let count =
                            count_value
                                .as_integer()
                                .ok_or_else(|| ConfigError::InvalidValue {
                                    field: format!("{}.{}", rule_id_str, region),
                                    message: "Count must be a non-negative integer".to_string(),
                                })?;

                        if count < 0 {
                            return Err(ConfigError::InvalidValue {
                                field: format!("{}.{}", rule_id_str, region),
                                message: "Count must be non-negative".to_string(),
                            });
                        }

                        tree.set_count(&region, count as u64);
                    }
                } else {
                    return Err(ConfigError::InvalidValue {
                        field: rule_id_str,
                        message: "Rule section must be a table of region counts".to_string(),
                    });
                }

                counts.insert(rule_id, tree);
            }
        } else {
            return Err(ConfigError::InvalidSyntax(
                "Expected a TOML table at root level".to_string(),
            ));
        }

        Ok(CountsManager { counts })
    }

    /// Loads a CountsManager from a file
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Gets the budget for a specific rule and file path
    ///
    /// Returns the budget using inheritance logic from the RegionTree.
    /// If the rule is not present in the counts, returns 0 (strict enforcement).
    pub fn get_budget(&self, rule_id: &RuleId, file_path: &Path) -> u64 {
        self.counts
            .get(rule_id)
            .map(|tree| tree.get_budget(file_path))
            .unwrap_or(0)
    }

    /// Sets the count for a specific rule and region
    pub fn set_count(&mut self, rule_id: &RuleId, region: &RegionPath, count: u64) {
        self.counts
            .entry(rule_id.clone())
            .or_default()
            .set_count(region, count);
    }

    /// Serializes the CountsManager back to TOML format
    ///
    /// Output format matches the input format:
    /// ```toml
    /// [rule-id]
    /// "." = 0
    /// "region/path" = 15
    /// ```
    pub fn to_toml_string(&self) -> String {
        let mut result = String::new();
        result.push_str("# Ratchet violation budgets\n");
        result.push_str("# These counts represent the maximum tolerated violations.\n");
        result.push_str(
            "# Counts can only be reduced (tightened) or explicitly bumped with justification.\n\n",
        );

        // Sort rule IDs for deterministic output
        let mut rule_ids: Vec<_> = self.counts.keys().collect();
        rule_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        for rule_id in rule_ids {
            let tree = &self.counts[rule_id];

            result.push_str(&format!("[{}]\n", rule_id));

            // Collect and sort regions for deterministic output
            let mut regions: Vec<_> = tree.overrides.iter().collect();
            regions.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

            for (region, count) in regions {
                result.push_str(&format!("\"{}\" = {}\n", region, count));
            }

            result.push('\n');
        }

        result
    }
}

impl Default for CountsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_tree_new() {
        let tree = RegionTree::new();
        assert_eq!(tree.root_count, 0);
        assert!(tree.overrides.is_empty());
    }

    #[test]
    fn test_region_tree_with_root_count() {
        let tree = RegionTree::with_root_count(42);
        assert_eq!(tree.root_count, 42);
        assert!(tree.overrides.is_empty());
    }

    #[test]
    fn test_region_tree_set_count() {
        let mut tree = RegionTree::new();
        let region = RegionPath::new("src/legacy");

        tree.set_count(&region, 15);

        assert_eq!(tree.overrides.get(&region), Some(&15));
    }

    #[test]
    fn test_region_tree_set_count_root() {
        let mut tree = RegionTree::new();
        let root = RegionPath::new(".");

        tree.set_count(&root, 10);

        assert_eq!(tree.root_count, 10);
        assert_eq!(tree.overrides.get(&root), Some(&10));
    }

    #[test]
    fn test_region_tree_get_budget_root() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 5);

        // Any file should inherit from root
        assert_eq!(tree.get_budget(Path::new("src/foo.rs")), 5);
        assert_eq!(tree.get_budget(Path::new("tests/bar.rs")), 5);
    }

    #[test]
    fn test_region_tree_get_budget_specific_region() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 0);
        tree.set_count(&RegionPath::new("src/legacy"), 15);

        // Files in src/legacy should get 15
        assert_eq!(tree.get_budget(Path::new("src/legacy/foo.rs")), 15);
        assert_eq!(tree.get_budget(Path::new("src/legacy/bar.rs")), 15);

        // Files in other locations should get 0 from root
        assert_eq!(tree.get_budget(Path::new("src/foo.rs")), 0);
        assert_eq!(tree.get_budget(Path::new("tests/test.rs")), 0);
    }

    #[test]
    fn test_region_tree_get_budget_nested_inheritance() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 0);
        tree.set_count(&RegionPath::new("src/legacy"), 15);
        tree.set_count(&RegionPath::new("src/legacy/parser"), 7);

        // Files in src/legacy/parser should get 7
        assert_eq!(tree.get_budget(Path::new("src/legacy/parser/lexer.rs")), 7);

        // Files in src/legacy should get 15
        assert_eq!(tree.get_budget(Path::new("src/legacy/foo.rs")), 15);

        // Files in src should get 0 from root
        assert_eq!(tree.get_budget(Path::new("src/main.rs")), 0);
    }

    #[test]
    fn test_region_tree_get_budget_default_zero() {
        let tree = RegionTree::new();

        // With no overrides, should return root_count (0)
        assert_eq!(tree.get_budget(Path::new("src/foo.rs")), 0);
    }

    #[test]
    fn test_counts_manager_new() {
        let manager = CountsManager::new();
        assert!(manager.counts.is_empty());
    }

    #[test]
    fn test_counts_manager_parse_simple() {
        let toml = r#"
[no-unwrap]
"." = 0
"src/legacy" = 15
        "#;

        let manager = CountsManager::parse(toml).unwrap();

        let rule_id = RuleId::new("no-unwrap").unwrap();
        assert_eq!(
            manager.get_budget(&rule_id, Path::new("src/legacy/foo.rs")),
            15
        );
        assert_eq!(manager.get_budget(&rule_id, Path::new("src/main.rs")), 0);
    }

    #[test]
    fn test_counts_manager_parse_multiple_rules() {
        let toml = r#"
[no-unwrap]
"." = 0
"src/legacy" = 15

[no-todo-comments]
"." = 0
"src" = 23
        "#;

        let manager = CountsManager::parse(toml).unwrap();

        let no_unwrap = RuleId::new("no-unwrap").unwrap();
        let no_todo = RuleId::new("no-todo-comments").unwrap();

        assert_eq!(
            manager.get_budget(&no_unwrap, Path::new("src/legacy/foo.rs")),
            15
        );
        assert_eq!(manager.get_budget(&no_todo, Path::new("src/main.rs")), 23);
    }

    #[test]
    fn test_counts_manager_parse_complex_example() {
        // Example from DESIGN.md
        let toml = r#"
# Ratchet violation budgets
# These counts represent the maximum tolerated violations.
# Counts can only be reduced (tightened) or explicitly bumped with justification.

[no-unwrap]
# Root default: 0 (inherited by all regions unless overridden)
"." = 0
"src/legacy" = 15
"src/legacy/parser" = 7
"tests" = 50

[no-todo-comments]
"." = 0
"src" = 23

[my-company-rule]
"src/experimental" = 5
        "#;

        let manager = CountsManager::parse(toml).unwrap();

        // Test no-unwrap rule
        let no_unwrap = RuleId::new("no-unwrap").unwrap();
        assert_eq!(
            manager.get_budget(&no_unwrap, Path::new("src/foo/bar.rs")),
            0
        );
        assert_eq!(
            manager.get_budget(&no_unwrap, Path::new("src/legacy/foo.rs")),
            15
        );
        assert_eq!(
            manager.get_budget(&no_unwrap, Path::new("src/legacy/parser/x.rs")),
            7
        );
        assert_eq!(
            manager.get_budget(&no_unwrap, Path::new("tests/test.rs")),
            50
        );

        // Test no-todo-comments rule
        let no_todo = RuleId::new("no-todo-comments").unwrap();
        assert_eq!(manager.get_budget(&no_todo, Path::new("src/main.rs")), 23);
        assert_eq!(manager.get_budget(&no_todo, Path::new("tests/test.rs")), 0);

        // Test my-company-rule
        let company_rule = RuleId::new("my-company-rule").unwrap();
        assert_eq!(
            manager.get_budget(&company_rule, Path::new("src/experimental/foo.rs")),
            5
        );
        assert_eq!(
            manager.get_budget(&company_rule, Path::new("src/main.rs")),
            0
        );
    }

    #[test]
    fn test_counts_manager_parse_invalid_rule_id() {
        let toml = r#"
[invalid rule]
"." = 0
        "#;

        let result = CountsManager::parse(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_counts_manager_parse_invalid_count_negative() {
        let toml = r#"
[no-unwrap]
"." = -5
        "#;

        let result = CountsManager::parse(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_counts_manager_parse_invalid_count_non_integer() {
        let toml = r#"
[no-unwrap]
"." = "not a number"
        "#;

        let result = CountsManager::parse(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_counts_manager_get_budget_missing_rule() {
        let manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();

        // Missing rule should return 0 (strict enforcement)
        assert_eq!(manager.get_budget(&rule_id, Path::new("src/foo.rs")), 0);
    }

    #[test]
    fn test_counts_manager_set_count() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();
        let region = RegionPath::new("src/legacy");

        manager.set_count(&rule_id, &region, 15);

        assert_eq!(
            manager.get_budget(&rule_id, Path::new("src/legacy/foo.rs")),
            15
        );
    }

    #[test]
    fn test_counts_manager_set_count_overwrites() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();
        let region = RegionPath::new("src/legacy");

        manager.set_count(&rule_id, &region, 15);
        manager.set_count(&rule_id, &region, 10);

        assert_eq!(
            manager.get_budget(&rule_id, Path::new("src/legacy/foo.rs")),
            10
        );
    }

    #[test]
    fn test_counts_manager_to_toml_string_empty() {
        let manager = CountsManager::new();
        let toml = manager.to_toml_string();

        // Should have header comments but no rules
        assert!(toml.contains("# Ratchet violation budgets"));
        assert!(!toml.contains("["));
    }

    #[test]
    fn test_counts_manager_to_toml_string_simple() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();

        manager.set_count(&rule_id, &RegionPath::new("."), 0);
        manager.set_count(&rule_id, &RegionPath::new("src/legacy"), 15);

        let toml = manager.to_toml_string();

        assert!(toml.contains("[no-unwrap]"));
        assert!(toml.contains("\".\" = 0"));
        assert!(toml.contains("\"src/legacy\" = 15"));
    }

    #[test]
    fn test_counts_manager_to_toml_string_multiple_rules() {
        let mut manager = CountsManager::new();

        let no_unwrap = RuleId::new("no-unwrap").unwrap();
        manager.set_count(&no_unwrap, &RegionPath::new("."), 0);
        manager.set_count(&no_unwrap, &RegionPath::new("src/legacy"), 15);

        let no_todo = RuleId::new("no-todo-comments").unwrap();
        manager.set_count(&no_todo, &RegionPath::new("."), 0);
        manager.set_count(&no_todo, &RegionPath::new("src"), 23);

        let toml = manager.to_toml_string();

        assert!(toml.contains("[no-todo-comments]"));
        assert!(toml.contains("[no-unwrap]"));
    }

    #[test]
    fn test_counts_manager_roundtrip() {
        let original_toml = r#"
[no-unwrap]
"." = 0
"src/legacy" = 15

[no-todo-comments]
"." = 0
"src" = 23
        "#;

        let manager = CountsManager::parse(original_toml).unwrap();
        let serialized = manager.to_toml_string();
        let reparsed = CountsManager::parse(&serialized).unwrap();

        // Verify the counts are the same after roundtrip
        let no_unwrap = RuleId::new("no-unwrap").unwrap();
        let no_todo = RuleId::new("no-todo-comments").unwrap();

        assert_eq!(
            manager.get_budget(&no_unwrap, Path::new("src/legacy/foo.rs")),
            reparsed.get_budget(&no_unwrap, Path::new("src/legacy/foo.rs"))
        );
        assert_eq!(
            manager.get_budget(&no_todo, Path::new("src/main.rs")),
            reparsed.get_budget(&no_todo, Path::new("src/main.rs"))
        );
    }
}
