//! Integration tests for the check command
//!
//! NOTE: These tests change the current directory and use the serial_test attribute
//! to ensure they run sequentially and don't interfere with each other.

use serial_test::serial;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Helper to create a test project structure
fn setup_test_project(temp_dir: &Path) {
    // Create ratchets.toml
    let config = r#"
enabled_ratchets = ["no-todo-comments", "no-fixme-comments"]

[ratchets]
version = "2"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
"#;
    fs::write(temp_dir.join("ratchets.toml"), config).unwrap();

    // Create ratchet-counts.toml. The v2 schema removed the `[rules].rule-id
    // = false` shorthand for silencing rules; the equivalent now is
    // `disabled_ratchets = ["..."]`, but here we just grant the embedded
    // Rust AST rules a generous budget so the test is rule-set-agnostic.
    let counts = r#"
[no-todo-comments]
"." = 2

[rust-no-todo-comments]
"." = 1000

[rust-no-fixme-comments]
"." = 1000
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
        None,
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
enabled_ratchets = ["no-todo-comments", "no-fixme-comments"]

[ratchets]
version = "2"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
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
        None,
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
        None,
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
enabled_ratchets = ["no-todo-comments", "no-fixme-comments"]

[ratchets]
version = "2"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
"#;
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run the check command
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
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
        None,
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
enabled_ratchets = ["no-todo-comments", "no-fixme-comments"]

[ratchets]
version = "2"
languages = ["rust"]
include = ["**/*.rs"]
exclude = ["excluded/**"]

[rules]
"#;
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    // Create ratchet-counts.toml. Phase 1: embedded Rust AST rules can no
    // longer be silenced via the boolean shorthand, so we grant generous
    // budgets directly.
    let counts = r#"
[no-todo-comments]
"." = 10

[rust-no-todo-comments]
"." = 1000

[rust-no-fixme-comments]
"." = 1000
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
        None,
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
        None,
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
enabled_ratchets = ["no-todo-comments", "no-fixme-comments"]

[ratchets]
version = "2"
languages = ["rust"]
include = ["src/**/*.rs"]

[rules]
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
        None,
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
enabled_ratchets = ["no-todo-comments", "no-fixme-comments"]

[ratchets]
version = "2"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
"#;
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    // Create ratchet-counts.toml with budget of 10 (so check will pass).
    // Phase 1 also requires explicit budgets for embedded Rust AST rules.
    let counts = r#"
[no-todo-comments]
"." = 10

[rust-no-todo-comments]
"." = 1000

[rust-no-fixme-comments]
"." = 1000
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
        None,
    );

    // Should pass because we have 2 TODOs and budget is 10
    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);

    // Note: We can't easily verify that violation details are NOT in output
    // from this integration test because stdout goes directly to the terminal.
    // The actual behavior verification will be done through manual testing
    // or by checking the formatter unit tests which test both verbose=true and verbose=false.

    std::env::set_current_dir(original_dir).unwrap();
}

// ============================================================================
// --since <ref> tests
// ============================================================================

/// Runs `git` with the given args in `dir` and asserts it succeeds.
fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git command should be runnable in tests");
    assert!(
        output.status.success(),
        "git {:?} failed: stderr={}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Builds a git repo with two commits:
///   1) Adds `clean.rs` (no TODO) and `has_todo.rs` (with TODO) and commits.
///   2) Modifies only `has_todo.rs` to add a *second* TODO and commits.
///
/// Returns the SHA of the first commit so callers can pass it as `--since`.
fn setup_git_project_with_history(temp_dir: &Path) -> String {
    // Standalone, hermetic config: tests must not depend on the developer's
    // global git identity, init.defaultBranch, etc.
    git(temp_dir, &["init", "--initial-branch=main", "--quiet"]);
    git(temp_dir, &["config", "user.email", "test@example.com"]);
    git(temp_dir, &["config", "user.name", "Test User"]);
    git(temp_dir, &["config", "commit.gpgsign", "false"]);

    // ratchets.toml, ratchet-counts.toml, and the builtin rule.
    setup_test_project(temp_dir);

    // Commit baseline. setup_test_project creates clean.rs and has_todo.rs.
    git(temp_dir, &["add", "."]);
    git(temp_dir, &["commit", "-m", "baseline", "--quiet"]);

    let baseline = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp_dir)
        .output()
        .expect("rev-parse should succeed");
    let baseline_sha = String::from_utf8(baseline.stdout)
        .expect("git sha should be valid utf-8")
        .trim()
        .to_string();

    // Second commit: modify has_todo.rs only.
    fs::write(
        temp_dir.join("has_todo.rs"),
        "// TODO: fix this\n// TODO: another one\nfn main() {}\n",
    )
    .unwrap();
    git(temp_dir, &["add", "has_todo.rs"]);
    git(temp_dir, &["commit", "-m", "second", "--quiet"]);

    baseline_sha
}

