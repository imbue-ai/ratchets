//! Comprehensive CLI integration tests
//!
//! These tests verify all CLI commands and their behavior, including:
//! - init: Creates files, --force behavior
//! - check: Exit codes, output formats
//! - bump: Updates counts, auto-detect
//! - tighten: Reduces counts, fails on exceeded
//! - list: Output formats
//! - merge-driver: Minimum wins
//!
//! NOTE: These tests change the current directory and use std::sync::Mutex
//! to ensure they don't interfere with each other.

use ratchets::cli;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tempfile::TempDir;

// Global mutex to ensure tests that change directory don't interfere with each other
static TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Helper to run a test in an isolated temporary directory
fn with_temp_dir<F>(f: F)
where
    F: FnOnce(&TempDir),
{
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();
    f(&temp_dir);
    std::env::set_current_dir(&original_dir).unwrap();
}

/// Helper to create a basic test project structure
fn setup_basic_project(temp_dir: &Path) {
    // Create ratchets.toml
    let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
    fs::write(temp_dir.join("ratchets.toml"), config).unwrap();

    // Create ratchet-counts.toml
    let counts = r#"
[no-todo-comments]
"." = 5
"#;
    fs::write(temp_dir.join("ratchet-counts.toml"), counts).unwrap();

    // Create builtin-ratchets/regex directory with no-todo-comments rule
    let builtin_regex_dir = temp_dir
        .join("builtin-ratchets")
        .join("common")
        .join("regex");
    fs::create_dir_all(&builtin_regex_dir).unwrap();

    let rule_toml = r#"
[rule]
id = "no-todo-comments"
description = "Disallow TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
    fs::write(builtin_regex_dir.join("no-todo-comments.toml"), rule_toml).unwrap();

    // Create a test source file
    fs::write(temp_dir.join("test.rs"), "// TODO: test\nfn main() {}\n").unwrap();
}

// ============================================================================
// INIT COMMAND TESTS
// ============================================================================

#[test]
fn test_init_creates_all_files() {
    with_temp_dir(|temp_dir| {
        let result = cli::init::run_init(false).expect("init should succeed");

        // Check that all expected items were created
        assert!(result.created.contains(&"ratchets.toml".to_string()));
        assert!(result.created.contains(&"ratchet-counts.toml".to_string()));
        assert!(result.created.contains(&"ratchets/regex/".to_string()));
        assert!(result.created.contains(&"ratchets/ast/".to_string()));
        assert!(result.skipped.is_empty());
        assert!(result.overwritten.is_empty());

        // Verify files exist
        assert!(temp_dir.path().join("ratchets.toml").exists());
        assert!(temp_dir.path().join("ratchet-counts.toml").exists());
        assert!(temp_dir.path().join("ratchets/regex").is_dir());
        assert!(temp_dir.path().join("ratchets/ast").is_dir());
    });
}

#[test]
fn test_init_without_force_skips_existing() {
    with_temp_dir(|temp_dir| {
        // Create an existing file
        fs::write(temp_dir.path().join("ratchets.toml"), "existing content").unwrap();

        let result = cli::init::run_init(false).expect("init should succeed");

        // Existing file should be skipped
        assert!(result.skipped.contains(&"ratchets.toml".to_string()));
        assert!(!result.created.contains(&"ratchets.toml".to_string()));
        assert!(!result.overwritten.contains(&"ratchets.toml".to_string()));

        // Verify file wasn't changed
        let content = fs::read_to_string(temp_dir.path().join("ratchets.toml")).unwrap();
        assert_eq!(content, "existing content");
    });
}

#[test]
fn test_init_with_force_overwrites_existing() {
    with_temp_dir(|temp_dir| {
        // Create existing files
        fs::write(temp_dir.path().join("ratchets.toml"), "old content").unwrap();
        fs::write(temp_dir.path().join("ratchet-counts.toml"), "old counts").unwrap();

        let result = cli::init::run_init(true).expect("init should succeed");

        // Files should be overwritten
        assert!(result.overwritten.contains(&"ratchets.toml".to_string()));
        assert!(
            result
                .overwritten
                .contains(&"ratchet-counts.toml".to_string())
        );
        assert!(!result.skipped.contains(&"ratchets.toml".to_string()));

        // Verify file was changed
        let content = fs::read_to_string(temp_dir.path().join("ratchets.toml")).unwrap();
        assert_ne!(content, "old content");
        assert!(content.contains("[ratchets]"));
    });
}

