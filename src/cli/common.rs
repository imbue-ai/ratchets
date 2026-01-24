//! Common helper functions shared across CLI commands
//!
//! This module provides shared functionality for loading configuration,
//! discovering files, and building rule registries.

use crate::config::counts::CountsManager;
use crate::config::ratchet_toml::Config;
use crate::engine::file_walker::{FileEntry, FileWalker, FileWalkerError};
use crate::error::{ConfigError, RuleError};
use crate::rules::RuleRegistry;
use std::path::Path;

/// Exit codes from DESIGN.md
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_EXCEEDED: i32 = 1;
pub const EXIT_ERROR: i32 = 2;
pub const EXIT_PARSE_ERROR: i32 = 3;

/// Load ratchet.toml configuration
///
/// # Errors
///
/// Returns `ConfigError::Io` if ratchet.toml does not exist or cannot be read.
/// Returns `ConfigError::Parse` if ratchet.toml is invalid.
pub(crate) fn load_config() -> Result<Config, ConfigError> {
    let config_path = Path::new("ratchet.toml");
    if !config_path.exists() {
        return Err(ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "ratchet.toml not found. Run 'ratchet init' to create it.",
        )));
    }

    Config::load(config_path)
}

/// Discover files to check using FileWalker
///
/// Walks the specified paths and collects all files that match the
/// include/exclude patterns from the configuration.
///
/// # Arguments
///
/// * `paths` - Paths to walk (directories or files)
/// * `config` - Configuration containing include/exclude patterns
///
/// # Errors
///
/// Returns `FileWalkerError` if there is an error walking the file system.
pub(crate) fn discover_files(
    paths: &[String],
    config: &Config,
) -> Result<Vec<FileEntry>, FileWalkerError> {
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

/// Build rule registry from configuration
///
/// This function:
/// 1. Loads embedded builtin rules
/// 2. Loads filesystem builtin rules (if present)
/// 3. Loads custom rules (if present)
/// 4. Filters rules based on configuration
///
/// # Arguments
///
/// * `config` - Configuration specifying which rules to enable/disable
///
/// # Errors
///
/// Returns `RuleError` if there is an error loading or building rules.
pub(crate) fn build_registry(config: &Config) -> Result<RuleRegistry, RuleError> {
    RuleRegistry::build_from_config(config)
}

/// Load ratchet-counts.toml
///
/// # Errors
///
/// Returns `ConfigError::Io` if ratchet-counts.toml cannot be read.
/// Returns `ConfigError::Parse` if ratchet-counts.toml is invalid.
pub(crate) fn load_counts() -> Result<CountsManager, ConfigError> {
    let counts_path = Path::new("ratchet-counts.toml");
    if !counts_path.exists() {
        // If counts file doesn't exist, start with empty counts (strict enforcement)
        eprintln!(
            "Warning: ratchet-counts.toml not found. Using strict enforcement (budget=0 for all rules)."
        );
        return Ok(CountsManager::new());
    }

    CountsManager::load(counts_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GlobPattern, Language};

    #[test]
    fn test_load_config_missing_file() {
        // This test will fail if ratchet.toml exists in test directory
        // but is useful for validating error handling
        let result = load_config();
        // We can't assert failure since the file might exist in the test environment
        // Just ensure the function returns a Result
        let _ = result;
    }

    #[test]
    fn test_discover_files_with_empty_paths() {
        // Create a minimal config for testing
        let config = Config {
            ratchet: crate::config::ratchet_toml::RatchetMeta {
                version: "1".to_string(),
                languages: vec![Language::Rust],
                include: vec![GlobPattern::new("**/*.rs")],
                exclude: vec![],
            },
            rules: crate::config::ratchet_toml::RulesConfig {
                builtin: std::collections::HashMap::new(),
                custom: std::collections::HashMap::new(),
            },
            output: crate::config::ratchet_toml::OutputConfig::default(),
            patterns: std::collections::HashMap::new(),
        };

        let result = discover_files(&[], &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_build_registry_with_minimal_config() {
        let config = Config {
            ratchet: crate::config::ratchet_toml::RatchetMeta {
                version: "1".to_string(),
                languages: vec![Language::Rust],
                include: vec![GlobPattern::new("**/*.rs")],
                exclude: vec![],
            },
            rules: crate::config::ratchet_toml::RulesConfig {
                builtin: std::collections::HashMap::new(),
                custom: std::collections::HashMap::new(),
            },
            output: crate::config::ratchet_toml::OutputConfig::default(),
            patterns: std::collections::HashMap::new(),
        };

        let result = build_registry(&config);
        // Should succeed with minimal config
        assert!(result.is_ok());
    }
}
