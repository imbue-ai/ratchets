//! Initialize a ratchet project
//!
//! Creates the necessary configuration files and directories for a new ratchet project.

use std::fs;
use std::path::Path;

/// Default content for ratchets.toml
///
/// `[ratchets].version = "2"` is the only accepted schema. `enabled_ratchets`
/// entries are either `"$set-name"` (ratchet-set) or `"rule-id"` (single rule)
/// references; `disabled_ratchets` wins over `enabled_ratchets`. `[rules]`
/// carries only per-rule severity / regions settings.
const DEFAULT_RATCHET_TOML: &str = r#"# Ratchets v2 configuration scaffold.
#
# enabled_ratchets / disabled_ratchets accept two reference shapes:
#   - "$set-name" — a ratchet-set (group of rules) reference.
#   - "rule-id"   — a single rule reference.
#
# The only set that ships with this binary today is `$common-starter`.
# Per-language starter sets are planned as follow-ups. To opt in to the
# common cross-language rules write:
#   enabled_ratchets = ["$common-starter"]
enabled_ratchets = []
# disabled_ratchets always wins over enabled_ratchets at resolution time.
disabled_ratchets = []

[ratchets]
version = "2"

# Languages to enable (uncomment as needed)
# languages = ["rust", "typescript", "javascript", "python", "go"]

# File patterns to include (defaults to all)
# include = ["src/**", "tests/**"]

# File patterns to exclude
# exclude = ["**/generated/**"]

# Per-rule settings (severity / regions) live in [rules]. Enable / disable
# moved out of [rules] — use enabled_ratchets / disabled_ratchets above.
[rules]

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

    /// An existing `ratchets.toml` declares the v1 schema and `--force` was not
    /// given. The CLI dispatcher (`main.rs`) renders the embedded upgrade notice
    /// when it sees this variant and exits with the standard error code.
    #[error(
        "ratchets.toml already exists with version = \"1\". Migrate to v2 (see the upgrade notice above) or re-run with --force to overwrite."
    )]
    ExistingV1Config,
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
    // Without `--force`, an existing v1 `ratchets.toml` surfaces
    // `InitError::ExistingV1Config` so the dispatcher can print the upgrade
    // notice. `--force` overwrites it so a half-migrated repo can re-scaffold.
    if !force && existing_ratchets_toml_is_v1(Path::new("ratchets.toml"))? {
        return Err(InitError::ExistingV1Config);
    }

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

