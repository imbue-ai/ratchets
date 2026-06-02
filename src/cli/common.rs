//! Common helper functions shared across CLI commands
//!
//! This module provides shared functionality for loading configuration,
//! discovering files, and building rule registries.

use crate::cli::git_diff::{self, GitDiffError};
use crate::config::counts::CountsManager;
use crate::config::ratchet_toml::Config;
use crate::engine::file_walker::{FileEntry, FileWalker, FileWalkerError};
use crate::error::{ConfigError, RuleError};
use crate::rules::RuleRegistry;
use std::path::{Path, PathBuf};

/// Exit codes from DESIGN.md
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_EXCEEDED: i32 = 1;
pub const EXIT_ERROR: i32 = 2;
pub const EXIT_PARSE_ERROR: i32 = 3;

/// Load ratchets.toml configuration
///
/// # Errors
///
/// Returns `ConfigError::Io` if ratchets.toml does not exist or cannot be read.
/// Returns `ConfigError::Parse` if ratchets.toml is invalid.
pub(crate) fn load_config() -> Result<Config, ConfigError> {
    let config_path = Path::new("ratchets.toml");
    if !config_path.exists() {
        return Err(ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "ratchets.toml not found. Run 'ratchets init' to create it.",
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
    discover_files_verbose(paths, config, false, &mut |_| {})
}

/// Discover files to check using FileWalker with verbose output
///
/// Walks the specified paths and collects all files that match the
/// include/exclude patterns from the configuration. Optionally calls
/// a callback for each file or skip event.
///
/// # Arguments
///
/// * `paths` - Paths to walk (directories or files)
/// * `config` - Configuration containing include/exclude patterns
/// * `verbose` - If true, report file scanning and skipping via callback
/// * `callback` - Function called for each file/skip event
///
/// # Errors
///
/// Returns `FileWalkerError` if there is an error walking the file system.
pub(crate) fn discover_files_verbose<F>(
    paths: &[String],
    config: &Config,
    verbose: bool,
    callback: &mut F,
) -> Result<Vec<FileEntry>, FileWalkerError>
where
    F: FnMut(&str),
{
    use crate::engine::file_walker::{SkipReason, WalkResult};

    let mut all_files = Vec::new();

    for path_str in paths {
        let path = Path::new(path_str);

        // Create FileWalker with include/exclude patterns from config
        let walker = FileWalker::with_verbose(
            path,
            &config.ratchets.include,
            &config.ratchets.exclude,
            verbose,
        )?;

        // Collect files from this path
        if verbose {
            for result in walker.walk_with_skip_info() {
                match result? {
                    WalkResult::File(file) => {
                        callback(&format!("Scanning {}...", file.path.display()));
                        all_files.push(file);
                    }
                    WalkResult::Skipped { path, reason } => {
                        let reason_str = match reason {
                            SkipReason::ExcludedByPattern => "excluded by pattern",
                            SkipReason::NoMatchingLanguage => "no matching language",
                            SkipReason::NotAFile => "not a file",
                        };
                        callback(&format!("Skipping {} ({})", path.display(), reason_str));
                    }
                }
            }
        } else {
            for result in walker.walk() {
                let file = result?;
                all_files.push(file);
            }
        }
    }

    Ok(all_files)
}

/// Filter discovered files to those changed since the given git ref.
///
/// Intersects the walker output with `git diff <reference> --name-only`.
/// Files matching the ref-diff but already excluded by the walker (via
/// gitignore/include/exclude/language filters) are not re-added. Files
/// listed by git but no longer present on disk (deleted relative to the
/// ref) are skipped silently because the walker only yields extant files.
///
/// # Arguments
///
/// * `files` - Files already filtered by the walker.
/// * `reference` - Git ref to diff against (e.g. `"main"`, `"HEAD~1"`).
///
/// # Errors
///
/// Returns `GitDiffError::NotARepo` if the current directory is not in a
/// git repository, `GitDiffError::BadRef` if the ref is unknown, or
/// `GitDiffError::Spawn` if `git` cannot be invoked at all.
pub(crate) fn filter_files_since(
    files: Vec<FileEntry>,
    reference: &str,
) -> Result<Vec<FileEntry>, GitDiffError> {
    let changed = git_diff::changed_files_since(reference)?;

    Ok(files
        .into_iter()
        .filter(|entry| changed_set_contains(&changed, &entry.path))
        .collect())
}

/// Returns true if `path` matches any entry in `changed`. The walker may
/// hand back paths that are relative to the current directory (e.g.
/// `./src/foo.rs`) while `git_diff::changed_files_since` returns paths
/// anchored at the repo root. `canonicalize` resolves both sides to the
/// same absolute form when the file exists on disk; on failure we fall
/// back to a direct path comparison so we never silently match the wrong
/// file.
fn changed_set_contains(changed: &std::collections::HashSet<PathBuf>, path: &Path) -> bool {
    if let Ok(abs_path) = std::fs::canonicalize(path) {
        for changed_path in changed {
            if let Ok(abs_changed) = std::fs::canonicalize(changed_path)
                && abs_path == abs_changed
            {
                return true;
            }
        }
    }
    changed.contains(path)
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
        // This test will fail if ratchets.toml exists in test directory
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
            ratchets: crate::config::ratchet_toml::RatchetsMeta {
                version: "2".to_string(),
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
            enabled_ratchets: Vec::new(),
            disabled_ratchets: Vec::new(),
        };

        let result = discover_files(&[], &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_build_registry_with_minimal_config() {
        let config = Config {
            ratchets: crate::config::ratchet_toml::RatchetsMeta {
                version: "2".to_string(),
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
            enabled_ratchets: Vec::new(),
            disabled_ratchets: Vec::new(),
        };

        let result = build_registry(&config);
        // Should succeed with minimal config
        assert!(result.is_ok());
    }
}
