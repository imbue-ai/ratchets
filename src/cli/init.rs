//! Initialize a ratchet project
//!
//! Creates the necessary configuration files and directories for a new ratchet project.

use std::fs;
use std::path::Path;

/// Default content for ratchets.toml
const DEFAULT_RATCHET_TOML: &str = r#"[ratchets]
version = "1"

# Languages to enable (uncomment as needed)
# languages = ["rust", "typescript", "javascript", "python", "go"]

# File patterns to include (defaults to all)
# include = ["src/**", "tests/**"]

# File patterns to exclude
# exclude = ["**/generated/**"]

[rules]
# Built-in rules are enabled by default
# Disable a rule: rule-name = false
# Configure a rule: rule-name = { severity = "warning" }

[output]
format = "human"
"#;

/// Default content for ratchet-counts.toml
const DEFAULT_COUNTS_TOML: &str = r#"# Ratchet violation budgets
# These counts represent the maximum tolerated violations.
# Counts can only be reduced (tightened) or explicitly bumped with justification.

# Example:
# [no-unwrap]
# "." = 0
# "src/legacy" = 15
"#;

/// Error type for init command
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Path error
    #[error("Path error: {0}")]
    Path(String),
}

/// Result of init command
#[derive(Debug, PartialEq, Eq)]
pub struct InitResult {
    /// Files that were created
    pub created: Vec<String>,
    /// Files that were skipped (already existed)
    pub skipped: Vec<String>,
    /// Files that were overwritten
    pub overwritten: Vec<String>,
}

impl InitResult {
    /// Create a new empty InitResult
    fn new() -> Self {
        Self {
            created: Vec::new(),
            skipped: Vec::new(),
            overwritten: Vec::new(),
        }
    }
}

/// Run the init command
///
/// Creates the following files and directories:
/// - ratchets.toml (main configuration)
/// - ratchet-counts.toml (violation budgets)
/// - ratchets/regex/ (directory for custom regex rules)
/// - ratchets/ast/ (directory for custom AST rules)
///
/// # Arguments
/// * `force` - If true, overwrite existing files. If false, skip existing files.
///
/// # Returns
/// * `Ok(InitResult)` - Summary of created/skipped/overwritten files
/// * `Err(InitError)` - If an I/O error occurred
pub fn run_init(force: bool) -> Result<InitResult, InitError> {
    let mut result = InitResult::new();

    // Create ratchets.toml
    handle_file(
        Path::new("ratchets.toml"),
        DEFAULT_RATCHET_TOML,
        force,
        &mut result,
    )?;

    // Create ratchet-counts.toml
    handle_file(
        Path::new("ratchet-counts.toml"),
        DEFAULT_COUNTS_TOML,
        force,
        &mut result,
    )?;

    // Create directories (always create if they don't exist)
    create_directory("ratchets/regex", &mut result)?;
    create_directory("ratchets/ast", &mut result)?;

    Ok(result)
}

/// Handle creation of a single file
fn handle_file(
    path: &Path,
    content: &str,
    force: bool,
    result: &mut InitResult,
) -> Result<(), InitError> {
    let path_str = path_to_string(path)?;

    if path.exists() {
        if force {
            fs::write(path, content)?;
            result.overwritten.push(path_str);
        } else {
            result.skipped.push(path_str);
        }
    } else {
        fs::write(path, content)?;
        result.created.push(path_str);
    }

    Ok(())
}

/// Create a directory if it doesn't exist
fn create_directory(path: &str, result: &mut InitResult) -> Result<(), InitError> {
    let dir_path = Path::new(path);

    if dir_path.exists() {
        if dir_path.is_dir() {
            // Already exists, nothing to do
            Ok(())
        } else {
            Err(InitError::Path(format!(
                "Path '{}' exists but is not a directory",
                path
            )))
        }
    } else {
        // Create parent directories if needed
        if let Some(parent) = dir_path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir(dir_path)?;
        result.created.push(format!("{}/", path));
        Ok(())
    }
}