/// Returns `true` if `path` exists and its `[ratchets].version` is `"1"`.
///
/// A malformed `ratchets.toml` (or any version other than `"1"`) returns
/// `false` so the regular skip / overwrite flow handles those cases; a
/// parse error from a half-migrated file must not block `init --force`.
fn existing_ratchets_toml_is_v1(path: &Path) -> Result<bool, InitError> {
    if !path.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(path)?;
    let parsed: toml::Value = match toml::from_str(&content) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    let version = parsed
        .get("ratchets")
        .and_then(|table| table.get("version"))
        .and_then(|value| value.as_str());
    Ok(version == Some("1"))
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
    fn with_temp_dir<F, R>(f: F) -> Result<R, Box<dyn std::error::Error>>
    where
        F: FnOnce(&TempDir) -> Result<R, Box<dyn std::error::Error>>,
    {
        // Lock to prevent parallel execution
        let _guard = TEST_MUTEX.lock().map_err(|e| e.to_string())?;

        let temp_dir = TempDir::new()?;
        let original_dir = std::env::current_dir()?;

        // Change to temp directory
        std::env::set_current_dir(temp_dir.path())?;

        // Run the test function
        let result = f(&temp_dir);

        // Change back to original directory
        std::env::set_current_dir(&original_dir)?;

        result
    }

    #[test]
    fn test_init_creates_all_files() -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|temp_dir| {
            let result = run_init(false)?;

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
            let content = fs::read_to_string(&ratchet_toml)?;
            assert!(content.contains("[ratchets]"));
            assert!(content.contains("version = \"2\""));
            assert!(content.contains("enabled_ratchets = []"));

            let counts_toml = temp_dir.path().join("ratchet-counts.toml");
            assert!(counts_toml.exists());
            let content = fs::read_to_string(&counts_toml)?;
            assert!(content.contains("Ratchet violation budgets"));

            // Check that directories exist
            assert!(temp_dir.path().join("ratchets/regex").is_dir());
            assert!(temp_dir.path().join("ratchets/ast").is_dir());
            Ok(())
        })
    }

    #[test]
    fn test_init_skips_existing_files_without_force() -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|temp_dir| {
            // Create an existing file with different content
            fs::write("ratchets.toml", "existing content")?;

            // Run init without force
            let result = run_init(false)?;

            // Check that existing file was skipped
            assert!(result.skipped.contains(&"ratchets.toml".to_string()));
            assert!(!result.created.contains(&"ratchets.toml".to_string()));
            assert!(!result.overwritten.contains(&"ratchets.toml".to_string()));

            // Verify file content wasn't changed
            let content = fs::read_to_string(temp_dir.path().join("ratchets.toml"))?;
            assert_eq!(content, "existing content");

            // Other files should still be created
            assert!(result.created.contains(&"ratchet-counts.toml".to_string()));
            Ok(())
        })
    }

    #[test]
    fn test_init_overwrites_existing_files_with_force() -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|temp_dir| {
            // Create existing files with different content
            fs::write("ratchets.toml", "old content")?;
            fs::write("ratchet-counts.toml", "old counts")?;

            // Run init with force
            let result = run_init(true)?;

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
            let content = fs::read_to_string(temp_dir.path().join("ratchets.toml"))?;
            assert!(content.contains("[ratchets]"));
            assert_ne!(content, "old content");
            Ok(())
        })
    }

    #[test]
    fn test_init_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|_temp_dir| {
            // First run should create everything
            let result1 = run_init(false)?;
            assert_eq!(result1.created.len(), 4); // 2 files + 2 directories
            assert!(result1.skipped.is_empty());
            assert!(result1.overwritten.is_empty());

            // Second run should skip files but not list directories (they already exist)
            let result2 = run_init(false)?;
            assert!(result2.skipped.contains(&"ratchets.toml".to_string()));
            assert!(result2.skipped.contains(&"ratchet-counts.toml".to_string()));
            assert!(result2.created.is_empty());
            assert!(result2.overwritten.is_empty());
            Ok(())
        })
    }

    #[test]
    fn test_init_creates_nested_directories() -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|temp_dir| {
            let _result = run_init(false)?;

            // Check that nested directories were created
            let regex_dir = temp_dir.path().join("ratchets/regex");
            let ast_dir = temp_dir.path().join("ratchets/ast");

            assert!(regex_dir.exists());
            assert!(regex_dir.is_dir());
            assert!(ast_dir.exists());
            assert!(ast_dir.is_dir());
            Ok(())
        })
    }

    #[test]
    fn test_init_with_existing_directories() -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|_temp_dir| {
            // Pre-create directories
            fs::create_dir_all("ratchets/regex")?;
            fs::create_dir_all("ratchets/ast")?;

            // Run init
            let result = run_init(false)?;

            // Directories should not be reported as created (they already existed)
            assert!(!result.created.contains(&"ratchets/regex/".to_string()));
            assert!(!result.created.contains(&"ratchets/ast/".to_string()));

            // But files should be created
            assert!(result.created.contains(&"ratchets.toml".to_string()));
            assert!(result.created.contains(&"ratchet-counts.toml".to_string()));
            Ok(())
        })
    }

    #[test]
    fn test_init_error_when_path_is_file_not_directory() -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|_temp_dir| {
            // Create a file where a directory should be
            fs::create_dir("ratchets")?;
            fs::write("ratchets/regex", "this is a file")?;

            // Run init - should fail
            let result = run_init(false);

            // Should return an error
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("not a directory"));
            Ok(())
        })
    }

    #[test]
    fn test_default_ratchet_toml_content() {
        assert!(DEFAULT_RATCHET_TOML.contains("[ratchets]"));
        assert!(DEFAULT_RATCHET_TOML.contains("version = \"2\""));
        assert!(DEFAULT_RATCHET_TOML.contains("[rules]"));
        assert!(DEFAULT_RATCHET_TOML.contains("[output]"));
        assert!(DEFAULT_RATCHET_TOML.contains("format = \"human\""));
        assert!(DEFAULT_RATCHET_TOML.contains("enabled_ratchets = []"));
        assert!(DEFAULT_RATCHET_TOML.contains("disabled_ratchets = []"));
        assert!(DEFAULT_RATCHET_TOML.contains("$common-starter"));
        // The scaffolded template is user-facing and must not leak internal
        // bead IDs (e.g. `code-...`).
        assert!(!DEFAULT_RATCHET_TOML.contains("code-"));
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
    fn test_path_to_string() -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new("test/path");
        let result = path_to_string(path);
        assert!(result.is_ok());
        assert_eq!(result?, "test/path");
        Ok(())
    }

    #[test]
    fn test_existing_ratchets_toml_is_v1_returns_false_when_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|temp_dir| {
            let path = temp_dir.path().join("ratchets.toml");
            assert!(!existing_ratchets_toml_is_v1(&path)?);
            Ok(())
        })
    }

    #[test]
    fn test_existing_ratchets_toml_is_v1_detects_v1_version()
    -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|_temp_dir| {
            fs::write(
                "ratchets.toml",
                r#"[ratchets]
version = "1"
languages = ["rust"]
"#,
            )?;
            assert!(existing_ratchets_toml_is_v1(Path::new("ratchets.toml"))?);
            Ok(())
        })
    }

    #[test]
    fn test_existing_ratchets_toml_is_v1_rejects_v2_version()
    -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|_temp_dir| {
            fs::write(
                "ratchets.toml",
                r#"[ratchets]
version = "2"
languages = ["rust"]
"#,
            )?;
            assert!(!existing_ratchets_toml_is_v1(Path::new("ratchets.toml"))?);
            Ok(())
        })
    }

    #[test]
    fn test_existing_ratchets_toml_is_v1_rejects_malformed_toml()
    -> Result<(), Box<dyn std::error::Error>> {
        // Malformed TOML must not be classified as v1 — the regular skip path
        // handles those files.
        with_temp_dir(|_temp_dir| {
            fs::write("ratchets.toml", "= = =")?;
            assert!(!existing_ratchets_toml_is_v1(Path::new("ratchets.toml"))?);
            Ok(())
        })
    }

    #[test]
    fn test_existing_ratchets_toml_is_v1_rejects_missing_version_field()
    -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|_temp_dir| {
            fs::write(
                "ratchets.toml",
                r#"[ratchets]
languages = ["rust"]
"#,
            )?;
            assert!(!existing_ratchets_toml_is_v1(Path::new("ratchets.toml"))?);
            Ok(())
        })
    }

    #[test]
    fn test_run_init_errors_for_existing_v1_without_force() -> Result<(), Box<dyn std::error::Error>>
    {
        with_temp_dir(|_temp_dir| {
            fs::write(
                "ratchets.toml",
                r#"[ratchets]
version = "1"
languages = ["rust"]
"#,
            )?;
            let result = run_init(false);
            assert!(matches!(result, Err(InitError::ExistingV1Config)));
            Ok(())
        })
    }

    #[test]
    fn test_run_init_force_overwrites_existing_v1() -> Result<(), Box<dyn std::error::Error>> {
        with_temp_dir(|_temp_dir| {
            fs::write(
                "ratchets.toml",
                r#"[ratchets]
version = "1"
languages = ["rust"]
"#,
            )?;
            let result = run_init(true)?;
            assert!(result.overwritten.contains(&"ratchets.toml".to_string()));

            // Confirm the v2 scaffold replaced the v1 content.
            let content = fs::read_to_string("ratchets.toml")?;
            assert!(content.contains("version = \"2\""));
            Ok(())
        })
    }
}
