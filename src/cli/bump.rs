//! Bump command implementation
//!
//! This module implements the `ratchet bump` command, which:
//! - Increases a rule's violation budget for a specific region
//! - Auto-detects current violations if count is not provided
//! - Updates ratchet-counts.toml with the new budget
//! - Validates that bumps don't go below current violation counts

use crate::cli::common::{EXIT_ERROR, EXIT_SUCCESS};
use crate::config::counts::CountsManager;
use crate::config::ratchet_toml::Config;
use crate::engine::aggregator::ViolationAggregator;
use crate::engine::executor::ExecutionEngine;
use crate::error::ConfigError;
use crate::rules::RuleRegistry;
use crate::types::{RegionPath, RuleId};
use std::path::{Path, PathBuf};

/// Error type specific to bump command
#[derive(Debug, thiserror::Error)]
enum BumpError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Rule error: {0}")]
    Rule(#[from] crate::error::RuleError),

    #[error("File walker error: {0}")]
    FileWalker(#[from] crate::engine::file_walker::FileWalkerError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

/// Run the bump command
///
/// This is the main entry point for the bump command. It:
/// 1. Validates the rule ID exists
/// 2. If count is None, runs check to get current violation count
/// 3. If count is Some, validates it's not below current violations
/// 4. Updates ratchet-counts.toml with the new budget
///
/// # Arguments
///
/// * `rule_id` - The rule ID to bump
/// * `region` - The region path to bump (defaults to ".")
/// * `count` - Optional new count (auto-detects if None)
///
/// # Returns
///
/// Exit code:
/// - 0: Success
/// - 2: Error (config error, invalid rule ID, count below current violations)
pub fn run_bump(rule_id: &str, region: &str, count: Option<u64>) -> i32 {
    match run_bump_inner(rule_id, region, count) {
        Ok(()) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            EXIT_ERROR
        }
    }
}

/// Internal implementation of bump command
fn run_bump_inner(rule_id: &str, region: &str, count: Option<u64>) -> Result<(), BumpError> {
    // 1. Validate rule_id
    let rule_id = RuleId::new(rule_id).ok_or_else(|| {
        BumpError::Other(format!(
            "Invalid rule ID '{}'. Rule IDs must contain only alphanumeric characters, hyphens, and underscores.",
            rule_id
        ))
    })?;

    // 2. Load configuration and verify rule exists
    let config = super::common::load_config().map_err(BumpError::Config)?;
    let registry = super::common::build_registry(&config)?;

    // Verify the rule exists in the registry
    if registry.get_rule(&rule_id).is_none() {
        return Err(BumpError::Other(format!(
            "Rule '{}' not found. Run 'ratchet list' to see available rules.",
            rule_id.as_str()
        )));
    }

    // 3. Get current violation count for this rule/region
    let current_count = get_current_violation_count(&rule_id, region, &config)?;

    // 4. Determine the new count
    let new_count = match count {
        Some(n) => {
            // User specified a count - validate it's not below current violations
            if n < current_count {
                return Err(BumpError::Other(format!(
                    "Cannot bump '{}' in region '{}' to {} (below current {} violations). Use 'ratchet tighten' to reduce budgets.",
                    rule_id.as_str(),
                    region,
                    n,
                    current_count
                )));
            }
            n
        }
        None => {
            // Auto-detect: use current violation count
            current_count
        }
    };

    // 5. Load existing counts
    let counts_path = Path::new("ratchet-counts.toml");
    let mut counts = if counts_path.exists() {
        CountsManager::load(counts_path)?
    } else {
        CountsManager::new()
    };

    // Get the old count for display purposes
    let region_path = RegionPath::new(region);
    let old_count = get_budget_for_region(&counts, &rule_id, &region_path);

    // 6. Update the count
    counts.set_count(&rule_id, &region_path, new_count);

    // 7. Write back to file
    let toml_content = counts.to_toml_string();
    std::fs::write(counts_path, toml_content)?;

    // 8. Print success message
    if old_count == new_count {
        println!(
            "Budget for '{}' in region '{}' is already {}",
            rule_id.as_str(),
            region,
            new_count
        );
    } else {
        println!(
            "Bumped '{}' budget for region '{}' from {} to {}",
            rule_id.as_str(),
            region,
            old_count,
            new_count
        );
    }

    Ok(())
}

/// Get current violation count by running check for a specific rule/region
fn get_current_violation_count(
    rule_id: &RuleId,
    region: &str,
    config: &Config,
) -> Result<u64, BumpError> {
    // Load existing counts (we'll use budget 0 for this rule to count all violations)
    let counts_path = Path::new("ratchet-counts.toml");
    let mut counts = if counts_path.exists() {
        CountsManager::load(counts_path)?
    } else {
        CountsManager::new()
    };

    // Temporarily set the budget to a very high number so check passes and we can count violations
    // We'll use the existing count manager but won't save it
    let region_path = RegionPath::new(region);
    counts.set_count(rule_id, &region_path, u64::MAX);

    // Build a filtered registry with only the target rule
    let full_registry = super::common::build_registry(config)?;
    let mut single_rule_registry = RuleRegistry::new();

    // Copy only the target rule to the single rule registry
    // We need to re-load the rule since we can't clone Box<dyn Rule>
    if full_registry.get_rule(rule_id).is_some() {
        // The rule exists, now we need to create a registry with only this rule
        // Filter to only keep the target rule
        let mut filtered_config = config.clone();
        // Disable all rules except the target
        for other_rule_id in full_registry
            .iter_rules()
            .map(|r| r.id().clone())
            .collect::<Vec<_>>()
        {
            if other_rule_id != *rule_id {
                filtered_config.rules.builtin.insert(
                    other_rule_id.clone(),
                    crate::config::ratchet_toml::RuleValue::Enabled(false),
                );
                filtered_config.rules.custom.insert(
                    other_rule_id,
                    crate::config::ratchet_toml::RuleValue::Enabled(false),
                );
            }
        }

        // Rebuild registry with the filtered config
        single_rule_registry = super::common::build_registry(&filtered_config)?;
    }

    // Discover files in the region
    let files = super::common::discover_files(&[region.to_string()], config)?;

    // Run execution engine with the single rule
    let engine = ExecutionEngine::new(single_rule_registry);
    let execution_result = engine.execute(files);

    // Aggregate violations
    let aggregator = ViolationAggregator::new(counts);
    let aggregation_result = aggregator.aggregate(execution_result.violations);

    // Find the status for our target rule/region
    let status = aggregation_result
        .statuses
        .iter()
        .find(|s| s.rule_id == *rule_id && s.region.as_str() == region);

    // Return the actual count
    match status {
        Some(s) => Ok(s.actual_count),
        None => Ok(0), // No violations found
    }
}

/// Get the budget for a specific region from the CountsManager
///
/// This is different from CountsManager::get_budget which takes a file path.
/// This function looks up the budget for a specific region path directly.
fn get_budget_for_region(counts: &CountsManager, rule_id: &RuleId, region: &RegionPath) -> u64 {
    // We need to use a file path in that region to query the budget
    // Construct a dummy file path based on the region
    let dummy_file_path = if region.as_str() == "." {
        PathBuf::from("file.rs")
    } else {
        PathBuf::from(region.as_str()).join("file.rs")
    };

    counts.get_budget(rule_id, &dummy_file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_error_display() {
        let err = BumpError::Other("test error".to_string());
        assert_eq!(err.to_string(), "test error");
    }

    #[test]
    fn test_invalid_rule_id() {
        let result = RuleId::new("invalid rule");
        assert!(result.is_none());
    }

    #[test]
    fn test_valid_rule_id() {
        let result = RuleId::new("no-unwrap");
        assert!(result.is_some());
        assert_eq!(result.unwrap().as_str(), "no-unwrap");
    }

    #[test]
    fn test_region_path_normalization() {
        let region = RegionPath::new("src/legacy");
        assert_eq!(region.as_str(), "src/legacy");

        let root = RegionPath::new(".");
        assert_eq!(root.as_str(), ".");
    }

    #[test]
    fn test_get_budget_for_region_root() {
        let mut counts = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();
        counts.set_count(&rule_id, &RegionPath::new("."), 10);

        let budget = get_budget_for_region(&counts, &rule_id, &RegionPath::new("."));
        assert_eq!(budget, 10);
    }

    #[test]
    fn test_get_budget_for_region_specific() {
        let mut counts = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();
        counts.set_count(&rule_id, &RegionPath::new("src/legacy"), 15);

        let budget = get_budget_for_region(&counts, &rule_id, &RegionPath::new("src/legacy"));
        assert_eq!(budget, 15);
    }

    #[test]
    fn test_get_budget_for_region_missing() {
        let counts = CountsManager::new();
        let rule_id = RuleId::new("no-unwrap").unwrap();

        let budget = get_budget_for_region(&counts, &rule_id, &RegionPath::new("src"));
        assert_eq!(budget, 0); // Should default to 0
    }
}