/// Convert a path to a string representation
fn path_to_string(path: &Path) -> Result<String, InitError> {
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| InitError::Path(format!("Invalid UTF-8 in path: {:?}", path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Global mutex to ensure tests that change directory don't interfere with each other
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    /// Helper to run init in a temporary directory
    /// Returns the temp dir (which must be kept alive) and the result
    fn with_temp_dir<F, R>(f: F) -> R
    where
        F: FnOnce(&TempDir) -> R,
    {
        // Lock to prevent parallel execution
        let _guard = TEST_MUTEX.lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Change to temp directory
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Run the test function
        let result = f(&temp_dir);

        // Change back to original directory
        std::env::set_current_dir(&original_dir).unwrap();

        result
    }

    #[test]
    fn test_init_creates_all_files() {
        with_temp_dir(|temp_dir| {
            let result = run_init(false).expect("init should succeed");

            // Check that all expected items were created
            assert!(result.created.contains(&"ratchets.toml".to_string()));
            assert!(result.created.contains(&"ratchet-counts.toml".to_string()));
            assert!(result.created.contains(&"ratchets/regex/".to_string()));
            assert!(result.created.contains(&"ratchets/ast/".to_string()));
            assert!(result.skipped.is_empty());
            assert!(result.overwritten.is_empty());

            // Check that files exist with correct content
            let ratchet_toml = temp_dir.path().join("ratchets.toml");
            assert!(ratchet_toml.exists());
            let content = fs::read_to_string(&ratchet_toml).unwrap();
            assert!(content.contains("[ratchets]"));
            assert!(content.contains("version = \"1\""));

            let counts_toml = temp_dir.path().join("ratchet-counts.toml");
            assert!(counts_toml.exists());
            let content = fs::read_to_string(&counts_toml).unwrap();
            assert!(content.contains("Ratchet violation budgets"));

            // Check that directories exist
            assert!(temp_dir.path().join("ratchets/regex").is_dir());
            assert!(temp_dir.path().join("ratchets/ast").is_dir());
        });
    }

    #[test]
    fn test_init_skips_existing_files_without_force() {
        with_temp_dir(|temp_dir| {
            // Create an existing file with different content
            fs::write("ratchets.toml", "existing content").unwrap();

            // Run init without force
            let result = run_init(false).expect("init should succeed");

            // Check that existing file was skipped
            assert!(result.skipped.contains(&"ratchets.toml".to_string()));
            assert!(!result.created.contains(&"ratchets.toml".to_string()));
            assert!(!result.overwritten.contains(&"ratchets.toml".to_string()));

            // Verify file content wasn't changed
            let content = fs::read_to_string(temp_dir.path().join("ratchets.toml")).unwrap();
            assert_eq!(content, "existing content");

            // Other files should still be created
            assert!(result.created.contains(&"ratchet-counts.toml".to_string()));
        });
    }

    #[test]
    fn test_init_overwrites_existing_files_with_force() {
        with_temp_dir(|temp_dir| {
            // Create existing files with different content
            fs::write("ratchets.toml", "old content").unwrap();
            fs::write("ratchet-counts.toml", "old counts").unwrap();

            // Run init with force
            let result = run_init(true).expect("init should succeed");

            // Check that existing files were overwritten
            assert!(result.overwritten.contains(&"ratchets.toml".to_string()));
            assert!(
                result
                    .overwritten
                    .contains(&"ratchet-counts.toml".to_string())
            );
            assert!(!result.skipped.contains(&"ratchets.toml".to_string()));
            assert!(!result.skipped.contains(&"ratchet-counts.toml".to_string()));

            // Verify file content was changed
            let content = fs::read_to_string(temp_dir.path().join("ratchets.toml")).unwrap();
            assert!(content.contains("[ratchets]"));
            assert_ne!(content, "old content");
        });
    }

    #[test]
    fn test_init_is_idempotent() {
        with_temp_dir(|_temp_dir| {
            // First run should create everything
            let result1 = run_init(false).expect("first init should succeed");
            assert_eq!(result1.created.len(), 4); // 2 files + 2 directories
            assert!(result1.skipped.is_empty());
            assert!(result1.overwritten.is_empty());

            // Second run should skip files but not list directories (they already exist)
            let result2 = run_init(false).expect("second init should succeed");
            assert!(result2.skipped.contains(&"ratchets.toml".to_string()));
            assert!(result2.skipped.contains(&"ratchet-counts.toml".to_string()));
            assert!(result2.created.is_empty());
            assert!(result2.overwritten.is_empty());
        });
    }

    #[test]
    fn test_init_creates_nested_directories() {
        with_temp_dir(|temp_dir| {
            let _result = run_init(false).expect("init should succeed");

            // Check that nested directories were created
            let regex_dir = temp_dir.path().join("ratchets/regex");
            let ast_dir = temp_dir.path().join("ratchets/ast");

            assert!(regex_dir.exists());
            assert!(regex_dir.is_dir());
            assert!(ast_dir.exists());
            assert!(ast_dir.is_dir());
        });
    }

    #[test]
    fn test_init_with_existing_directories() {
        with_temp_dir(|_temp_dir| {
            // Pre-create directories
            fs::create_dir_all("ratchets/regex").unwrap();
            fs::create_dir_all("ratchets/ast").unwrap();

            // Run init
            let result = run_init(false).expect("init should succeed");

            // Directories should not be reported as created (they already existed)
            assert!(!result.created.contains(&"ratchets/regex/".to_string()));
            assert!(!result.created.contains(&"ratchets/ast/".to_string()));

            // But files should be created
            assert!(result.created.contains(&"ratchets.toml".to_string()));
            assert!(result.created.contains(&"ratchet-counts.toml".to_string()));
        });
    }

    #[test]
    fn test_init_error_when_path_is_file_not_directory() {
        with_temp_dir(|_temp_dir| {
            // Create a file where a directory should be
            fs::create_dir("ratchets").unwrap();
            fs::write("ratchets/regex", "this is a file").unwrap();

            // Run init - should fail
            let result = run_init(false);

            // Should return an error
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("not a directory"));
        });
    }

    #[test]
    fn test_default_ratchet_toml_content() {
        assert!(DEFAULT_RATCHET_TOML.contains("[ratchets]"));
        assert!(DEFAULT_RATCHET_TOML.contains("version = \"1\""));
        assert!(DEFAULT_RATCHET_TOML.contains("[rules]"));
        assert!(DEFAULT_RATCHET_TOML.contains("[output]"));
        assert!(DEFAULT_RATCHET_TOML.contains("format = \"human\""));
    }

    #[test]
    fn test_default_counts_toml_content() {
        assert!(DEFAULT_COUNTS_TOML.contains("Ratchet violation budgets"));
        assert!(DEFAULT_COUNTS_TOML.contains("[no-unwrap]"));
        assert!(DEFAULT_COUNTS_TOML.contains("Example:"));
    }

    #[test]
    fn test_init_result_new() {
        let result = InitResult::new();
        assert!(result.created.is_empty());
        assert!(result.skipped.is_empty());
        assert!(result.overwritten.is_empty());
    }

    #[test]
    fn test_path_to_string() {
        let path = Path::new("test/path");
        let result = path_to_string(path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test/path");
    }
}
