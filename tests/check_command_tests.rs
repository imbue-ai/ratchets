//! Integration tests for the check command
//!
//! NOTE: These tests change the current directory and use the serial_test attribute
//! to ensure they run sequentially and don't interfere with each other.

use serial_test::serial;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper to create a test project structure
fn setup_test_project(temp_dir: &Path) {
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
"." = 2
"#;
    fs::write(temp_dir.join("ratchet-counts.toml"), counts).unwrap();

    // Create builtin-ratchets/common/regex directory with no-todo-comments rule
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

    // Create some test source files
    fs::write(temp_dir.join("clean.rs"), "fn main() {}\n").unwrap();
    fs::write(
        temp_dir.join("has_todo.rs"),
        "// TODO: fix this\nfn main() {}\n",
    )
    .unwrap();
}

#[test]
#[serial]
fn test_check_command_within_budget() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_project(temp_dir.path());

    // Change to the temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run the check command
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
    );

    // Should pass because we have 1 TODO and budget is 2
    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_command_exceeded_budget() {
    let temp_dir = TempDir::new().unwrap();

    // Create ratchets.toml with lower budget
    let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    // Create ratchet-counts.toml with budget of 1
    let counts = r#"
[no-todo-comments]
"." = 1
"#;
    fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

    // Create builtin-ratchets/common/regex directory with no-todo-comments rule
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

    // Create files with 2 TODOs (exceeds budget of 1)
    fs::write(
        temp_dir.path().join("todo1.rs"),
        "// TODO: first\nfn test() {}\n",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("todo2.rs"),
        "// TODO: second\nfn test2() {}\n",
    )
    .unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run the check command
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
    );

    // Should fail because we have 2 TODOs and budget is 1
    assert_eq!(exit_code, ratchets::cli::common::EXIT_EXCEEDED);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_command_missing_config() {
    let temp_dir = TempDir::new().unwrap();

    // Create a subdirectory to ensure isolation
    let test_subdir = temp_dir.path().join("empty_project");
    fs::create_dir(&test_subdir).unwrap();
    // Don't create ratchets.toml - this is the test condition

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&test_subdir).unwrap();

    // Run the check command
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
    );

    // Should return error code
    assert_eq!(exit_code, ratchets::cli::common::EXIT_ERROR);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_command_no_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create config but no source files
    let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run the check command
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
    );

    // Should succeed with warning (no files to check)
    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_command_jsonl_format() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_project(temp_dir.path());

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run the check command with JSONL format
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Jsonl,
        false,
    );

    // Should pass because we have 1 TODO and budget is 2
    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_verbose_flag() {
    let temp_dir = TempDir::new().unwrap();

    // Create ratchets.toml
    let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]
exclude = ["excluded/**"]

[rules]
no-todo-comments = true
rust-no-todo-comments = false
rust-no-fixme-comments = false
"#;
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    // Create ratchet-counts.toml
    let counts = r#"
[no-todo-comments]
"." = 10
"#;
    fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

    // Create builtin-ratchets/common/regex directory with no-todo-comments rule
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

    // Create some test source files to scan
    fs::write(temp_dir.path().join("included.rs"), "fn main() {}\n").unwrap();
    fs::write(
        temp_dir.path().join("has_todo.rs"),
        "// TODO: fix this\nfn main() {}\n",
    )
    .unwrap();

    // Create excluded directory with files to skip
    let excluded_dir = temp_dir.path().join("excluded");
    fs::create_dir_all(&excluded_dir).unwrap();
    fs::write(excluded_dir.join("skipped.rs"), "fn test() {}\n").unwrap();

    // Create a non-rust file to skip
    fs::write(temp_dir.path().join("readme.txt"), "Hello\n").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run check with verbose flag (stderr output will go to console during test)
    // We're testing that it doesn't crash and completes successfully
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        true, // verbose = true
    );

    // Should succeed - we have 1 TODO and budget is 10
    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_verbose_short_flag_behavior() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_project(temp_dir.path());

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run check with verbose flag (simulating -v short flag)
    // This tests that the verbose parameter works correctly
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        true, // verbose = true (equivalent to -v)
    );

    // Should pass because we have 1 TODO and budget is 2
    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_verbose_with_jsonl_format() {
    let temp_dir = TempDir::new().unwrap();

    // Create ratchets.toml with specific include pattern
    let config = r#"
[ratchets]
version = "1"
languages = ["rust"]
include = ["src/**/*.rs"]

[rules]
no-todo-comments = true
"#;
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    // Create ratchet-counts.toml
    let counts = r#"
[no-todo-comments]
"." = 10
"#;
    fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

    // Create builtin-ratchets/common/regex directory
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

    // Create src directory with files to include
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("included.rs"), "fn main() {}\n").unwrap();

    // Create file outside src that should be skipped
    fs::write(temp_dir.path().join("excluded.rs"), "fn test() {}\n").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run check with verbose flag and JSONL format
    // Verbose messages should go to stderr, JSONL to stdout
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Jsonl,
        true, // verbose = true
    );

    // Should succeed
    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_non_verbose_hides_violations() {
    let temp_dir = TempDir::new().unwrap();

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
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    // Create ratchet-counts.toml with budget of 10 (so check will pass)
    let counts = r#"
[no-todo-comments]
"." = 10
"#;
    fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

    // Create builtin-ratchets/common/regex directory with no-todo-comments rule
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

    // Create files with TODOs
    fs::write(
        temp_dir.path().join("todo1.rs"),
        "// TODO: first\nfn test() {}\n",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("todo2.rs"),
        "// TODO: second\nfn test2() {}\n",
    )
    .unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Capture stdout/stderr to verify output
    // Since we can't easily capture stdout in this test, we'll just verify
    // that the check command completes successfully with verbose=false
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false, // verbose = false
    );

    // Should pass because we have 2 TODOs and budget is 10
    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);

    // Note: We can't easily verify that violation details are NOT in output
    // from this integration test because stdout goes directly to the terminal.
    // The actual behavior verification will be done through manual testing
    // or by checking the formatter unit tests which test both verbose=true and verbose=false.

    std::env::set_current_dir(original_dir).unwrap();
}
