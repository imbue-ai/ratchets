//! Check command implementation
//!
//! This module implements the `ratchet check` command, which:
//! - Loads configuration from ratchets.toml
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
use crate::output::{HumanFormatter, JsonlFormatter};
use std::path::PathBuf;
use termcolor::ColorChoice;

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
/// * `verbose` - If true, show individual violation details. If false, show only summary.
///
/// # Returns
///
/// Exit code:
/// - 0: Success (all rules passed)
/// - 1: Exceeded (one or more rules exceeded budget)
/// - 2: Error (configuration/I/O error)
/// - 3: Parse error (invalid TOML configuration)
pub fn run_check(paths: &[String], format: OutputFormat, verbose: bool) -> i32 {
    match run_check_inner(paths, format, verbose) {
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
fn run_check_inner(
    paths: &[String],
    format: OutputFormat,
    verbose: bool,
) -> Result<bool, CheckError> {
    // 1. Load ratchets.toml config
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
    let files = if verbose {
        super::common::discover_files_verbose(paths, &config, true, &mut |msg| {
            eprintln!("{}", msg);
        })?
    } else {
        super::common::discover_files(paths, &config)?
    };

    if files.is_empty() {
        eprintln!("Warning: No files found to check.");
        return Ok(true);
    }

    // Print progress for human format (only if not verbose, since verbose already printed)
    if format == OutputFormat::Human && !verbose {
        eprintln!(
            "Checking {} files with {} rules...",
            files.len(),
            registry.len()
        );
    }

    // 6. Run ExecutionEngine with CountsManager for region resolution
    let engine = ExecutionEngine::new(registry, Some(std::sync::Arc::new(counts.clone())));
    let execution_result = engine.execute(files);

    // 7. Aggregate violations with ViolationAggregator
    let aggregator = ViolationAggregator::new(counts);
    let aggregation_result = aggregator.aggregate(execution_result.violations);

    // 8. Format and print output
    match format {
        OutputFormat::Human => {
            eprintln!(); // Blank line after "Checking..." message
            let formatter = HumanFormatter::new(ColorChoice::Auto);
            if let Err(e) = formatter.write_to_stdout(&aggregation_result, verbose) {
                eprintln!("Error writing output: {}", e);
            }
        }
        OutputFormat::Jsonl => {
            let formatter = JsonlFormatter::new();
            print!("{}", formatter.format(&aggregation_result, verbose));
        }
    }

    // 9. Return pass/fail status
    Ok(aggregation_result.passed)
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
}