#[test]
fn test_init_is_idempotent() {
    with_temp_dir(|_temp_dir| {
        // First init
        let result1 = cli::init::run_init(false).expect("first init should succeed");
        assert_eq!(result1.created.len(), 4); // 2 files + 2 directories

        // Second init should skip files
        let result2 = cli::init::run_init(false).expect("second init should succeed");
        assert!(result2.skipped.contains(&"ratchets.toml".to_string()));
        assert!(result2.skipped.contains(&"ratchet-counts.toml".to_string()));
        assert!(result2.created.is_empty());
        assert!(result2.overwritten.is_empty());
    });
}

// ============================================================================
// CHECK COMMAND TESTS
// ============================================================================

#[test]
fn test_check_returns_success_when_within_budget() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        let exit_code = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human, false);

        // Should pass: 1 TODO with budget of 5
        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_check_returns_exceeded_when_over_budget() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Update budget to 0
        let counts = r#"
[no-todo-comments]
"." = 0
"#;
        fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

        let exit_code = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human, false);

        // Should fail: 1 TODO with budget of 0
        assert_eq!(exit_code, cli::common::EXIT_EXCEEDED);
    });
}

#[test]
fn test_check_returns_error_when_config_missing() {
    with_temp_dir(|_temp_dir| {
        // Don't create any config files

        let exit_code = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human, false);

        // Should return error
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}

#[test]
fn test_check_jsonl_format_returns_success() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        let exit_code = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Jsonl, false);

        // Should pass with JSONL format
        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_check_with_multiple_paths() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Create subdirectories with files
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("src/lib.rs"),
            "// TODO: impl\nfn lib() {}",
        )
        .unwrap();

        let exit_code = cli::check::run_check(
            &["src".to_string(), ".".to_string()],
            cli::OutputFormat::Human,
            false,
        );

        // Should still be within budget (2 TODOs, budget 5)
        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_check_with_no_files_found() {
    with_temp_dir(|temp_dir| {
        // Create config but no source files
        let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

        let exit_code = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human, false);

        // Should succeed with warning (no files to check)
        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

// ============================================================================
// BUMP COMMAND TESTS
// ============================================================================

#[test]
fn test_bump_with_explicit_count() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Bump to explicit count
        let exit_code = cli::bump::run_bump(Some("no-todo-comments"), ".", Some(10), false);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify counts file was updated
        let counts_content =
            fs::read_to_string(temp_dir.path().join("ratchet-counts.toml")).unwrap();
        assert!(counts_content.contains("10"));
    });
}

#[test]
fn test_bump_with_auto_detect() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Bump with auto-detect (should set to current count of 1)
        let exit_code = cli::bump::run_bump(Some("no-todo-comments"), ".", None, false);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify counts file was updated to current count
        let counts_content =
            fs::read_to_string(temp_dir.path().join("ratchet-counts.toml")).unwrap();
        // Should be set to 1 (current violation count)
        assert!(counts_content.contains("\".\"")); // Region "." should exist
    });
}

#[test]
fn test_bump_rejects_count_below_current() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Try to bump to 0 (below current count of 1)
        let exit_code = cli::bump::run_bump(Some("no-todo-comments"), ".", Some(0), false);

        // Should fail
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}

#[test]
fn test_bump_with_custom_region() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Create src directory with files
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("src/lib.rs"),
            "// TODO: impl\nfn lib() {}",
        )
        .unwrap();

        // Bump the src region
        let exit_code = cli::bump::run_bump(Some("no-todo-comments"), "src", Some(5), false);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify counts file has the src region
        let counts_content =
            fs::read_to_string(temp_dir.path().join("ratchet-counts.toml")).unwrap();
        assert!(counts_content.contains("src"));
    });
}

#[test]
fn test_bump_invalid_rule_id() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Try to bump non-existent rule
        let exit_code = cli::bump::run_bump(Some("nonexistent-rule"), ".", Some(10), false);

        // Should fail
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}

