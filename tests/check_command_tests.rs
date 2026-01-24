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
    // Create ratchet.toml
    let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
    fs::write(temp_dir.join("ratchet.toml"), config).unwrap();

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
    let exit_code =
        ratchet::cli::check::run_check(&[".".to_string()], ratchet::cli::OutputFormat::Human);

    // Should pass because we have 1 TODO and budget is 2
    assert_eq!(exit_code, ratchet::cli::common::EXIT_SUCCESS);

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_command_exceeded_budget() {
    let temp_dir = TempDir::new().unwrap();

    // Create ratchet.toml with lower budget
    let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
    fs::write(temp_dir.path().join("ratchet.toml"), config).unwrap();

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
    let exit_code =
        ratchet::cli::check::run_check(&[".".to_string()], ratchet::cli::OutputFormat::Human);

    // Should fail because we have 2 TODOs and budget is 1
    assert_eq!(exit_code, ratchet::cli::common::EXIT_EXCEEDED);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_command_missing_config() {
    let temp_dir = TempDir::new().unwrap();

    // Create a subdirectory to ensure isolation
    let test_subdir = temp_dir.path().join("empty_project");
    fs::create_dir(&test_subdir).unwrap();
    // Don't create ratchet.toml - this is the test condition

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&test_subdir).unwrap();

    // Run the check command
    let exit_code =
        ratchet::cli::check::run_check(&[".".to_string()], ratchet::cli::OutputFormat::Human);

    // Should return error code
    assert_eq!(exit_code, ratchet::cli::common::EXIT_ERROR);

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_command_no_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create config but no source files
    let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
    fs::write(temp_dir.path().join("ratchet.toml"), config).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run the check command
    let exit_code =
        ratchet::cli::check::run_check(&[".".to_string()], ratchet::cli::OutputFormat::Human);

    // Should succeed with warning (no files to check)
    assert_eq!(exit_code, ratchet::cli::common::EXIT_SUCCESS);

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
    let exit_code =
        ratchet::cli::check::run_check(&[".".to_string()], ratchet::cli::OutputFormat::Jsonl);

    // Should pass because we have 1 TODO and budget is 2
    assert_eq!(exit_code, ratchet::cli::common::EXIT_SUCCESS);

    std::env::set_current_dir(original_dir).unwrap();
}
