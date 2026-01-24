//! List command implementation
//!
//! This module implements the `ratchet list` command, which:
//! - Lists all enabled rules with their status
//! - Shows rule ID, source (built-in/custom), languages, current count, budget, status
//! - Supports both human-readable and JSONL output formats

use crate::cli::args::OutputFormat;
use crate::cli::common::{EXIT_ERROR, EXIT_SUCCESS, load_counts};
use crate::config::counts::CountsManager;
use crate::engine::aggregator::ViolationAggregator;
use crate::engine::executor::ExecutionEngine;
use crate::rules::Rule;
use crate::types::{Language, RuleId, Severity};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Error type specific to list command
#[derive(Debug, thiserror::Error)]
enum ListError {
    #[error("Configuration error: {0}")]
    Config(#[from] crate::error::ConfigError),

    #[error("Rule error: {0}")]
    Rule(#[from] crate::error::RuleError),

    #[error("File walker error: {0}")]
    FileWalker(#[from] crate::engine::file_walker::FileWalkerError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    #[allow(dead_code)] // Reserved for future use
    Other(String),
}

/// Run the list command
///
/// This is the main entry point for the list command. It loads the configuration,
/// runs a check to get current violation counts, and displays all enabled rules
/// with their current status.
///
/// # Arguments
///
/// * `format` - Output format (human or JSONL)
///
/// # Returns
///
/// Exit code:
/// - 0: Success
/// - 2: Error
pub fn run_list(format: OutputFormat) -> i32 {
    match run_list_inner(format) {
        Ok(()) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            EXIT_ERROR
        }
    }
}

/// Internal implementation of list command
fn run_list_inner(format: OutputFormat) -> Result<(), ListError> {
    // 1. Load ratchet.toml config
    let config = super::common::load_config()?;

    // 2. Load ratchet-counts.toml
    let counts = load_counts()?;

    // 3. Build rule registry (load builtin + custom rules, apply config filter)
    let registry = super::common::build_registry(&config)?;

    // If no rules are enabled, show empty list
    if registry.is_empty() {
        if format == OutputFormat::Human {
            println!("No rules are enabled.");
        }
        return Ok(());
    }

    // 5. Discover files using FileWalker
    let files = super::common::discover_files(&[".".to_string()], &config)?;

    // 6. Run ExecutionEngine to get current violation counts
    // We need to clone rule metadata before moving registry into engine
    let rule_metadata: Vec<RuleMetadata> = registry
        .iter_rules()
        .map(|rule: &dyn Rule| RuleMetadata {
            rule_id: rule.id().clone(),
            description: rule.description().to_string(),
            languages: rule.languages().to_vec(),
            severity: rule.severity(),
        })
        .collect();

    let engine = ExecutionEngine::new(registry);
    let execution_result = engine.execute(files);

    // 7. Aggregate violations to get per-rule counts
    let aggregator = ViolationAggregator::new(counts.clone());
    let aggregation_result = aggregator.aggregate(execution_result.violations);

    // 8. Build rule status list
    let rule_statuses = build_rule_statuses(&rule_metadata, &counts, &aggregation_result);

    // 9. Format and print output
    match format {
        OutputFormat::Human => print_human_output(&rule_statuses),
        OutputFormat::Jsonl => print_jsonl_output(&rule_statuses),
    }

    Ok(())
}

/// Metadata about a rule extracted from the registry
#[derive(Debug, Clone)]
struct RuleMetadata {
    rule_id: RuleId,
    description: String,
    languages: Vec<Language>,
    severity: Severity,
}

/// Status information for a single rule
#[derive(Debug, Clone)]
struct RuleStatus {
    rule_id: RuleId,
    description: String,
    source: RuleSource,
    languages: Vec<Language>,
    severity: Severity,
    violations: u64,
    budget: u64,
    status: CheckStatus,
}

/// Source of a rule (builtin or custom)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Custom variant reserved for future use
enum RuleSource {
    Builtin,
    Custom,
}

impl RuleSource {
    fn as_str(&self) -> &'static str {
        match self {
            RuleSource::Builtin => "builtin",
            RuleSource::Custom => "custom",
        }
    }
}

/// Status of a rule check (pass, fail, or over-budget)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckStatus {
    Pass,
    OverBudget,
}

