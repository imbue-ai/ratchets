//! ratchet-counts.toml parsing and management
//!
//! This module handles the violation budget tracking system. It parses
//! ratchet-counts.toml, resolves region inheritance, and provides methods
//! for querying and mutating counts.

use crate::error::ConfigError;
use crate::types::{RegionPath, RuleId};
use std::collections::{HashMap, HashSet};
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
/// - A set of explicitly configured regions
///
/// Regions inherit from their parent unless they have an explicit override.
#[derive(Debug, Clone)]
pub struct RegionTree {
    root_count: u64,
    overrides: HashMap<RegionPath, u64>,
    configured_regions: HashSet<RegionPath>,
}

impl RegionTree {
    /// Creates a new RegionTree with default root count of 0
    ///
    /// The root region "." is always implicitly configured.
    pub fn new() -> Self {
        let mut configured_regions = HashSet::new();
        configured_regions.insert(RegionPath::new("."));
        RegionTree {
            root_count: 0,
            overrides: HashMap::new(),
            configured_regions,
        }
    }

    /// Creates a new RegionTree with a specific root count
    ///
    /// The root region "." is always implicitly configured.
    pub fn with_root_count(count: u64) -> Self {
        let mut configured_regions = HashSet::new();
        configured_regions.insert(RegionPath::new("."));
        RegionTree {
            root_count: count,
            overrides: HashMap::new(),
            configured_regions,
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

    /// Gets the budget for a specific region path using inheritance
    ///
    /// This method is similar to `get_budget()` but operates on region paths
    /// directly instead of file paths. It walks up the region hierarchy to
    /// find the appropriate budget using the same inheritance logic.
    ///
    /// Algorithm:
    /// 1. Check if there's an explicit override for the given region
    /// 2. If not, walk up to parent regions
    /// 3. Repeat until reaching root or finding an override
    /// 4. If no override found, return root_count (default 0)
    pub fn get_budget_by_region(&self, region: &RegionPath) -> u64 {
        let region_str = region.as_str();
        let mut current_path = Path::new(region_str);

        loop {
            let current_region = RegionPath::new(current_path.to_string_lossy().to_string());

            // Check if there's an explicit override for this region
            if let Some(&count) = self.overrides.get(&current_region) {
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
    ///
    /// This also marks the region as explicitly configured.
    pub fn set_count(&mut self, region: &RegionPath, count: u64) {
        if region.as_str() == "." {
            self.root_count = count;
        }
        self.overrides.insert(region.clone(), count);
        self.configured_regions.insert(region.clone());
    }

    /// Returns true if the given region is explicitly configured
    ///
    /// The root region "." is always considered configured.
    /// Other regions are considered configured only if they have been
    /// explicitly added via `set_count()` or parsed from configuration.
    pub fn is_configured(&self, region: &RegionPath) -> bool {
        self.configured_regions.contains(region)
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

    /// Gets the budget for a specific rule and region path
    ///
    /// This is a convenience method for querying budgets by region path directly,
    /// without needing to construct a dummy file path. It uses the same inheritance
    /// logic as `get_budget()`, but operates on region paths instead of file paths.
    ///
    /// Returns the budget using inheritance logic from the RegionTree.
    /// If the rule is not present in the counts, returns 0 (strict enforcement).
    ///
    /// # Arguments
    ///
    /// * `rule_id` - The rule to query
    /// * `region` - The region path to query (e.g., ".", "src", "src/legacy")
    ///
    /// # Examples
    ///
    /// ```
    /// # use ratchets::config::counts::CountsManager;
    /// # use ratchets::types::{RuleId, RegionPath};
    /// let mut counts = CountsManager::new();
    /// let rule_id = RuleId::new("no-unwrap").unwrap();
    /// counts.set_count(&rule_id, &RegionPath::new("."), 0);
    /// counts.set_count(&rule_id, &RegionPath::new("src/legacy"), 15);
    ///
    /// assert_eq!(counts.get_budget_by_region(&rule_id, &RegionPath::new(".")), 0);
    /// assert_eq!(counts.get_budget_by_region(&rule_id, &RegionPath::new("src/legacy")), 15);
    /// ```
    pub fn get_budget_by_region(&self, rule_id: &RuleId, region: &RegionPath) -> u64 {
        self.counts
            .get(rule_id)
            .map(|tree| tree.get_budget_by_region(region))
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
    fn test_region_tree_get_budget_by_region_root() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 5);

        // Query by region should return the same as file-based query
        assert_eq!(tree.get_budget_by_region(&RegionPath::new(".")), 5);
    }

    #[test]
    fn test_region_tree_get_budget_by_region_specific() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 0);
        tree.set_count(&RegionPath::new("src/legacy"), 15);

        // Query specific region
        assert_eq!(
            tree.get_budget_by_region(&RegionPath::new("src/legacy")),
            15
        );

        // Query parent region
        assert_eq!(tree.get_budget_by_region(&RegionPath::new("src")), 0);
    }

    #[test]
    fn test_region_tree_get_budget_by_region_inheritance() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 0);
        tree.set_count(&RegionPath::new("src/legacy"), 15);

        // Child region should inherit from parent
        assert_eq!(
            tree.get_budget_by_region(&RegionPath::new("src/legacy/parser")),
            15
        );
    }

    #[test]
    fn test_region_tree_get_budget_by_region_nested() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 0);
        tree.set_count(&RegionPath::new("src/legacy"), 15);
        tree.set_count(&RegionPath::new("src/legacy/parser"), 7);

