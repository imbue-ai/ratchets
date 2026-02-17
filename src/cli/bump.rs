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
use std::path::Path;

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
/// 1. Validates the rule ID exists (unless --all is used)
/// 2. If --all is used, bumps all enabled rules to their current violation counts
/// 3. If count is None, runs check to get current violation count
/// 4. If count is Some, validates it's not below current violations
/// 5. Updates ratchet-counts.toml with the new budget
///
/// # Arguments
///
/// * `rule_id` - The rule ID to bump (None when --all is used)
/// * `region` - The region path to bump (defaults to ".")
/// * `count` - Optional new count (auto-detects if None)
/// * `all` - Whether to bump all rules
///
/// # Returns
///
/// Exit code:
/// - 0: Success
/// - 2: Error (config error, invalid rule ID, count below current violations)
pub fn run_bump(rule_id: Option<&str>, region: &str, count: Option<u64>, all: bool) -> i32 {
    match run_bump_inner(rule_id, region, count, all) {
        Ok(()) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            EXIT_ERROR
        }
    }
}

/// Internal implementation of bump command
fn run_bump_inner(
    rule_id: Option<&str>,
    region: &str,
    count: Option<u64>,
    all: bool,
) -> Result<(), BumpError> {
    // Load configuration
    let config = super::common::load_config().map_err(BumpError::Config)?;
    let registry = super::common::build_registry(&config)?;

    // Handle --all flag
    if all {
        return run_bump_all(&config, &registry);
    }

    // When not using --all, rule_id is required
    let rule_id_str = rule_id.ok_or_else(|| {
        BumpError::Other("Rule ID is required when --all is not used".to_string())
    })?;

    // 1. Validate rule_id
    let rule_id = RuleId::new(rule_id_str).ok_or_else(|| {
        BumpError::Other(format!(
            "Invalid rule ID '{}'. Rule IDs must contain only alphanumeric characters, hyphens, and underscores.",
            rule_id_str
        ))
    })?;

    // 2. Verify rule exists in the registry
    if registry.get_rule(&rule_id).is_none() {
        return Err(BumpError::Other(format!(
            "Rule '{}' not found. Run 'ratchets list' to see available rules.",
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
                    "Cannot bump '{}' in region '{}' to {} (below current {} violations). Use 'ratchets tighten' to reduce budgets.",
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
    let counts = if counts_path.exists() {
        CountsManager::load(counts_path)?
    } else {
        CountsManager::new()
    };

    // 6. Validate region is configured (unless it's the root region ".")
    let region_path = RegionPath::new(region);
    if region != "." && !counts.is_configured_region(&rule_id, &region_path) {
        return Err(BumpError::Other(format!(
            "Region '{}' is not configured for rule '{}'. Add it to ratchet-counts.toml first.",
            region,
            rule_id.as_str()
        )));
    }

    // Make counts mutable for updates
    let mut counts = counts;
    let old_count = counts.get_budget_by_region(&rule_id, &region_path);

    // 7. Update the count
    counts.set_count(&rule_id, &region_path, new_count);

    // 8. Write back to file
    let toml_content = counts.to_toml_string();
    std::fs::write(counts_path, toml_content)?;

    // 9. Print success message
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

/// Bump all enabled rules to their current violation counts
fn run_bump_all(config: &Config, registry: &RuleRegistry) -> Result<(), BumpError> {
    // Load existing counts
    let counts_path = Path::new("ratchet-counts.toml");
    let mut counts = if counts_path.exists() {
        CountsManager::load(counts_path)?
    } else {
        CountsManager::new()
    };

    // Get all enabled rules from the registry
    let rule_ids: Vec<RuleId> = registry.iter_rules().map(|r| r.id().clone()).collect();

    if rule_ids.is_empty() {
        return Err(BumpError::Other(
            "No enabled rules found. Check your ratchets.toml configuration.".to_string(),
        ));
    }

    println!(
        "Bumping {} rules to current violation counts...",
        rule_ids.len()
    );

    // For each rule, get current violations and update budget for root region
    let mut updated = 0;
    let mut unchanged = 0;

    for rule_id in rule_ids {
        // Get current violation count for root region
        let current_count = get_current_violation_count(&rule_id, ".", config)?;

        // Get old budget
        let region_path = RegionPath::new(".");
        let old_count = counts.get_budget_by_region(&rule_id, &region_path);

        // Update the count
        counts.set_count(&rule_id, &region_path, current_count);

        if old_count == current_count {
            unchanged += 1;
        } else {
            println!(
                "  {} budget: {} -> {}",
                rule_id.as_str(),
                old_count,
                current_count
            );
            updated += 1;
        }
    }

    // Write back to file
    let toml_content = counts.to_toml_string();
    std::fs::write(counts_path, toml_content)?;

    println!(
        "\nCompleted: {} rules updated, {} unchanged",
        updated, unchanged
    );

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

    // Build a registry with only the target rule
    let mut single_rule_registry = super::common::build_registry(config)?;
    single_rule_registry.filter_to_single_rule(rule_id);

    // Discover files in the region
    let files = super::common::discover_files(&[region.to_string()], config)?;

    // Run execution engine with the single rule and CountsManager for region resolution
    let engine = ExecutionEngine::new(
        single_rule_registry,
        Some(std::sync::Arc::new(counts.clone())),
    );
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
}