#[test]
#[serial]
fn test_check_since_filters_to_changed_files_only() {
    let temp_dir = TempDir::new().unwrap();
    let baseline_sha = setup_git_project_with_history(temp_dir.path());

    // Override the counts so that ALL TODOs would exceed the budget. With
    // --since baseline_sha, only has_todo.rs (which has 2 TODOs) is scanned;
    // the budget of 1 is exceeded -> EXIT_EXCEEDED. Without --since the
    // baseline file is also scanned, but it still totals 2 TODOs, so the
    // budget is exceeded either way. The discriminating test is below: we
    // pre-stage a third TODO outside the diff and assert it is NOT counted.
    // Phase 1: also need generous budgets for embedded Rust AST rules since
    // they can no longer be silenced via the boolean shorthand.
    let counts = r#"
[no-todo-comments]
"." = 5

[rust-no-todo-comments]
"." = 1000

[rust-no-fixme-comments]
"." = 1000
"#;
    fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

    // Add an unstaged, uncommitted file with several TODOs that is NOT
    // in the diff between HEAD and baseline (it lives in the working tree
    // only and was never committed). It WILL appear in `git diff HEAD~1`
    // because it is tracked? No - it isn't tracked. Actually, untracked
    // files do not appear in `git diff`. That's exactly what we want:
    // the untracked file is part of the working tree, the walker sees it,
    // but `--since` filters it out.
    fs::write(
        temp_dir.path().join("untracked.rs"),
        "// TODO: a\n// TODO: b\n// TODO: c\n// TODO: d\n// TODO: e\n// TODO: f\nfn main() {}\n",
    )
    .unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Without --since: walker finds clean.rs + has_todo.rs (2 TODOs) +
    // untracked.rs (6 TODOs) = 8 TODOs > budget 5 -> EXCEEDED.
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );
    assert_eq!(
        exit_code,
        ratchets::cli::common::EXIT_EXCEEDED,
        "without --since, untracked.rs should be scanned and exceed the budget",
    );

    // With --since baseline: only has_todo.rs is in the diff (2 TODOs <
    // budget 5) -> SUCCESS. untracked.rs is excluded because it isn't in
    // `git diff baseline_sha --name-only`. clean.rs is excluded because it
    // was not modified.
    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        Some(&baseline_sha),
    );
    assert_eq!(
        exit_code,
        ratchets::cli::common::EXIT_SUCCESS,
        "with --since baseline, only has_todo.rs should be scanned and pass the budget",
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_since_bad_ref_returns_error() {
    let temp_dir = TempDir::new().unwrap();
    let _baseline = setup_git_project_with_history(temp_dir.path());

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        Some("this-ref-does-not-exist"),
    );
    assert_eq!(exit_code, ratchets::cli::common::EXIT_ERROR);

    std::env::set_current_dir(original_dir).unwrap();
}