        // Should get exact match first
        assert_eq!(
            tree.get_budget_by_region(&RegionPath::new("src/legacy/parser")),
            7
        );

        // Deeper nesting should inherit
        assert_eq!(
            tree.get_budget_by_region(&RegionPath::new("src/legacy/parser/deep")),
            7
        );

        // Sibling should inherit from parent
        assert_eq!(
            tree.get_budget_by_region(&RegionPath::new("src/legacy/other")),
            15
        );
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
    fn test_counts_manager_get_budget_by_region_root() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();
        manager.set_count(&rule_id, &RegionPath::new("."), 10);

        assert_eq!(
            manager.get_budget_by_region(&rule_id, &RegionPath::new(".")),
            10
        );
    }

    #[test]
    fn test_counts_manager_get_budget_by_region_specific() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();
        manager.set_count(&rule_id, &RegionPath::new("."), 0);
        manager.set_count(&rule_id, &RegionPath::new("src/legacy"), 15);

        assert_eq!(
            manager.get_budget_by_region(&rule_id, &RegionPath::new("src/legacy")),
            15
        );
    }

    #[test]
    fn test_counts_manager_get_budget_by_region_inheritance() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();
        manager.set_count(&rule_id, &RegionPath::new("."), 0);
        manager.set_count(&rule_id, &RegionPath::new("src/legacy"), 15);

        // Child region should inherit from parent
        assert_eq!(
            manager.get_budget_by_region(&rule_id, &RegionPath::new("src/legacy/parser")),
            15
        );
    }

    #[test]
    fn test_counts_manager_get_budget_by_region_missing_rule() {
        let manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();

        // Missing rule should return 0 (strict enforcement)
        assert_eq!(
            manager.get_budget_by_region(&rule_id, &RegionPath::new("src")),
            0
        );
    }

    #[test]
    fn test_counts_manager_get_budget_by_region_consistency() {
        // Verify that get_budget_by_region returns the same result as get_budget
        // for equivalent file and region paths
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();
        manager.set_count(&rule_id, &RegionPath::new("."), 0);
        manager.set_count(&rule_id, &RegionPath::new("src"), 10);
        manager.set_count(&rule_id, &RegionPath::new("src/legacy"), 20);

        // Root region
        assert_eq!(
            manager.get_budget_by_region(&rule_id, &RegionPath::new(".")),
            manager.get_budget(&rule_id, Path::new("file.rs"))
        );

        // src region
        assert_eq!(
            manager.get_budget_by_region(&rule_id, &RegionPath::new("src")),
            manager.get_budget(&rule_id, Path::new("src/file.rs"))
        );

        // src/legacy region
        assert_eq!(
            manager.get_budget_by_region(&rule_id, &RegionPath::new("src/legacy")),
            manager.get_budget(&rule_id, Path::new("src/legacy/file.rs"))
        );
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

    #[test]
    fn test_region_tree_get_budget_deeply_nested() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 0);
        tree.set_count(&RegionPath::new("a"), 10);
        tree.set_count(&RegionPath::new("a/b"), 20);
        tree.set_count(&RegionPath::new("a/b/c"), 30);

        // Verify each level inherits correctly
        assert_eq!(tree.get_budget(Path::new("x/y.rs")), 0); // Root default
        assert_eq!(tree.get_budget(Path::new("a/file.rs")), 10);
        assert_eq!(tree.get_budget(Path::new("a/b/file.rs")), 20);
        assert_eq!(tree.get_budget(Path::new("a/b/c/file.rs")), 30);

        // Deeper nesting should inherit from nearest parent
        assert_eq!(tree.get_budget(Path::new("a/b/c/d/e/file.rs")), 30);
    }

    #[test]
    fn test_region_tree_get_budget_sibling_regions() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 0);
        tree.set_count(&RegionPath::new("src"), 10);
        tree.set_count(&RegionPath::new("tests"), 20);
        tree.set_count(&RegionPath::new("benches"), 30);

        // Verify siblings don't interfere with each other
        assert_eq!(tree.get_budget(Path::new("src/main.rs")), 10);
        assert_eq!(tree.get_budget(Path::new("tests/test.rs")), 20);
        assert_eq!(tree.get_budget(Path::new("benches/bench.rs")), 30);

        // Files in other locations should inherit from root
        assert_eq!(tree.get_budget(Path::new("docs/readme.md")), 0);
    }

    #[test]
    fn test_counts_manager_parse_empty_file() {
        let toml = "";
        let manager = CountsManager::parse(toml).unwrap();

        // Empty file should parse successfully
        let rule_id = RuleId::new("any-rule").unwrap();
        assert_eq!(manager.get_budget(&rule_id, Path::new("src/foo.rs")), 0);
    }

    #[test]
    fn test_counts_manager_parse_only_root_counts() {
        let toml = r#"
[no-unwrap]
"." = 5

[no-todo]
"." = 10
        "#;

        let manager = CountsManager::parse(toml).unwrap();

        let no_unwrap = RuleId::new("no-unwrap").unwrap();
        let no_todo = RuleId::new("no-todo").unwrap();

        // All files should inherit from root
        assert_eq!(manager.get_budget(&no_unwrap, Path::new("src/foo.rs")), 5);
        assert_eq!(manager.get_budget(&no_unwrap, Path::new("a/b/c.rs")), 5);
        assert_eq!(manager.get_budget(&no_todo, Path::new("src/foo.rs")), 10);
        assert_eq!(manager.get_budget(&no_todo, Path::new("x/y/z.rs")), 10);
    }

    #[test]
    fn test_counts_manager_parse_zero_counts() {
        let toml = r#"
[no-unwrap]
"." = 0
"src/legacy" = 0
        "#;

        let manager = CountsManager::parse(toml).unwrap();
        let rule_id = RuleId::new("no-unwrap").unwrap();

        // Zero counts should be preserved
        assert_eq!(manager.get_budget(&rule_id, Path::new("src/foo.rs")), 0);
        assert_eq!(
            manager.get_budget(&rule_id, Path::new("src/legacy/bar.rs")),
            0
        );
    }

    #[test]
    fn test_counts_manager_parse_large_counts() {
        let toml = r#"
[no-unwrap]
"." = 999999
"src" = 1000000
        "#;

        let manager = CountsManager::parse(toml).unwrap();
        let rule_id = RuleId::new("no-unwrap").unwrap();

        // Large counts should be supported
        assert_eq!(manager.get_budget(&rule_id, Path::new("root.rs")), 999999);
        assert_eq!(
            manager.get_budget(&rule_id, Path::new("src/main.rs")),
            1000000
        );
    }

    #[test]
    fn test_counts_manager_set_count_creates_new_rule() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("new-rule").unwrap();
        let region = RegionPath::new("src");

        // Setting count for non-existent rule should create it
        manager.set_count(&rule_id, &region, 42);

        assert_eq!(manager.get_budget(&rule_id, Path::new("src/file.rs")), 42);
    }

    #[test]
    fn test_counts_manager_set_count_multiple_regions() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("my-rule").unwrap();

        manager.set_count(&rule_id, &RegionPath::new("."), 0);
        manager.set_count(&rule_id, &RegionPath::new("src"), 10);
        manager.set_count(&rule_id, &RegionPath::new("tests"), 20);

        assert_eq!(manager.get_budget(&rule_id, Path::new("root.rs")), 0);
        assert_eq!(manager.get_budget(&rule_id, Path::new("src/main.rs")), 10);
        assert_eq!(manager.get_budget(&rule_id, Path::new("tests/test.rs")), 20);
    }

    #[test]
    fn test_counts_manager_to_toml_string_sorted_output() {
        let mut manager = CountsManager::new();

        // Add rules in non-alphabetical order
        manager.set_count(
            &RuleId::new("zebra-rule").unwrap(),
            &RegionPath::new("."),
            1,
        );
        manager.set_count(
            &RuleId::new("alpha-rule").unwrap(),
            &RegionPath::new("."),
            2,
        );
        manager.set_count(&RuleId::new("beta-rule").unwrap(), &RegionPath::new("."), 3);

        let toml = manager.to_toml_string();

        // Verify alphabetical ordering in output
        let alpha_pos = toml.find("[alpha-rule]").unwrap();
        let beta_pos = toml.find("[beta-rule]").unwrap();
        let zebra_pos = toml.find("[zebra-rule]").unwrap();

        assert!(alpha_pos < beta_pos);
        assert!(beta_pos < zebra_pos);
    }

    #[test]
    fn test_counts_manager_parse_invalid_rule_not_table() {
        let toml = r#"
no-unwrap = 5
        "#;

        let result = CountsManager::parse(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_counts_manager_parse_invalid_toml_syntax() {
        let toml = r#"
[no-unwrap
"." = 0
        "#;

        let result = CountsManager::parse(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_region_tree_default_trait() {
        let tree = RegionTree::default();
        assert_eq!(tree.root_count, 0);
        assert!(tree.overrides.is_empty());
    }

    #[test]
    fn test_counts_manager_default_trait() {
        let manager = CountsManager::default();
        assert!(manager.counts.is_empty());
    }

    #[test]
    fn test_counts_manager_parse_windows_style_paths() {
        // CountsManager should handle normalized region paths
        let toml = r#"
[no-unwrap]
"." = 0
"src/legacy" = 10
        "#;

        let manager = CountsManager::parse(toml).unwrap();
        let rule_id = RuleId::new("no-unwrap").unwrap();

        // Both Unix and Windows style paths should work (internally normalized)
        assert_eq!(
            manager.get_budget(&rule_id, Path::new("src/legacy/file.rs")),
            10
        );
    }

    #[test]
    fn test_region_tree_get_budget_with_trailing_slash() {
        let mut tree = RegionTree::new();
        tree.set_count(&RegionPath::new("."), 0);
        tree.set_count(&RegionPath::new("src"), 10);

        // Path normalization should handle trailing slashes
        assert_eq!(tree.get_budget(Path::new("src/main.rs")), 10);
    }

    #[test]
    fn test_counts_manager_multiple_rules_same_regions() {
        let toml = r#"
[rule-a]
"." = 1
"src" = 2

[rule-b]
"." = 10
"src" = 20

[rule-c]
"." = 100
"src" = 200
        "#;

        let manager = CountsManager::parse(toml).unwrap();

        let rule_a = RuleId::new("rule-a").unwrap();
        let rule_b = RuleId::new("rule-b").unwrap();
        let rule_c = RuleId::new("rule-c").unwrap();

        // Each rule should maintain independent budgets
        assert_eq!(manager.get_budget(&rule_a, Path::new("src/x.rs")), 2);
        assert_eq!(manager.get_budget(&rule_b, Path::new("src/x.rs")), 20);
        assert_eq!(manager.get_budget(&rule_c, Path::new("src/x.rs")), 200);
    }

    #[test]
    fn test_counts_manager_to_toml_regions_sorted() {
        let mut manager = CountsManager::new();
        let rule_id = RuleId::new("test-rule").unwrap();

        // Add regions in non-alphabetical order
        manager.set_count(&rule_id, &RegionPath::new("z/region"), 1);
        manager.set_count(&rule_id, &RegionPath::new("a/region"), 2);
        manager.set_count(&rule_id, &RegionPath::new("m/region"), 3);

        let toml = manager.to_toml_string();

        // Find positions of each region
        let a_pos = toml.find("\"a/region\"").unwrap();
        let m_pos = toml.find("\"m/region\"").unwrap();
        let z_pos = toml.find("\"z/region\"").unwrap();

        // Verify alphabetical ordering
        assert!(a_pos < m_pos);
        assert!(m_pos < z_pos);
    }

    #[test]
    fn test_region_tree_configured_regions_from_parsing() {
        // Verify that parsing TOML populates configured_regions correctly
        let toml = r#"
[no-unwrap]
"." = 0
"src/legacy" = 15
"src/legacy/parser" = 7
"tests" = 50
        "#;

        let manager = CountsManager::parse(toml).unwrap();
        let rule_id = RuleId::new("no-unwrap").unwrap();

        // Get the region tree for this rule
        let tree = manager.counts.get(&rule_id).unwrap();

        // Verify all configured regions are tracked
        assert!(tree.is_configured(&RegionPath::new(".")));
        assert!(tree.is_configured(&RegionPath::new("src/legacy")));
        assert!(tree.is_configured(&RegionPath::new("src/legacy/parser")));
        assert!(tree.is_configured(&RegionPath::new("tests")));

        // Verify unconfigured regions are not in the set
        assert!(!tree.is_configured(&RegionPath::new("src")));
        assert!(!tree.is_configured(&RegionPath::new("src/new")));
    }

    #[test]
    fn test_region_tree_is_configured_region() {
        // Test the is_configured method returns true for configured regions
        // and false for unconfigured ones
        let mut tree = RegionTree::new();

        // Root "." should always be considered configured
        assert!(tree.is_configured(&RegionPath::new(".")));

        // Set some explicit regions
        tree.set_count(&RegionPath::new("src/legacy"), 15);
        tree.set_count(&RegionPath::new("tests"), 50);

        // Explicitly configured regions should return true
        assert!(tree.is_configured(&RegionPath::new("src/legacy")));
        assert!(tree.is_configured(&RegionPath::new("tests")));

        // Regions not explicitly configured should return false
        assert!(!tree.is_configured(&RegionPath::new("src")));
        assert!(!tree.is_configured(&RegionPath::new("src/legacy/parser")));
        assert!(!tree.is_configured(&RegionPath::new("other")));
    }
}
