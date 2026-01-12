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
use crate::config::counts::CountsManager;
use crate::config::ratchet_toml::Config;
use crate::engine::aggregator::ViolationAggregator;
use crate::engine::executor::ExecutionEngine;
use crate::engine::file_walker::FileWalker;
use crate::error::ConfigError;
use crate::rules::RuleRegistry;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Exit codes from DESIGN.md
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_EXCEEDED: i32 = 1;
pub const EXIT_ERROR: i32 = 2;
pub const EXIT_PARSE_ERROR: i32 = 3;

/// Error type specific to check command
#[derive(Debug, thiserror::Error)]
enum CheckError {
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
/// - 3: Parse error (in source file)
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
                _ => EXIT_ERROR,
            }
        }
    }
}

/// Internal implementation of check command
fn run_check_inner(paths: &[String], format: OutputFormat) -> Result<bool, CheckError> {
    // 1. Load ratchet.toml config
    let config = load_config()?;

    // 2. Load ratchet-counts.toml
    let counts = load_counts()?;

    // 3. Build rule registry (load builtin + custom rules)
    let mut registry = build_rule_registry()?;

    // 4. Filter rules by config
    registry.filter_by_config(&config.rules);

    // If no rules are enabled, warn and exit successfully
    if registry.is_empty() {
        eprintln!("Warning: No rules are enabled. Nothing to check.");
        return Ok(true);
    }

    // 5. Discover files using FileWalker
    let files = discover_files(paths, &config)?;

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

/// Load ratchet.toml configuration
fn load_config() -> Result<Config, CheckError> {
    let config_path = Path::new("ratchet.toml");
    if !config_path.exists() {
        return Err(CheckError::Other(
            "ratchet.toml not found. Run 'ratchet init' to create it.".to_string(),
        ));
    }

    Ok(Config::load(config_path)?)
}

/// Load ratchet-counts.toml
fn load_counts() -> Result<CountsManager, CheckError> {
    let counts_path = Path::new("ratchet-counts.toml");
    if !counts_path.exists() {
        // If counts file doesn't exist, start with empty counts (strict enforcement)
        eprintln!(
            "Warning: ratchet-counts.toml not found. Using strict enforcement (budget=0 for all rules)."
        );
        return Ok(CountsManager::new());
    }

    Ok(CountsManager::load(counts_path)?)
}

/// Build rule registry with all builtin and custom rules
fn build_rule_registry() -> Result<RuleRegistry, CheckError> {
    let mut registry = RuleRegistry::new();

    // Load builtin regex rules from builtin-ratchets/regex/
    let builtin_regex_dir = PathBuf::from("builtin-ratchets").join("regex");
    if builtin_regex_dir.exists() {
        registry.load_builtin_regex_rules(&builtin_regex_dir)?;
    }

    // Load builtin AST rules from builtin-ratchets/ast/
    let builtin_ast_dir = PathBuf::from("builtin-ratchets").join("ast");
    if builtin_ast_dir.exists() {
        registry.load_builtin_ast_rules(&builtin_ast_dir)?;
    }

    // Load custom regex rules from ratchets/regex/
    let custom_regex_dir = PathBuf::from("ratchets").join("regex");
    if custom_regex_dir.exists() {
        registry.load_custom_regex_rules(&custom_regex_dir)?;
    }

    // Load custom AST rules from ratchets/ast/
    let custom_ast_dir = PathBuf::from("ratchets").join("ast");
    if custom_ast_dir.exists() {
        registry.load_custom_ast_rules(&custom_ast_dir)?;
    }

    Ok(registry)
}

/// Discover files to check using FileWalker
fn discover_files(
    paths: &[String],
    config: &Config,
) -> Result<Vec<crate::engine::file_walker::FileEntry>, CheckError> {
    let mut all_files = Vec::new();

    for path_str in paths {
        let path = Path::new(path_str);

        // Create FileWalker with include/exclude patterns from config
        let walker = FileWalker::new(path, &config.ratchet.include, &config.ratchet.exclude)?;

        // Collect files from this path
        for result in walker.walk() {
            let file = result?;
            all_files.push(file);
        }
    }

    Ok(all_files)
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