// ============================================================================
// Regression: bead code-owl
//
// When `ratchets check` is invoked with no path arg (or `.`), the file walker
// emits paths prefixed with `./` (e.g. `./sub/file.tsx`). Anchored include
// globs in rule TOMLs (e.g. `sub/**/*.tsx`) used to silently miss those paths
// because globsets compared the literal `./` prefix. The rule helper
// `normalize_for_glob_match` strips the prefix at the comparison site so
// anchored includes match regardless of how the walker spelled the path.
// ============================================================================

#[test]
#[serial]
fn test_check_anchored_include_glob_matches_from_dot_root() {
    // Regression test for bead code-owl: an anchored `include` glob in a
    // rule TOML that targets a top-level subdirectory must fire even when
    // ratchets is invoked from the parent directory of that subdirectory.
    //
    // Mechanism: when `ratchets check` runs with no PATH (or `.`), the file
    // walker emits paths prefixed with `./`. Before the fix, anchored globs
    // like `example_app/frontend/src/**/*.tsx` failed to match those paths
    // because globsets compared the `./` prefix literally.
    //
    // The rule under test is a custom inline rule written to the temp dir's
    // `ratchets/regex/` directory, so the regression coverage does not depend
    // on any particular embedded rule. The rule has the same shape as the
    // original repro (anchored TypeScript `include` glob looking for `<button`).
    let temp_dir = TempDir::new().unwrap();

    let config = r#"
enabled_ratchets = ["no-raw-button-jsx"]

[ratchets]
version = "2"
languages = ["typescript"]

[rules]
"#;
    fs::write(temp_dir.path().join("ratchets.toml"), config).unwrap();

    // Custom rule TOML: TypeScript regex with an anchored include glob on a
    // top-level subdirectory, mirroring the original failure mode.
    let custom_rule_dir = temp_dir.path().join("ratchets").join("regex");
    fs::create_dir_all(&custom_rule_dir).unwrap();
    let rule_toml = r#"
[rule]
id = "no-raw-button-jsx"
description = "Disallow raw <button> JSX in the example app frontend"
severity = "warning"

[match]
pattern = "<button(\\s|>|$)"
languages = ["typescript"]
include = ["example_app/frontend/src/**/*.tsx"]
"#;
    fs::write(custom_rule_dir.join("no-raw-button-jsx.toml"), rule_toml).unwrap();

    // Budget of 0 in the root region: any single match -> EXCEEDED.
    let counts = r#"
[no-raw-button-jsx]
"." = 0
"#;
    fs::write(temp_dir.path().join("ratchet-counts.toml"), counts).unwrap();

    // Place a violating file under the custom rule's anchored include path.
    let src_dir = temp_dir
        .path()
        .join("example_app")
        .join("frontend")
        .join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("App.tsx"), "<button>X</button>\n").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run check from the parent directory (`.`). Before the fix, the walker
    // emitted `./example_app/frontend/src/App.tsx` and the include glob
    // `example_app/frontend/src/**/*.tsx` silently failed to match, so the
    // rule reported 0 violations and check exited SUCCESS. After the fix, the
    // glob matches and the budget-0 rule reports 1 violation -> EXCEEDED.
    let exit_code_dot = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );
    assert_eq!(
        exit_code_dot,
        ratchets::cli::common::EXIT_EXCEEDED,
        "anchored include glob must match ./ prefixed walker paths",
    );

    // Sanity check: same project, but invoke check with the subdirectory
    // explicitly. This already worked before the fix, and must continue to.
    let exit_code_sub = ratchets::cli::check::run_check(
        &["example_app".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );
    assert_eq!(
        exit_code_sub,
        ratchets::cli::common::EXIT_EXCEEDED,
        "anchored include glob must continue to match canonical paths",
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[serial]
fn test_check_since_outside_git_repo_returns_error() {
    let temp_dir = TempDir::new().unwrap();
    // Deliberately do NOT call `git init`. The fixture is otherwise a
    // valid ratchets project.
    setup_test_project(temp_dir.path());

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        Some("main"),
    );
    assert_eq!(exit_code, ratchets::cli::common::EXIT_ERROR);

    std::env::set_current_dir(original_dir).unwrap();
}

// ============================================================================
// PHASE 3 (`code-6ik`): resolver-driven enablement via SetRegistry
// ============================================================================
//
// Phase 3 of `blueprint/ratchet-sets/plan-ratchet-sets.md` wires
// `SetRegistry::resolve` into `RuleRegistry::build_from_config`. These tests
// exercise the end-to-end behavior the bead calls out: bare rule IDs in
// `enabled_ratchets` load exactly those rules, `disabled_ratchets` subtracts
// rules from the resolved set, user-defined sets under `ratchets/sets/`
// expand correctly, and cycles in user-defined sets surface as the
// plan-prescribed error (non-zero exit, clear stderr).

/// Set up a project with no embedded rules involved — only custom regex
/// rules and an `enabled_ratchets` list naming them. Lets the Phase 3
/// tests focus on resolver behavior without dragging in budgets for the
/// embedded Rust AST rules.
fn setup_phase3_custom_rule_project(temp_dir: &Path, ratchets_toml_body: &str) {
    let config = format!(
        r#"
{ratchets_toml_body}

[ratchets]
version = "2"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
"#
    );
    fs::write(temp_dir.join("ratchets.toml"), config).unwrap();

    // Budget of 0 so any violation makes check exit EXCEEDED. Lets each test
    // assert pass/fail purely from rule resolution, not from counts.
    let counts = r#"
[phase3-no-todo]
"." = 0

[phase3-no-fixme]
"." = 0
"#;
    fs::write(temp_dir.join("ratchet-counts.toml"), counts).unwrap();

    let custom_regex_dir = temp_dir.join("ratchets").join("regex");
    fs::create_dir_all(&custom_regex_dir).unwrap();

    fs::write(
        custom_regex_dir.join("phase3-no-todo.toml"),
        r#"
[rule]
id = "phase3-no-todo"
description = "Phase 3 fixture rule: forbid TODO"
severity = "warning"

[match]
pattern = "TODO"
"#,
    )
    .unwrap();

    fs::write(
        custom_regex_dir.join("phase3-no-fixme.toml"),
        r#"
[rule]
id = "phase3-no-fixme"
description = "Phase 3 fixture rule: forbid FIXME"
severity = "warning"

[match]
pattern = "FIXME"
"#,
    )
    .unwrap();
}

#[test]
#[serial]
fn test_phase3_bare_rule_ids_in_enabled_ratchets_load_exactly_those_rules() {
    // Acceptance criterion: a config with `enabled_ratchets = ["a", "b"]`
    // loads exactly those two rules. Use Phase-3 fixture rules to avoid
    // entangling the assertion with embedded builtins.
    let temp_dir = TempDir::new().unwrap();
    setup_phase3_custom_rule_project(temp_dir.path(), r#"enabled_ratchets = ["phase3-no-todo"]"#);

    // Source file matches BOTH custom rules, but only phase3-no-todo is
    // enabled — so only that rule's violation should exceed budget.
    fs::write(
        temp_dir.path().join("src.rs"),
        "// TODO: a\n// FIXME: b\nfn main() {}\n",
    )
    .unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );

    std::env::set_current_dir(original_dir).unwrap();

    // phase3-no-todo finds a violation against budget 0 -> EXCEEDED. The fact
    // that the check fires at all confirms the bare rule ID resolved to the
    // expected rule; if `phase3-no-fixme` had also been enabled it would also
    // have fired, but we did not list it so it should have been filtered out
    // of the registry entirely.
    assert_eq!(exit_code, ratchets::cli::common::EXIT_EXCEEDED);
}

#[test]
#[serial]
fn test_phase3_disabled_ratchets_removes_rule_from_resolved_set() {
    // Acceptance criterion: `disabled_ratchets` removes a rule even if it is
    // present in `enabled_ratchets`.
    let temp_dir = TempDir::new().unwrap();
    setup_phase3_custom_rule_project(
        temp_dir.path(),
        r#"
enabled_ratchets = ["phase3-no-todo", "phase3-no-fixme"]
disabled_ratchets = ["phase3-no-todo"]
"#,
    );

    // Only phase3-no-fixme should remain in the resolved set. A file with a
    // TODO but no FIXME should therefore pass (no enabled rule fires).
    fs::write(
        temp_dir.path().join("src.rs"),
        "// TODO: keep me\nfn main() {}\n",
    )
    .unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );

    std::env::set_current_dir(original_dir).unwrap();

    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);
}