#[test]
fn test_bump_missing_config() {
    with_temp_dir(|_temp_dir| {
        // Don't create config

        let exit_code = cli::bump::run_bump(Some("no-todo-comments"), ".", Some(10), false);

        // Should fail
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}

// ============================================================================
// TIGHTEN COMMAND TESTS
// ============================================================================

#[test]
fn test_tighten_reduces_budget_to_current() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Current count is 1, budget is 5 - should tighten to 1
        let exit_code = cli::tighten::run_tighten(None, None);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify budget was reduced
        let counts_content =
            fs::read_to_string(temp_dir.path().join("ratchet-counts.toml")).unwrap();
        // Budget should now be 1
        assert!(counts_content.contains("1"));
        assert!(!counts_content.contains("5"));
    });
}

#[test]
fn test_tighten_specific_rule() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        let exit_code = cli::tighten::run_tighten(Some("no-todo-comments"), None);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_tighten_specific_region() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        let exit_code = cli::tighten::run_tighten(None, Some("."));

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_tighten_fails_when_violations_exceed_budget() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Set budget to 0 (below current count of 1)
        let counts = r#"
[no-todo-comments]
"." = 0
"#;
        fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

        let exit_code = cli::tighten::run_tighten(None, None);

        // Should fail because violations exceed budget
        assert_eq!(exit_code, cli::common::EXIT_EXCEEDED);
    });
}

#[test]
fn test_tighten_no_changes_needed() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // First tighten to current
        let exit_code1 = cli::tighten::run_tighten(None, None);
        assert_eq!(exit_code1, cli::common::EXIT_SUCCESS);

        // Second tighten should have no changes
        let exit_code2 = cli::tighten::run_tighten(None, None);
        assert_eq!(exit_code2, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_tighten_invalid_rule_id() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        let exit_code = cli::tighten::run_tighten(Some("invalid rule!"), None);

        // Should fail with invalid rule ID
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}

