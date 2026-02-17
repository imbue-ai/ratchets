//! Tighten command implementation
//!
//! This module implements the `ratchet tighten` command, which:
//! - Runs check to get current violation counts
//! - Reduces budgets to match current violations (if lower than budget)
//! - Fails if any violations exceed current budgets
//! - Supports filtering by rule_id and region

use crate::cli::common::{EXIT_ERROR, EXIT_EXCEEDED, EXIT_SUCCESS};
use crate::config::counts::CountsManager;
use crate::config::ratchet_toml::Config;
use crate::engine::aggregator::ViolationAggregator;
use crate::engine::executor::ExecutionEngine;
use crate::error::ConfigError;
use crate::types::{RegionPath, RuleId};
use std::path::Path;

/// Error type specific to tighten command
#[derive(Debug, thiserror::Error)]
enum TightenError {
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

/// Run the tighten command
///
/// This is the main entry point for the tighten command. It:
/// 1. Runs check to get current violation counts
/// 2. For each rule/region: if current < budget, reduce budget to current
/// 3. Fails if any current > budget (violations exceed budget)
/// 4. Updates ratchet-counts.toml with new budgets
///
/// # Arguments
///
/// * `rule_id` - Optional specific rule to tighten (tightens all if None)
/// * `region` - Optional specific region to tighten
///
/// # Returns
///
/// Exit code:
/// - 0: Success (including no changes needed)
/// - 1: Violations exceed budget (can't tighten)
/// - 2: Error (config error, etc.)
pub fn run_tighten(rule_id: Option<&str>, region: Option<&str>) -> i32 {
    match run_tighten_inner(rule_id, region) {
        Ok(TightenResult::Success(count)) => {
            if count == 0 {
                eprintln!("No budgets needed tightening");
            }
            EXIT_SUCCESS
        }
        Ok(TightenResult::ExceededBudget(violations)) => {
            eprintln!("Error: Cannot tighten while violations exceed budget\n");
            for violation in violations {
                eprintln!(
                    "{} in \"{}\": {} violations exceed budget of {}",
                    violation.rule_id.as_str(),
                    violation.region.as_str(),
                    violation.actual_count,
                    violation.budget
                );
            }
            eprintln!("\nFix the violations first or use 'ratchets bump' to increase budget");
            EXIT_EXCEEDED
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            EXIT_ERROR
        }
    }
}

/// Result of tighten operation
enum TightenResult {
    Success(usize), // Number of budgets tightened
    ExceededBudget(Vec<ExceededViolation>),
}

/// Information about a rule/region that exceeds budget
struct ExceededViolation {
    rule_id: RuleId,
    region: RegionPath,
    actual_count: u64,
    budget: u64,
}

/// Internal implementation of tighten command
fn run_tighten_inner(
    rule_id: Option<&str>,
    region: Option<&str>,
) -> Result<TightenResult, TightenError> {
    // 1. Validate rule_id if provided
    let rule_id_filter = if let Some(id) = rule_id {
        let validated = RuleId::new(id).ok_or_else(|| {
            TightenError::Other(format!(
                "Invalid rule ID '{}'. Rule IDs must contain only alphanumeric characters, hyphens, and underscores.",
                id
            ))
        })?;
        Some(validated)
    } else {
        None
    };

    // 2. Load configuration
    let config = super::common::load_config().map_err(TightenError::Config)?;

    // 3. Run check to get all current violation counts
    let aggregation_result = run_full_check(&config)?;

    // 4. Load existing counts
    let counts_path = Path::new("ratchet-counts.toml");
    let mut counts = if counts_path.exists() {
        CountsManager::load(counts_path)?
    } else {
        CountsManager::new()
    };

    // 5. Process each rule/region status
    let mut exceeded_violations = Vec::new();
    let mut tightened_budgets = Vec::new();

    for status in &aggregation_result.statuses {
        // Apply filters
        if let Some(ref filter_rule_id) = rule_id_filter
            && status.rule_id != *filter_rule_id
        {
            continue;
        }

        if let Some(filter_region) = region
            && status.region.as_str() != filter_region
        {
            continue;
        }

        // Check if we can tighten
        if status.actual_count > status.budget {
            // Violations exceed budget - can't tighten
            exceeded_violations.push(ExceededViolation {
                rule_id: status.rule_id.clone(),
                region: status.region.clone(),
                actual_count: status.actual_count,
                budget: status.budget,
            });
        } else if status.actual_count < status.budget {
            // Can tighten - reduce budget to current count
            tightened_budgets.push((
                status.rule_id.clone(),
                status.region.clone(),
                status.budget,
                status.actual_count,
            ));
            counts.set_count(&status.rule_id, &status.region, status.actual_count);
        }
        // If actual_count == budget, no change needed
    }

    // 6. Check if any violations exceeded budget
    if !exceeded_violations.is_empty() {
        return Ok(TightenResult::ExceededBudget(exceeded_violations));
    }

    // 7. Write updated counts to file if any changes were made
    if !tightened_budgets.is_empty() {
        let toml_content = counts.to_toml_string();
        std::fs::write(counts_path, toml_content)?;

        // 8. Print summary of changes
        eprintln!("Tightening budgets...\n");
        for (rule_id, region, old_budget, new_budget) in &tightened_budgets {
            eprintln!(
                "Tightened {} in \"{}\": {} -> {}",
                rule_id.as_str(),
                region.as_str(),
                old_budget,
                new_budget
            );
        }
        eprintln!("\n{} budgets tightened", tightened_budgets.len());
    }

    Ok(TightenResult::Success(tightened_budgets.len()))
}

/// Run a full check and return aggregation results
fn run_full_check(
    config: &Config,
) -> Result<crate::engine::aggregator::AggregationResult, TightenError> {
    // Load counts
    let counts_path = Path::new("ratchet-counts.toml");
    let counts = if counts_path.exists() {
        CountsManager::load(counts_path)?
    } else {
        CountsManager::new()
    };

    // Build rule registry
    let registry = super::common::build_registry(config)?;

    if registry.is_empty() {
        return Err(TightenError::Other(
            "No rules are enabled. Nothing to tighten.".to_string(),
        ));
    }

    // Discover files
    let files = super::common::discover_files(&[".".to_string()], config)?;

    if files.is_empty() {
        return Err(TightenError::Other("No files found to check.".to_string()));
    }

    // Run execution engine with CountsManager for region resolution
    let engine = ExecutionEngine::new(registry, Some(std::sync::Arc::new(counts.clone())));
    let execution_result = engine.execute(files);

    // Aggregate violations
    let aggregator = ViolationAggregator::new(counts);
    let aggregation_result = aggregator.aggregate(execution_result.violations);

    Ok(aggregation_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tighten_error_display() {
        let err = TightenError::Other("test error".to_string());
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
    fn test_exceeded_violation_construction() {
        let violation = ExceededViolation {
            rule_id: RuleId::new("no-unwrap").unwrap(),
            region: RegionPath::new("src"),
            actual_count: 5,
            budget: 3,
        };

        assert_eq!(violation.rule_id.as_str(), "no-unwrap");
        assert_eq!(violation.region.as_str(), "src");
        assert_eq!(violation.actual_count, 5);
        assert_eq!(violation.budget, 3);
    }

    #[test]
    fn test_tighten_result_variants() {
        // Test Success variant
        let success = TightenResult::Success(5);
        match success {
            TightenResult::Success(count) => assert_eq!(count, 5),
            _ => panic!("Expected Success variant"),
        }

        // Test ExceededBudget variant
        let exceeded = TightenResult::ExceededBudget(vec![ExceededViolation {
            rule_id: RuleId::new("test-rule").unwrap(),
            region: RegionPath::new("src"),
            actual_count: 10,
            budget: 5,
        }]);
        match exceeded {
            TightenResult::ExceededBudget(violations) => {
                assert_eq!(violations.len(), 1);
                assert_eq!(violations[0].actual_count, 10);
            }
            _ => panic!("Expected ExceededBudget variant"),
        }
    }
}