impl CheckStatus {
    fn as_str(&self) -> &'static str {
        match self {
            CheckStatus::Pass => "pass",
            CheckStatus::OverBudget => "over_budget",
        }
    }
}

/// Build rule statuses by combining rule metadata with violation counts
fn build_rule_statuses(
    rule_metadata: &[RuleMetadata],
    counts: &CountsManager,
    aggregation_result: &crate::engine::aggregator::AggregationResult,
) -> Vec<RuleStatus> {
    // Build a map of rule_id -> total violations across all regions
    let mut violation_counts: HashMap<RuleId, u64> = HashMap::new();
    let mut violation_budgets: HashMap<RuleId, u64> = HashMap::new();
    let mut violation_passed: HashMap<RuleId, bool> = HashMap::new();

    for status in &aggregation_result.statuses {
        *violation_counts.entry(status.rule_id.clone()).or_insert(0) += status.actual_count;
        *violation_budgets.entry(status.rule_id.clone()).or_insert(0) += status.budget;

        // If any region fails, the rule fails
        let current_passed = violation_passed
            .get(&status.rule_id)
            .copied()
            .unwrap_or(true);
        violation_passed.insert(status.rule_id.clone(), current_passed && status.passed);
    }

    let mut statuses = Vec::new();

    for metadata in rule_metadata {
        let rule_id = &metadata.rule_id;
        let violations = violation_counts.get(rule_id).copied().unwrap_or(0);
        let budget = violation_budgets.get(rule_id).copied().unwrap_or_else(|| {
            // If rule has no violations, get budget from counts manager
            // Use root region "." as default
            counts.get_budget(rule_id, Path::new("."))
        });
        let passed = violation_passed.get(rule_id).copied().unwrap_or(true);

        let status = RuleStatus {
            rule_id: rule_id.clone(),
            description: metadata.description.clone(),
            source: determine_rule_source(rule_id),
            languages: metadata.languages.clone(),
            severity: metadata.severity,
            violations,
            budget,
            status: if passed {
                CheckStatus::Pass
            } else {
                CheckStatus::OverBudget
            },
        };

        statuses.push(status);
    }

    // Sort by rule_id for deterministic output
    statuses.sort_by(|a, b| a.rule_id.as_str().cmp(b.rule_id.as_str()));

    statuses
}

/// Determine if a rule is builtin or custom based on naming convention
/// This is a heuristic - in practice, we'd want to track this in the registry
fn determine_rule_source(_rule_id: &RuleId) -> RuleSource {
    // For now, we'll assume all rules are builtin
    // In a full implementation, the registry would track this
    RuleSource::Builtin
}

/// Print human-readable output
fn print_human_output(statuses: &[RuleStatus]) {
    println!("Rules ({} enabled):", statuses.len());
    println!();

    for status in statuses {
        println!("{} ({})", status.rule_id.as_str(), status.source.as_str());
        println!("  Description: {}", status.description);

        // Format languages
        let langs: Vec<String> = status
            .languages
            .iter()
            .map(|l| format!("{:?}", l).to_lowercase())
            .collect();
        println!("  Languages: {}", langs.join(", "));

        println!("  Severity: {:?}", status.severity);
        println!("  Violations: {}", status.violations);
        println!("  Budget: {}", status.budget);

        let (icon, status_text) = match status.status {
            CheckStatus::Pass => ("✓", "within budget".to_string()),
            CheckStatus::OverBudget => {
                let excess = status.violations.saturating_sub(status.budget);
                ("✗", format!("exceeded by {}", excess))
            }
        };

        println!("  Status: {} {}", icon, status_text);
        println!();
    }
}