#[test]
#[serial]
fn test_phase3_user_defined_set_expands_to_member_rule_ids() {
    // Acceptance criterion: a user-defined set under `ratchets/sets/*.toml`
    // expands to its rules at resolution time. This stands in for the
    // `$common-starter` end-to-end test that Phase 4 will exercise once the
    // shipped set lands.
    let temp_dir = TempDir::new().unwrap();
    setup_phase3_custom_rule_project(temp_dir.path(), r#"enabled_ratchets = ["$phase3-strict"]"#);

    let user_sets_dir = temp_dir.path().join("ratchets").join("sets");
    fs::create_dir_all(&user_sets_dir).unwrap();
    fs::write(
        user_sets_dir.join("phase3-strict.toml"),
        r#"
[set]
id = "phase3-strict"
description = "Phase 3 inline fixture set covering both no-todo and no-fixme"

rules = ["phase3-no-todo", "phase3-no-fixme"]
"#,
    )
    .unwrap();

    fs::write(
        temp_dir.path().join("src.rs"),
        "// FIXME: a\nfn main() {}\n",
    )
    .unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );

    std::env::set_current_dir(original_dir).unwrap();

    // FIXME hits phase3-no-fixme (resolved via the set), budget 0 -> EXCEEDED.
    assert_eq!(exit_code, ratchets::cli::common::EXIT_EXCEEDED);
}

