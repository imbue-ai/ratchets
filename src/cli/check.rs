//! Check command implementation
//!
//! This module implements the `ratchet check` command, which:
//! - Loads configuration from ratchet.toml
//! - Loads violation budgets from ratchet-counts.toml
//! - Discovers files to check
//! - Executes all enabled rules in parallel
//! - Aggregates violations and checks against budgets
//! - Formats output (human or JSONL)
//! - Returns appropriate exit code

use crate::cli::args::OutputFormat;
use crate::cli::common::{EXIT_ERROR, EXIT_EXCEEDED, EXIT_PARSE_ERROR, EXIT_SUCCESS};
use crate::engine::aggregator::ViolationAggregator;
use crate::engine::executor::ExecutionEngine;
use crate::error::ConfigError;
use serde::Serialize;
use std::path::PathBuf;

/// Error type specific to check command
#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Rule error: {0}")]
    Rule(#[from] crate::error::RuleError),

    #[error("File walker error: {0}")]
    FileWalker(#[from] crate::engine::file_walker::FileWalkerError),

    #[error("Parse error in {file}: {message}")]
    #[allow(dead_code)] // Reserved for future use when we detect parse errors
    Parse { file: PathBuf, message: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    #[allow(dead_code)] // Reserved for future use
    Other(String),
}

/// Run the check command
///
/// This is the main entry point for the check command. It coordinates
/// all the components and returns an appropriate exit code.
///
/// # Arguments
///
/// * `paths` - Paths to check (defaults to current directory)
/// * `format` - Output format (human or JSONL)
///
/// # Returns
///
/// Exit code:
/// - 0: Success (all rules passed)
/// - 1: Exceeded (one or more rules exceeded budget)
/// - 2: Error (configuration/I/O error)
/// - 3: Parse error (invalid TOML configuration)
pub fn run_check(paths: &[String], format: OutputFormat) -> i32 {
    match run_check_inner(paths, format) {
        Ok(passed) => {
            if passed {
                EXIT_SUCCESS
            } else {
                EXIT_EXCEEDED
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            // Determine exit code based on error type
            match e {
                CheckError::Parse { .. } => EXIT_PARSE_ERROR,
                // TOML parse errors should return EXIT_PARSE_ERROR
                CheckError::Config(ConfigError::Parse(_)) => EXIT_PARSE_ERROR,
                _ => EXIT_ERROR,
            }
        }
    }
}

/// Internal implementation of check command
fn run_check_inner(paths: &[String], format: OutputFormat) -> Result<bool, CheckError> {
    // 1. Load ratchet.toml config
    let config = super::common::load_config()?;

    // 2. Load ratchet-counts.toml
    let counts = super::common::load_counts()?;

    // 3. Build rule registry (load builtin + custom rules, apply config filter)
    let registry = super::common::build_registry(&config)?;

    // If no rules are enabled, warn and exit successfully
    if registry.is_empty() {
        eprintln!("Warning: No rules are enabled. Nothing to check.");
        return Ok(true);
    }

    // 5. Discover files using FileWalker
    let files = super::common::discover_files(paths, &config)?;

    if files.is_empty() {
        eprintln!("Warning: No files found to check.");
        return Ok(true);
    }

    // Print progress for human format
    if format == OutputFormat::Human {
        eprintln!(
            "Checking {} files with {} rules...",
            files.len(),
            registry.len()
        );
    }

    // 6. Run ExecutionEngine
    let engine = ExecutionEngine::new(registry);
    let execution_result = engine.execute(files);

    // 7. Aggregate violations with ViolationAggregator
    let aggregator = ViolationAggregator::new(counts);
    let aggregation_result = aggregator.aggregate(execution_result.violations);

    // 8. Format and print output
    match format {
        OutputFormat::Human => print_human_output(&aggregation_result),
        OutputFormat::Jsonl => print_jsonl_output(&aggregation_result),
    }

    // 9. Return pass/fail status
    Ok(aggregation_result.passed)
}

/// Print human-readable output
fn print_human_output(result: &crate::engine::aggregator::AggregationResult) {
    // Print violations grouped by rule and region
    if !result.statuses.is_empty() {
        eprintln!(); // Blank line after "Checking..." message

        for status in &result.statuses {
            // Only print violations if there are any
            if !status.violations.is_empty() {
                for violation in &status.violations {
                    eprintln!(
                        "{}: {}:{}:{} - {}",
                        status.rule_id.as_str(),
                        violation.file.display(),
                        violation.line,
                        violation.column,
                        violation.message
                    );
                }
            }
        }

        eprintln!(); // Blank line before results summary
    }

    // Print summary of results
    eprintln!("Results:");
    if result.statuses.is_empty() {
        eprintln!("  No violations found.");
    } else {
        for status in &result.statuses {
            let status_icon = if status.passed { "✓" } else { "✗" };
            let status_text = if status.passed {
                "".to_string()
            } else {
                " exceeded".to_string()
            };

            eprintln!(
                "  {}: {} violations (budget: {}) {}{}",
                status.rule_id.as_str(),
                status.actual_count,
                status.budget,
                status_icon,
                status_text
            );
        }
    }

    eprintln!(); // Blank line before final status

    // Print final pass/fail status
    if result.passed {
        eprintln!("Check PASSED: All rules within budget");
    } else {
        let num_failed = result.statuses.iter().filter(|s| !s.passed).count();
        eprintln!("Check FAILED: {} rule(s) exceeded budget", num_failed);
    }
}

/// JSONL violation output structure
#[derive(Debug, Serialize)]
struct JsonlViolation {
    rule_id: String,
    file: String,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    message: String,
    region: String,
}

/// Print JSONL output (one JSON object per line for each violation)
fn print_jsonl_output(result: &crate::engine::aggregator::AggregationResult) {
    for status in &result.statuses {
        for violation in &status.violations {
            let jsonl_violation = JsonlViolation {
                rule_id: violation.rule_id.as_str().to_string(),
                file: violation.file.display().to_string(),
                line: violation.line,
                column: violation.column,
                end_line: violation.end_line,
                end_column: violation.end_column,
                message: violation.message.clone(),
                region: violation.region.as_str().to_string(),
            };

            // Print each violation as a JSON line
            if let Ok(json) = serde_json::to_string(&jsonl_violation) {
                println!("{}", json);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_codes() {
        assert_eq!(EXIT_SUCCESS, 0);
        assert_eq!(EXIT_EXCEEDED, 1);
        assert_eq!(EXIT_ERROR, 2);
        assert_eq!(EXIT_PARSE_ERROR, 3);
    }

    #[test]
    fn test_check_error_display() {
        let err = CheckError::Other("test error".to_string());
        assert_eq!(err.to_string(), "test error");
    }

    #[test]
    fn test_jsonl_violation_serialization() {
        let violation = JsonlViolation {
            rule_id: "no-unwrap".to_string(),
            file: "src/main.rs".to_string(),
            line: 10,
            column: 5,
            end_line: 10,
            end_column: 15,
            message: "Avoid using .unwrap()".to_string(),
            region: "src".to_string(),
        };

        let json = serde_json::to_string(&violation).unwrap();
        assert!(json.contains("no-unwrap"));
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("10"));
    }
}