/// JSONL output structure
#[derive(Debug, Serialize)]
struct JsonlRuleStatus {
    rule_id: String,
    source: String,
    description: String,
    languages: Vec<String>,
    severity: String,
    violations: u64,
    budget: u64,
    status: String,
}

/// Print JSONL output (one JSON object per line for each rule)
fn print_jsonl_output(statuses: &[RuleStatus]) {
    for status in statuses {
        let jsonl_status = JsonlRuleStatus {
            rule_id: status.rule_id.as_str().to_string(),
            source: status.source.as_str().to_string(),
            description: status.description.clone(),
            languages: status
                .languages
                .iter()
                .map(|l| format!("{:?}", l).to_lowercase())
                .collect(),
            severity: format!("{:?}", status.severity).to_lowercase(),
            violations: status.violations,
            budget: status.budget,
            status: status.status.as_str().to_string(),
        };

        if let Ok(json) = serde_json::to_string(&jsonl_status) {
            println!("{}", json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::aggregator::{AggregationResult, RuleRegionStatus};
    use crate::types::RegionPath;

    #[test]
    fn test_rule_source_as_str() {
        assert_eq!(RuleSource::Builtin.as_str(), "builtin");
        assert_eq!(RuleSource::Custom.as_str(), "custom");
    }

    #[test]
    fn test_check_status_as_str() {
        assert_eq!(CheckStatus::Pass.as_str(), "pass");
        assert_eq!(CheckStatus::OverBudget.as_str(), "over_budget");
    }

    #[test]
    fn test_jsonl_rule_status_serialization() {
        let status = JsonlRuleStatus {
            rule_id: "no-unwrap".to_string(),
            source: "builtin".to_string(),
            description: "Disallow .unwrap() calls".to_string(),
            languages: vec!["rust".to_string()],
            severity: "error".to_string(),
            violations: 5,
            budget: 10,
            status: "pass".to_string(),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("no-unwrap"));
        assert!(json.contains("builtin"));
        assert!(json.contains("\"violations\":5"));
        assert!(json.contains("\"budget\":10"));
    }

    #[test]
    fn test_build_rule_statuses_empty() {
        let rule_metadata = vec![];
        let counts = CountsManager::new();
        let aggregation_result = AggregationResult {
            statuses: vec![],
            passed: true,
            total_violations: 0,
            violations_over_budget: 0,
        };

        let statuses = build_rule_statuses(&rule_metadata, &counts, &aggregation_result);
        assert_eq!(statuses.len(), 0);
    }

    #[test]
    fn test_build_rule_statuses_single_rule_within_budget() {
        let rule_id = RuleId::new("test-rule").unwrap();
        let rule_metadata = vec![RuleMetadata {
            rule_id: rule_id.clone(),
            description: "Test rule".to_string(),
            languages: vec![Language::Rust],
            severity: Severity::Warning,
        }];

        let mut counts = CountsManager::new();
        counts.set_count(&rule_id, &RegionPath::new("."), 10);

        let aggregation_result = AggregationResult {
            statuses: vec![RuleRegionStatus {
                rule_id: rule_id.clone(),
                region: RegionPath::new("."),
                actual_count: 5,
                budget: 10,
                passed: true,
                violations: vec![],
            }],
            passed: true,
            total_violations: 5,
            violations_over_budget: 0,
        };

        let statuses = build_rule_statuses(&rule_metadata, &counts, &aggregation_result);
        assert_eq!(statuses.len(), 1);

        let status = &statuses[0];
        assert_eq!(status.rule_id.as_str(), "test-rule");
        assert_eq!(status.violations, 5);
        assert_eq!(status.budget, 10);
        assert_eq!(status.status, CheckStatus::Pass);
    }

    #[test]
    fn test_build_rule_statuses_single_rule_over_budget() {
        let rule_id = RuleId::new("test-rule").unwrap();
        let rule_metadata = vec![RuleMetadata {
            rule_id: rule_id.clone(),
            description: "Test rule".to_string(),
            languages: vec![Language::Rust],
            severity: Severity::Error,
        }];

        let mut counts = CountsManager::new();
        counts.set_count(&rule_id, &RegionPath::new("."), 5);

        let aggregation_result = AggregationResult {
            statuses: vec![RuleRegionStatus {
                rule_id: rule_id.clone(),
                region: RegionPath::new("."),
                actual_count: 10,
                budget: 5,
                passed: false,
                violations: vec![],
            }],
            passed: false,
            total_violations: 10,
            violations_over_budget: 5,
        };

        let statuses = build_rule_statuses(&rule_metadata, &counts, &aggregation_result);
        assert_eq!(statuses.len(), 1);

        let status = &statuses[0];
        assert_eq!(status.rule_id.as_str(), "test-rule");
        assert_eq!(status.violations, 10);
        assert_eq!(status.budget, 5);
        assert_eq!(status.status, CheckStatus::OverBudget);
    }

    #[test]
    fn test_build_rule_statuses_multiple_rules() {
        let rule1_id = RuleId::new("rule-1").unwrap();
        let rule2_id = RuleId::new("rule-2").unwrap();

        let rule_metadata = vec![
            RuleMetadata {
                rule_id: rule1_id.clone(),
                description: "First rule".to_string(),
                languages: vec![Language::Rust],
                severity: Severity::Warning,
            },
            RuleMetadata {
                rule_id: rule2_id.clone(),
                description: "Second rule".to_string(),
                languages: vec![Language::Python],
                severity: Severity::Error,
            },
        ];

        let mut counts = CountsManager::new();
        counts.set_count(&rule1_id, &RegionPath::new("."), 10);
        counts.set_count(&rule2_id, &RegionPath::new("."), 5);

        let aggregation_result = AggregationResult {
            statuses: vec![
                RuleRegionStatus {
                    rule_id: rule1_id.clone(),
                    region: RegionPath::new("."),
                    actual_count: 8,
                    budget: 10,
                    passed: true,
                    violations: vec![],
                },
                RuleRegionStatus {
                    rule_id: rule2_id.clone(),
                    region: RegionPath::new("."),
                    actual_count: 7,
                    budget: 5,
                    passed: false,
                    violations: vec![],
                },
            ],
            passed: false,
            total_violations: 15,
            violations_over_budget: 2,
        };

        let statuses = build_rule_statuses(&rule_metadata, &counts, &aggregation_result);
        assert_eq!(statuses.len(), 2);

        // Should be sorted by rule_id
        assert_eq!(statuses[0].rule_id.as_str(), "rule-1");
        assert_eq!(statuses[0].status, CheckStatus::Pass);

        assert_eq!(statuses[1].rule_id.as_str(), "rule-2");
        assert_eq!(statuses[1].status, CheckStatus::OverBudget);
    }

    #[test]
    fn test_build_rule_statuses_rule_with_no_violations() {
        let rule_id = RuleId::new("unused-rule").unwrap();
        let rule_metadata = vec![RuleMetadata {
            rule_id: rule_id.clone(),
            description: "Unused rule".to_string(),
            languages: vec![Language::Rust],
            severity: Severity::Info,
        }];

        let mut counts = CountsManager::new();
        counts.set_count(&rule_id, &RegionPath::new("."), 0);

        let aggregation_result = AggregationResult {
            statuses: vec![],
            passed: true,
            total_violations: 0,
            violations_over_budget: 0,
        };

        let statuses = build_rule_statuses(&rule_metadata, &counts, &aggregation_result);
        assert_eq!(statuses.len(), 1);

        let status = &statuses[0];
        assert_eq!(status.violations, 0);
        assert_eq!(status.budget, 0);
        assert_eq!(status.status, CheckStatus::Pass);
    }

    #[test]
    fn test_determine_rule_source() {
        let rule_id = RuleId::new("test-rule").unwrap();
        // Currently always returns Builtin - this is a placeholder
        assert_eq!(determine_rule_source(&rule_id), RuleSource::Builtin);
    }
}