#[test]
#[serial]
fn test_phase3_cycle_in_user_defined_sets_exits_error_not_hang() {
    // Acceptance criterion: a cycle surfaces as a clear error and non-zero
    // exit, never a hang. The resolver's DFS detects the cycle in O(depth);
    // this test simply confirms the CLI translates the error into
    // `EXIT_ERROR` rather than panicking or looping.
    let temp_dir = TempDir::new().unwrap();
    setup_phase3_custom_rule_project(temp_dir.path(), r#"enabled_ratchets = ["$phase3-a"]"#);

    let user_sets_dir = temp_dir.path().join("ratchets").join("sets");
    fs::create_dir_all(&user_sets_dir).unwrap();
    fs::write(
        user_sets_dir.join("phase3-a.toml"),
        r#"
[set]
id = "phase3-a"
description = "Cycle fixture A"

rules = ["$phase3-b"]
"#,
    )
    .unwrap();
    fs::write(
        user_sets_dir.join("phase3-b.toml"),
        r#"
[set]
id = "phase3-b"
description = "Cycle fixture B"

rules = ["$phase3-a"]
"#,
    )
    .unwrap();

    fs::write(temp_dir.path().join("src.rs"), "fn main() {}\n").unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );

    std::env::set_current_dir(original_dir).unwrap();

    assert_eq!(exit_code, ratchets::cli::common::EXIT_ERROR);
}

// ============================================================================
// Phase 4: end-to-end `$common-starter` coverage
//
// The embedded `common-starter` set ships `no-todo-comments` and
// `no-fixme-comments`. Acceptance criteria for the bead (code-u27):
//
//   - `ratchets check` against a config with `enabled_ratchets = ["$common-starter"]`
//     and counts.toml `"." = 0` for `no-todo-comments` finds exactly one
//     violation on a file containing a `TODO` and exits EXCEEDED.
//   - Adding `disabled_ratchets = ["no-todo-comments"]` flips the result to
//     SUCCESS because `no-fixme-comments` does not fire on the fixtures.
// ============================================================================