#[test]
fn test_tighten_missing_config() {
    with_temp_dir(|_temp_dir| {
        let exit_code = cli::tighten::run_tighten(None, None);

        // Should fail
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}

// ============================================================================
// LIST COMMAND TESTS
// ============================================================================

#[test]
fn test_list_human_format() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        let exit_code = cli::list::run_list(cli::OutputFormat::Human);

        // Should succeed
        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_list_jsonl_format() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        let exit_code = cli::list::run_list(cli::OutputFormat::Jsonl);

        // Should succeed
        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_list_with_no_rules_enabled() {
    with_temp_dir(|temp_dir| {
        // Create config with all rules disabled
        let config = r#"
[ratchets]
version = "1"
languages = ["rust"]

[rules]
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

        let exit_code = cli::list::run_list(cli::OutputFormat::Human);

        // Should succeed (just shows empty list)
        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_list_missing_config() {
    with_temp_dir(|_temp_dir| {
        let exit_code = cli::list::run_list(cli::OutputFormat::Human);

        // Should fail
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}

// ============================================================================
// MERGE-DRIVER COMMAND TESTS
// ============================================================================

#[test]
fn test_merge_driver_minimum_wins() {
    with_temp_dir(|temp_dir| {
        // Create three versions with different counts
        let base = r#"
[no-todo-comments]
"." = 20
"#;
        fs::write(temp_dir.path().join("base.toml"), base).unwrap();

        let ours = r#"
[no-todo-comments]
"." = 15
"#;
        fs::write(temp_dir.path().join("ours.toml"), ours).unwrap();

        let theirs = r#"
[no-todo-comments]
"." = 18
"#;
        fs::write(temp_dir.path().join("theirs.toml"), theirs).unwrap();

        let exit_code =
            cli::merge_driver::run_merge_driver("base.toml", "ours.toml", "theirs.toml");

        assert_eq!(exit_code, 0); // EXIT_SUCCESS

        // Verify merged result has minimum (15)
        let merged = fs::read_to_string(temp_dir.path().join("ours.toml")).unwrap();
        assert!(merged.contains("15"));
        assert!(!merged.contains("18"));
        assert!(!merged.contains("20"));
    });
}

#[test]
fn test_merge_driver_new_rule_in_ours() {
    with_temp_dir(|temp_dir| {
        let base = "";
        fs::write(temp_dir.path().join("base.toml"), base).unwrap();

        let ours = r#"
[no-todo-comments]
"." = 10
"#;
        fs::write(temp_dir.path().join("ours.toml"), ours).unwrap();

        let theirs = "";
        fs::write(temp_dir.path().join("theirs.toml"), theirs).unwrap();

        let exit_code =
            cli::merge_driver::run_merge_driver("base.toml", "ours.toml", "theirs.toml");

        assert_eq!(exit_code, 0);

        // Verify new rule is preserved
        let merged = fs::read_to_string(temp_dir.path().join("ours.toml")).unwrap();
        assert!(merged.contains("no-todo-comments"));
        assert!(merged.contains("10"));
    });
}

#[test]
fn test_merge_driver_new_rule_in_theirs() {
    with_temp_dir(|temp_dir| {
        let base = "";
        fs::write(temp_dir.path().join("base.toml"), base).unwrap();

        let ours = "";
        fs::write(temp_dir.path().join("ours.toml"), ours).unwrap();

        let theirs = r#"
[no-unwrap]
"." = 5
"#;
        fs::write(temp_dir.path().join("theirs.toml"), theirs).unwrap();

        let exit_code =
            cli::merge_driver::run_merge_driver("base.toml", "ours.toml", "theirs.toml");

        assert_eq!(exit_code, 0);

        // Verify new rule from theirs is merged
        let merged = fs::read_to_string(temp_dir.path().join("ours.toml")).unwrap();
        assert!(merged.contains("no-unwrap"));
        assert!(merged.contains("5"));
    });
}

#[test]
fn test_merge_driver_multiple_rules() {
    with_temp_dir(|temp_dir| {
        let base = r#"
[no-unwrap]
"." = 20
[no-todo-comments]
"." = 30
"#;
        fs::write(temp_dir.path().join("base.toml"), base).unwrap();

        let ours = r#"
[no-unwrap]
"." = 15
[no-todo-comments]
"." = 30
"#;
        fs::write(temp_dir.path().join("ours.toml"), ours).unwrap();

        let theirs = r#"
[no-unwrap]
"." = 18
[no-todo-comments]
"." = 25
"#;
        fs::write(temp_dir.path().join("theirs.toml"), theirs).unwrap();

        let exit_code =
            cli::merge_driver::run_merge_driver("base.toml", "ours.toml", "theirs.toml");

        assert_eq!(exit_code, 0);

        // Verify minimum for each rule
        let merged = fs::read_to_string(temp_dir.path().join("ours.toml")).unwrap();
        assert!(merged.contains("no-unwrap"));
        assert!(merged.contains("no-todo-comments"));
        // no-unwrap: min(15, 18) = 15
        // no-todo-comments: min(30, 25) = 25
        // Check that both values are in the file
        let parsed = ratchets::config::counts::CountsManager::parse(&merged).unwrap();
        let no_unwrap = ratchets::types::RuleId::new("no-unwrap").unwrap();
        let no_todo = ratchets::types::RuleId::new("no-todo-comments").unwrap();
        assert_eq!(parsed.get_budget(&no_unwrap, Path::new(".")), 15);
        assert_eq!(parsed.get_budget(&no_todo, Path::new(".")), 25);
    });
}

#[test]
fn test_merge_driver_multiple_regions() {
    with_temp_dir(|temp_dir| {
        let base = r#"
[no-todo-comments]
"." = 20
"src" = 15
"#;
        fs::write(temp_dir.path().join("base.toml"), base).unwrap();

        let ours = r#"
[no-todo-comments]
"." = 18
"src" = 10
"#;
        fs::write(temp_dir.path().join("ours.toml"), ours).unwrap();

        let theirs = r#"
[no-todo-comments]
"." = 19
"src" = 12
"#;
        fs::write(temp_dir.path().join("theirs.toml"), theirs).unwrap();

        let exit_code =
            cli::merge_driver::run_merge_driver("base.toml", "ours.toml", "theirs.toml");

        assert_eq!(exit_code, 0);

        // Verify minimums for both regions
        let merged = fs::read_to_string(temp_dir.path().join("ours.toml")).unwrap();
        let parsed = ratchets::config::counts::CountsManager::parse(&merged).unwrap();
        let rule_id = ratchets::types::RuleId::new("no-todo-comments").unwrap();

        // Root: min(18, 19) = 18
        assert_eq!(parsed.get_budget(&rule_id, Path::new(".")), 18);
        // Src: min(10, 12) = 10
        assert_eq!(parsed.get_budget(&rule_id, Path::new("src/file.rs")), 10);
    });
}

#[test]
fn test_merge_driver_missing_files() {
    with_temp_dir(|temp_dir| {
        // Only create ours, base and theirs are missing
        let ours = r#"
[no-todo-comments]
"." = 10
"#;
        fs::write(temp_dir.path().join("ours.toml"), ours).unwrap();

        let exit_code = cli::merge_driver::run_merge_driver(
            "nonexistent_base.toml",
            "ours.toml",
            "nonexistent_theirs.toml",
        );

        // Should succeed (missing files treated as empty)
        assert_eq!(exit_code, 0);

        // Verify ours is preserved
        let merged = fs::read_to_string(temp_dir.path().join("ours.toml")).unwrap();
        assert!(merged.contains("no-todo-comments"));
        assert!(merged.contains("10"));
    });
}

#[test]
fn test_merge_driver_invalid_toml() {
    with_temp_dir(|temp_dir| {
        let base = "";
        fs::write(temp_dir.path().join("base.toml"), base).unwrap();

        let ours = "invalid [[ toml syntax";
        fs::write(temp_dir.path().join("ours.toml"), ours).unwrap();

        let theirs = "";
        fs::write(temp_dir.path().join("theirs.toml"), theirs).unwrap();

        let exit_code =
            cli::merge_driver::run_merge_driver("base.toml", "ours.toml", "theirs.toml");

        // Should fail with parse error
        assert_eq!(exit_code, 1); // EXIT_ERROR
    });
}

// ============================================================================
// EXIT CODE VERIFICATION TESTS
// ============================================================================

#[test]
fn test_exit_codes_are_correct() {
    // Verify the exit codes match the specification
    assert_eq!(cli::common::EXIT_SUCCESS, 0);
    assert_eq!(cli::common::EXIT_EXCEEDED, 1);
    assert_eq!(cli::common::EXIT_ERROR, 2);
    assert_eq!(cli::common::EXIT_PARSE_ERROR, 3);
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_check_with_empty_counts_file() {
    with_temp_dir(|temp_dir| {
        // Create config but empty counts file
        let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();
        fs::write(temp_dir.path().join("ratchet-counts.toml"), "").unwrap();

        // Create builtin rule
        let builtin_regex_dir = temp_dir
            .path()
            .join("builtin-ratchets")
            .join("common")
            .join("regex");
        fs::create_dir_all(&builtin_regex_dir).unwrap();
        let rule_toml = r#"
[rule]
id = "no-todo-comments"
description = "Disallow TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
        fs::write(builtin_regex_dir.join("no-todo-comments.toml"), rule_toml).unwrap();

        // Create file with TODO
        fs::write(temp_dir.path().join("test.rs"), "// TODO: test\n").unwrap();

        let exit_code = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human, false);

        // Should fail with empty counts (budget defaults to 0)
        assert_eq!(exit_code, cli::common::EXIT_EXCEEDED);
    });
}

#[test]
fn test_bump_creates_counts_file_if_missing() {
    with_temp_dir(|temp_dir| {
        setup_basic_project(temp_dir.path());

        // Remove counts file
        fs::remove_file(temp_dir.path().join("ratchet-counts.toml")).unwrap();

        let exit_code = cli::bump::run_bump(Some("no-todo-comments"), ".", Some(10), false);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify counts file was created
        assert!(temp_dir.path().join("ratchet-counts.toml").exists());
    });
}

#[test]
fn test_tighten_with_multiple_rules() {
    with_temp_dir(|temp_dir| {
        // Create config with multiple rules
        let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

        let counts = r#"
[no-todo-comments]
"." = 10
"#;
        fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

        // Create builtin rule
        let builtin_regex_dir = temp_dir
            .path()
            .join("builtin-ratchets")
            .join("common")
            .join("regex");
        fs::create_dir_all(&builtin_regex_dir).unwrap();
        let rule_toml = r#"
[rule]
id = "no-todo-comments"
description = "Disallow TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
        fs::write(builtin_regex_dir.join("no-todo-comments.toml"), rule_toml).unwrap();

        // Create file with violations
        fs::write(temp_dir.path().join("test.rs"), "// TODO: test\n").unwrap();

        let exit_code = cli::tighten::run_tighten(None, None);

        // Should succeed and tighten all rules
        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);
    });
}

// ============================================================================
// BUMP --ALL COMMAND TESTS
// ============================================================================

#[test]
fn test_bump_all_with_empty_initial_counts() {
    with_temp_dir(|temp_dir| {
        // Create config with multiple rules
        let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
no-fixme-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

        // Create empty counts file
        fs::write(temp_dir.path().join("ratchet-counts.toml"), "").unwrap();

        // Create builtin rules
        let builtin_regex_dir = temp_dir
            .path()
            .join("builtin-ratchets")
            .join("common")
            .join("regex");
        fs::create_dir_all(&builtin_regex_dir).unwrap();

        let todo_rule = r#"
[rule]
id = "no-todo-comments"
description = "Disallow TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
        fs::write(builtin_regex_dir.join("no-todo-comments.toml"), todo_rule).unwrap();

        let fixme_rule = r#"
[rule]
id = "no-fixme-comments"
description = "Disallow FIXME comments"
severity = "warning"

[match]
pattern = "FIXME"
"#;
        fs::write(builtin_regex_dir.join("no-fixme-comments.toml"), fixme_rule).unwrap();

        // Create files with violations
        fs::write(
            temp_dir.path().join("test.rs"),
            "// TODO: test\n// FIXME: fix\n",
        )
        .unwrap();

        // Run bump --all
        let exit_code = cli::bump::run_bump(None, ".", None, true);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify counts file was updated with current violation counts
        let counts_content =
            fs::read_to_string(temp_dir.path().join("ratchet-counts.toml")).unwrap();

        // Should have entries for both rules
        assert!(counts_content.contains("no-todo-comments"));
        assert!(counts_content.contains("no-fixme-comments"));

        // Parse and verify specific counts
        let counts = ratchets::config::counts::CountsManager::parse(&counts_content).unwrap();
        let todo_id = ratchets::types::RuleId::new("no-todo-comments").unwrap();
        let fixme_id = ratchets::types::RuleId::new("no-fixme-comments").unwrap();

        // Both rules should have budget set to 1 (current violation count)
        assert_eq!(counts.get_budget(&todo_id, Path::new(".")), 1);
        assert_eq!(counts.get_budget(&fixme_id, Path::new(".")), 1);
    });
}

#[test]
fn test_bump_all_with_existing_counts() {
    with_temp_dir(|temp_dir| {
        // Create config with multiple rules
        let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
no-fixme-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

        // Create counts file with existing budgets
        let counts = r#"
[no-todo-comments]
"." = 5

[no-fixme-comments]
"." = 3
"#;
        fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

        // Create builtin rules
        let builtin_regex_dir = temp_dir
            .path()
            .join("builtin-ratchets")
            .join("common")
            .join("regex");
        fs::create_dir_all(&builtin_regex_dir).unwrap();

        let todo_rule = r#"
[rule]
id = "no-todo-comments"
description = "Disallow TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
        fs::write(builtin_regex_dir.join("no-todo-comments.toml"), todo_rule).unwrap();

        let fixme_rule = r#"
[rule]
id = "no-fixme-comments"
description = "Disallow FIXME comments"
severity = "warning"

[match]
pattern = "FIXME"
"#;
        fs::write(builtin_regex_dir.join("no-fixme-comments.toml"), fixme_rule).unwrap();

        // Create files with violations (2 TODOs, 1 FIXME)
        fs::write(
            temp_dir.path().join("test.rs"),
            "// TODO: test\n// TODO: another\n// FIXME: fix\n",
        )
        .unwrap();

        // Run bump --all
        let exit_code = cli::bump::run_bump(None, ".", None, true);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify counts were updated to current violation counts
        let counts_content =
            fs::read_to_string(temp_dir.path().join("ratchet-counts.toml")).unwrap();
        let counts = ratchets::config::counts::CountsManager::parse(&counts_content).unwrap();
        let todo_id = ratchets::types::RuleId::new("no-todo-comments").unwrap();
        let fixme_id = ratchets::types::RuleId::new("no-fixme-comments").unwrap();

        // Budgets should be set to current violation counts (2 and 1)
        assert_eq!(counts.get_budget(&todo_id, Path::new(".")), 2);
        assert_eq!(counts.get_budget(&fixme_id, Path::new(".")), 1);
    });
}

#[test]
fn test_bump_all_no_violations() {
    with_temp_dir(|temp_dir| {
        // Create config with rules
        let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

        // Create counts file
        let counts = r#"
[no-todo-comments]
"." = 5
"#;
        fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

        // Create builtin rule
        let builtin_regex_dir = temp_dir
            .path()
            .join("builtin-ratchets")
            .join("common")
            .join("regex");
        fs::create_dir_all(&builtin_regex_dir).unwrap();

        let todo_rule = r#"
[rule]
id = "no-todo-comments"
description = "Disallow TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
        fs::write(builtin_regex_dir.join("no-todo-comments.toml"), todo_rule).unwrap();

        // Create file with NO violations
        fs::write(temp_dir.path().join("test.rs"), "fn main() {}\n").unwrap();

        // Run bump --all
        let exit_code = cli::bump::run_bump(None, ".", None, true);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify budget was set to 0 (no violations)
        let counts_content =
            fs::read_to_string(temp_dir.path().join("ratchet-counts.toml")).unwrap();
        let counts = ratchets::config::counts::CountsManager::parse(&counts_content).unwrap();
        let todo_id = ratchets::types::RuleId::new("no-todo-comments").unwrap();

        assert_eq!(counts.get_budget(&todo_id, Path::new(".")), 0);
    });
}

#[test]
fn test_bump_all_with_unchanged_budgets() {
    with_temp_dir(|temp_dir| {
        // Create config with rules
        let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
no-fixme-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

        // Create counts file with budgets matching current violations
        let counts = r#"
[no-todo-comments]
"." = 1

[no-fixme-comments]
"." = 1
"#;
        fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

        // Create builtin rules
        let builtin_regex_dir = temp_dir
            .path()
            .join("builtin-ratchets")
            .join("common")
            .join("regex");
        fs::create_dir_all(&builtin_regex_dir).unwrap();

        let todo_rule = r#"
[rule]
id = "no-todo-comments"
description = "Disallow TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
        fs::write(builtin_regex_dir.join("no-todo-comments.toml"), todo_rule).unwrap();

        let fixme_rule = r#"
[rule]
id = "no-fixme-comments"
description = "Disallow FIXME comments"
severity = "warning"

[match]
pattern = "FIXME"
"#;
        fs::write(builtin_regex_dir.join("no-fixme-comments.toml"), fixme_rule).unwrap();

        // Create file with violations matching budgets
        fs::write(
            temp_dir.path().join("test.rs"),
            "// TODO: test\n// FIXME: fix\n",
        )
        .unwrap();

        // Run bump --all
        let exit_code = cli::bump::run_bump(None, ".", None, true);

        assert_eq!(exit_code, cli::common::EXIT_SUCCESS);

        // Verify budgets remained the same
        let counts_content =
            fs::read_to_string(temp_dir.path().join("ratchet-counts.toml")).unwrap();
        let counts = ratchets::config::counts::CountsManager::parse(&counts_content).unwrap();
        let todo_id = ratchets::types::RuleId::new("no-todo-comments").unwrap();
        let fixme_id = ratchets::types::RuleId::new("no-fixme-comments").unwrap();

        // Budgets should remain at 1
        assert_eq!(counts.get_budget(&todo_id, Path::new(".")), 1);
        assert_eq!(counts.get_budget(&fixme_id, Path::new(".")), 1);
    });
}

#[test]
fn test_bump_all_with_no_rules_enabled() {
    with_temp_dir(|temp_dir| {
        // Create config with no languages (which results in no rules)
        let config = r#"
[ratchets]
version = "1"
languages = []
include = ["**/*.rs"]

[rules]
"#;
        fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

        // Create empty counts file
        fs::write(temp_dir.path().join("ratchet-counts.toml"), "").unwrap();

        // Run bump --all
        let exit_code = cli::bump::run_bump(None, ".", None, true);

        // Should fail with error since no rules are enabled
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}

#[test]
fn test_bump_all_missing_config() {
    with_temp_dir(|_temp_dir| {
        // Don't create config

        let exit_code = cli::bump::run_bump(None, ".", None, true);

        // Should fail
        assert_eq!(exit_code, cli::common::EXIT_ERROR);
    });
}