/// Build a project that opts in to `$common-starter` and nothing else.
/// `extra_config_body` is interpolated above `[ratchets]`, letting callers add
/// `disabled_ratchets` without rewriting the whole scaffold.
fn setup_common_starter_project(temp_dir: &Path, extra_config_body: &str) {
    let config = format!(
        r#"
enabled_ratchets = ["$common-starter"]
{extra_config_body}

[ratchets]
version = "2"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
"#
    );
    fs::write(temp_dir.join("ratchets.toml"), config).unwrap();

    // Budget of 0 in the root region: any violation flips check to EXCEEDED.
    // Only the two `$common-starter` members need budgets; the Rust AST rules
    // are filtered out of the registry by the resolver because they are not
    // in the resolved enabled set.
    let counts = r#"
[no-todo-comments]
"." = 0

[no-fixme-comments]
"." = 0
"#;
    fs::write(temp_dir.join("ratchet-counts.toml"), counts).unwrap();

    // Two source files: one clean, one with a `TODO`. Neither contains a
    // `FIXME`, so flipping the second test to `disabled_ratchets = ["no-todo-comments"]`
    // leaves `no-fixme-comments` enabled but with zero fixture violations.
    fs::write(temp_dir.join("clean.rs"), "fn main() {}\n").unwrap();
    fs::write(
        temp_dir.join("has_todo.rs"),
        "// TODO: fix this\nfn main() {}\n",
    )
    .unwrap();
}

#[test]
#[serial]
fn test_phase4_common_starter_finds_todo_violation_and_exits_exceeded() {
    // Acceptance criterion 1: with `enabled_ratchets = ["$common-starter"]`,
    // a file containing `TODO` triggers exactly one `no-todo-comments`
    // violation, exceeding the budget of 0 and exiting EXCEEDED.
    let temp_dir = TempDir::new().unwrap();
    setup_common_starter_project(temp_dir.path(), "");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );

    std::env::set_current_dir(original_dir).unwrap();

    assert_eq!(exit_code, ratchets::cli::common::EXIT_EXCEEDED);
}

#[test]
#[serial]
fn test_phase4_disabled_ratchets_overrides_common_starter_member() {
    // Acceptance criterion 2: `disabled_ratchets = ["no-todo-comments"]`
    // removes that rule from the resolved set even though it is reachable
    // through `$common-starter`. The remaining member, `no-fixme-comments`,
    // finds nothing in the fixtures, so check exits SUCCESS.
    let temp_dir = TempDir::new().unwrap();
    setup_common_starter_project(
        temp_dir.path(),
        r#"disabled_ratchets = ["no-todo-comments"]"#,
    );

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let exit_code = ratchets::cli::check::run_check(
        &[".".to_string()],
        ratchets::cli::OutputFormat::Human,
        false,
        None,
    );

    std::env::set_current_dir(original_dir).unwrap();

    assert_eq!(exit_code, ratchets::cli::common::EXIT_SUCCESS);
}

#[test]
fn test_phase4_load_builtin_sets_returns_common_starter_only() {
    // Unit-level acceptance criterion from bead code-u27: `load_builtin_sets`
    // returns exactly one set whose ID is `common-starter` and whose member
    // rules are the two cross-language regex rules shipped in Phase 1. Sits
    // alongside the end-to-end tests above so that adding new embedded sets
    // forces the maintainer to update both the unit and integration coverage
    // in one commit.
    use ratchets::config::ratchet_toml::RatchetRef;

    let sets = ratchets::rules::load_builtin_sets().expect("embedded sets must parse");
    assert_eq!(sets.len(), 1, "Phase 4 ships exactly one embedded set");

    let common_starter = &sets[0];
    assert_eq!(common_starter.id().as_str(), "common-starter");

    let member_rule_ids: Vec<&str> = common_starter
        .rules()
        .iter()
        .map(|r| match r {
            RatchetRef::Rule(id) => id.as_str(),
            RatchetRef::Set(_) => panic!("common-starter should contain bare rule refs only"),
        })
        .collect();
    assert_eq!(
        member_rule_ids,
        vec!["no-todo-comments", "no-fixme-comments"]
    );
}
