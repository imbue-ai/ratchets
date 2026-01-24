//! End-to-end integration tests for Ratchet
//!
//! These tests verify the full workflow from init through check/tighten with realistic projects:
//! - Full workflow: init -> add violations -> check -> tighten
//! - Multi-file projects with directory structures
//! - Mixed regex and AST rules
//! - Region inheritance across directory structure
//! - Gitignore interaction
//! - All exit codes in realistic scenarios
//!
//! NOTE: These tests change the current directory and use std::sync::Mutex
//! to ensure they don't interfere with each other.

use ratchet::cli;
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
    // Use lock() but handle poison errors gracefully
    let _guard = match TEST_MUTEX.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    // Set up clean temp directory
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Run the test - use panic::catch_unwind to avoid poisoning the mutex
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f(&temp_dir);
    }));

    // Always restore directory
    std::env::set_current_dir(&original_dir).unwrap();

    // Re-panic if the test failed
    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}

/// Helper to create a realistic multi-file Rust project
fn create_multi_file_rust_project(base: &Path) {
    // Create src directory
    let src = base.join("src");
    fs::create_dir_all(&src).unwrap();

    // Main file with some violations
    fs::write(
        src.join("main.rs"),
        r#"
// TODO: refactor this
fn main() {
    let result = Some(42);
    println!("{}", result.unwrap());
}
"#,
    )
    .unwrap();

    // Lib file with violations
    fs::write(
        src.join("lib.rs"),
        r#"
// TODO: add tests
pub fn process(input: Option<i32>) -> i32 {
    input.unwrap() // TODO: handle error properly
}
"#,
    )
    .unwrap();

    // Legacy subdirectory with more violations
    let legacy = src.join("legacy");
    fs::create_dir_all(&legacy).unwrap();

    fs::write(
        legacy.join("old_code.rs"),
        r#"
// TODO: rewrite this module
pub fn legacy_process(data: Vec<i32>) -> Vec<i32> {
    // TODO: optimize
    data.iter().map(|x| x.unwrap_or(&0)).collect()
}
"#,
    )
    .unwrap();

    // Tests directory
    let tests = base.join("tests");
    fs::create_dir_all(&tests).unwrap();

    fs::write(
        tests.join("integration.rs"),
        r#"
#[test]
fn test_something() {
    // TODO: implement test
    assert_eq!(2 + 2, 4);
}
"#,
    )
    .unwrap();
}

/// Helper to create builtin regex rule for TODO comments
fn create_todo_rule(base: &Path) {
    let builtin_regex = base.join("builtin-ratchets").join("common").join("regex");
    fs::create_dir_all(&builtin_regex).unwrap();

    fs::write(
        builtin_regex.join("no-todo-comments.toml"),
        r#"
[rule]
id = "no-todo-comments"
description = "Disallow TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#,
    )
    .unwrap();
}

/// Helper to create builtin regex rule for FIXME comments
fn create_fixme_rule(base: &Path) {
    let builtin_regex = base.join("builtin-ratchets").join("common").join("regex");
    fs::create_dir_all(&builtin_regex).unwrap();

    fs::write(
        builtin_regex.join("no-fixme-comments.toml"),
        r#"
[rule]
id = "no-fixme-comments"
description = "Disallow FIXME comments"
severity = "error"

[match]
pattern = "FIXME"
"#,
    )
    .unwrap();
}

/// Helper to create builtin AST rule for unwrap
fn create_unwrap_rule(base: &Path) {
    let builtin_ast = base.join("builtin-ratchets").join("rust").join("ast");
    fs::create_dir_all(&builtin_ast).unwrap();

    fs::write(
        builtin_ast.join("no-unwrap.toml"),
        r#"
[rule]
id = "no-unwrap"
description = "Disallow .unwrap() calls"
severity = "error"
languages = ["rust"]

[match]
language = "rust"
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap"))
"""
message = "Avoid using .unwrap(), use proper error handling"
"#,
    )
    .unwrap();
}

// ============================================================================
// FULL WORKFLOW TESTS
// ============================================================================

#[test]
fn test_e2e_full_workflow_init_add_check_tighten() {
    with_temp_dir(|temp_dir| {
        // Step 1: Initialize ratchet project
        let init_result = cli::init::run_init(false).expect("init should succeed");
        assert!(init_result.created.contains(&"ratchet.toml".to_string()));
        assert!(
            init_result
                .created
                .contains(&"ratchet-counts.toml".to_string())
        );

        // Step 2: Set up rules and project structure
        create_todo_rule(temp_dir.path());
        create_multi_file_rust_project(temp_dir.path());

        // Update config to enable rust and include patterns
        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
# Disable other embedded rules that we don't want to test here
no-fixme-comments = false
no-unwrap = false
no-panic = false
no-expect = false
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Step 3: Run check (should find violations but with default budget 0, will fail)
        let check_exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(check_exit, cli::common::EXIT_EXCEEDED);

        // Step 4: Set budgets high enough to pass
        // (The project has violations in multiple regions: src, src/legacy, tests)
        let counts = r#"
[no-todo-comments]
"." = 100
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Step 5: Check should now pass
        let check_exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(check_exit, cli::common::EXIT_SUCCESS);

        // Step 6: Remove one TODO
        let main_content = fs::read_to_string("src/main.rs").unwrap();
        let cleaned = main_content.replace("// TODO: refactor this\n", "");
        fs::write("src/main.rs", cleaned).unwrap();

        // Step 7: Tighten should reduce budget
        let tighten_exit = cli::tighten::run_tighten(None, None);
        assert_eq!(tighten_exit, cli::common::EXIT_SUCCESS);

        // Step 8: Check should still pass
        let check_exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(check_exit, cli::common::EXIT_SUCCESS);

        // Step 9: Add a new TODO (should exceed budget now)
        fs::write("src/new_file.rs", "// TODO: new violation\nfn new_fn() {}").unwrap();

        // Step 10: Check should fail with exceeded
        let check_exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(check_exit, cli::common::EXIT_EXCEEDED);
    });
}

#[test]
fn test_e2e_multi_file_project_with_regions() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");

        // Set up project
        create_todo_rule(temp_dir.path());
        create_multi_file_rust_project(temp_dir.path());

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
# Disable other embedded rules that we don't want to test here
no-fixme-comments = false
no-unwrap = false
no-panic = false
no-expect = false
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Set different budgets for different regions
        let counts = r#"
[no-todo-comments]
"." = 100
"src" = 10
"src/legacy" = 5
"tests" = 2
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check should pass (within all region budgets)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Now set src/legacy budget to 0
        let counts = r#"
[no-todo-comments]
"." = 100
"src" = 10
"src/legacy" = 0
"tests" = 2
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check should fail (src/legacy exceeds budget)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_EXCEEDED);
    });
}

// ============================================================================
// MIXED REGEX AND AST RULES TESTS
// ============================================================================

#[test]
fn test_e2e_mixed_regex_and_ast_rules() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");

        // Set up both regex and AST rules
        create_todo_rule(temp_dir.path());
        create_unwrap_rule(temp_dir.path());

        // Create project with both types of violations
        let src = temp_dir.path().join("src");
        fs::create_dir_all(&src).unwrap();

        fs::write(
            src.join("main.rs"),
            r#"
// TODO: handle errors
fn main() {
    let x = Some(42);
    println!("{}", x.unwrap());
}
"#,
        )
        .unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
no-unwrap = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Check should fail (no budgets set)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_EXCEEDED);

        // Set budgets for both rules
        let counts = r#"
[no-todo-comments]
"." = 1

[no-unwrap]
"." = 1
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check should pass
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Add more violations
        fs::write(
            src.join("lib.rs"),
            r#"
// TODO: another todo
fn process() {
    let y = Some(10);
    y.unwrap();
}
"#,
        )
        .unwrap();

        // Check should fail (both rules exceeded)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_EXCEEDED);
    });
}

// ============================================================================
// REGION INHERITANCE TESTS
// ============================================================================

#[test]
fn test_e2e_region_inheritance() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");

        create_todo_rule(temp_dir.path());

        // Create nested directory structure
        let src = temp_dir.path().join("src");
        let core = src.join("core");
        let utils = src.join("utils");
        fs::create_dir_all(&core).unwrap();
        fs::create_dir_all(&utils).unwrap();

        // Add TODOs at different levels
        fs::write(src.join("lib.rs"), "// TODO: 1\n").unwrap();
        fs::write(core.join("engine.rs"), "// TODO: 2\n").unwrap();
        fs::write(utils.join("helpers.rs"), "// TODO: 3\n").unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Set budget only at root - should apply to all nested regions
        let counts = r#"
[no-todo-comments]
"." = 3
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check should pass (3 TODOs, budget 3)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Set specific budget for src/core
        let counts = r#"
[no-todo-comments]
"." = 10
"src/core" = 1
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check should pass (core has 1 TODO, budget 1)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Add another TODO in core
        fs::write(core.join("processor.rs"), "// TODO: 4\n").unwrap();

        // Check should fail (core has 2 TODOs, budget 1)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_EXCEEDED);
    });
}

// ============================================================================
// GITIGNORE INTERACTION TESTS
// ============================================================================

#[test]
fn test_e2e_gitignore_respected() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");

        create_todo_rule(temp_dir.path());

        // Create .gitignore
        fs::write(
            temp_dir.path().join(".gitignore"),
            "target/\n*.log\ngenerated/\n",
        )
        .unwrap();

        // Create files in ignored directories
        let target = temp_dir.path().join("target");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("debug.rs"), "// TODO: should be ignored\n").unwrap();

        let generated = temp_dir.path().join("generated");
        fs::create_dir_all(&generated).unwrap();
        fs::write(generated.join("code.rs"), "// TODO: also ignored\n").unwrap();

        // Create a log file
        fs::write(temp_dir.path().join("app.log"), "// TODO: ignored\n").unwrap();

        // Create files NOT in gitignore
        let src = temp_dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "// TODO: this counts\n").unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        let counts = r#"
[no-todo-comments]
"." = 1
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check should pass (only 1 TODO counted, gitignored files excluded)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Add another TODO in non-ignored location
        fs::write(src.join("lib.rs"), "// TODO: another\n").unwrap();

        // Check should fail
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_EXCEEDED);
    });
}

// ============================================================================
// EXIT CODE TESTS
// ============================================================================

#[test]
fn test_e2e_all_exit_codes() {
    with_temp_dir(|temp_dir| {
        // EXIT_SUCCESS (0): Within budget
        cli::init::run_init(false).expect("init should succeed");
        create_todo_rule(temp_dir.path());

        fs::write(temp_dir.path().join("test.rs"), "// TODO: test\n").unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        let counts = r#"
[no-todo-comments]
"." = 1
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // EXIT_EXCEEDED (1): Over budget
        let counts = r#"
[no-todo-comments]
"." = 0
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_EXCEEDED);

        // EXIT_ERROR (2): Missing config
        fs::remove_file("ratchet.toml").unwrap();

        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_ERROR);

        // EXIT_PARSE_ERROR (3): Invalid TOML syntax
        let invalid_config = r#"
[ratchet]
version = "1"
languages = ["rust"
# Missing closing bracket - invalid TOML
"#;
        fs::write("ratchet.toml", invalid_config).unwrap();

        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_PARSE_ERROR);
    });
}

// ============================================================================
// COMPLEX MULTI-RULE MULTI-REGION TESTS
// ============================================================================

#[test]
fn test_e2e_complex_multi_rule_multi_region() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");

        // Set up multiple rules
        create_todo_rule(temp_dir.path());
        create_fixme_rule(temp_dir.path());
        create_unwrap_rule(temp_dir.path());

        // Create complex project structure
        let src = temp_dir.path().join("src");
        let core = src.join("core");
        let legacy = src.join("legacy");
        fs::create_dir_all(&core).unwrap();
        fs::create_dir_all(&legacy).unwrap();

        // Core module - clean
        fs::write(
            core.join("engine.rs"),
            r#"
pub fn process(x: i32) -> Result<i32, String> {
    Ok(x * 2)
}
"#,
        )
        .unwrap();

        // Legacy module - many violations
        fs::write(
            legacy.join("old.rs"),
            r#"
// TODO: rewrite this
// FIXME: broken logic
pub fn legacy_process(data: Option<i32>) -> i32 {
    data.unwrap()
}
"#,
        )
        .unwrap();

        // Main - some violations
        fs::write(
            src.join("main.rs"),
            r#"
// TODO: add error handling
fn main() {
    let x = Some(42);
    println!("{}", x.unwrap());
}
"#,
        )
        .unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
no-fixme-comments = true
no-unwrap = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Set different budgets per rule per region
        let counts = r#"
[no-todo-comments]
"." = 2
"src/legacy" = 1

[no-fixme-comments]
"." = 1
"src/legacy" = 1

[no-unwrap]
"." = 2
"src/core" = 0
"src/legacy" = 1
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check should pass (all within budgets)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // List rules
        let list_exit = cli::list::run_list(cli::OutputFormat::Human);
        assert_eq!(list_exit, cli::common::EXIT_SUCCESS);

        // Tighten - should work since no region is exceeded
        let tighten_exit = cli::tighten::run_tighten(None, None);
        assert_eq!(tighten_exit, cli::common::EXIT_SUCCESS);

        // Check should still pass
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);
    });
}

// ============================================================================
// REAL-WORLD SCENARIO TESTS
// ============================================================================

#[test]
fn test_e2e_gradual_cleanup_workflow() {
    with_temp_dir(|temp_dir| {
        // Scenario: Team is gradually cleaning up TODOs in a legacy codebase

        // Initialize project
        cli::init::run_init(false).expect("init should succeed");
        create_todo_rule(temp_dir.path());

        // Create legacy project with many TODOs
        let src = temp_dir.path().join("src");
        fs::create_dir_all(&src).unwrap();

        for i in 1..=10 {
            fs::write(
                src.join(format!("module{}.rs", i)),
                format!("// TODO: clean up module {}\n", i),
            )
            .unwrap();
        }

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Week 1: Establish baseline - set budget to current count (10)
        let counts = r#"
[no-todo-comments]
"." = 10
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Week 2: Clean up 2 TODOs
        fs::remove_file(src.join("module1.rs")).unwrap();
        fs::remove_file(src.join("module2.rs")).unwrap();

        let tighten_exit = cli::tighten::run_tighten(Some("no-todo-comments"), None);
        assert_eq!(tighten_exit, cli::common::EXIT_SUCCESS);

        // Week 3: Clean up 3 more
        fs::remove_file(src.join("module3.rs")).unwrap();
        fs::remove_file(src.join("module4.rs")).unwrap();
        fs::remove_file(src.join("module5.rs")).unwrap();

        let tighten_exit = cli::tighten::run_tighten(Some("no-todo-comments"), None);
        assert_eq!(tighten_exit, cli::common::EXIT_SUCCESS);

        // Should still pass
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Week 4: Someone accidentally adds a new TODO
        fs::write(src.join("new_feature.rs"), "// TODO: implement\n").unwrap();

        // Check should fail (budget exceeded)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_EXCEEDED);
    });
}

#[test]
fn test_e2e_multiple_paths_check() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");
        create_todo_rule(temp_dir.path());

        // Create multiple directories
        let src = temp_dir.path().join("src");
        let tests = temp_dir.path().join("tests");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&tests).unwrap();

        fs::write(src.join("lib.rs"), "// TODO: src todo\n").unwrap();
        fs::write(tests.join("test.rs"), "// TODO: test todo\n").unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        let counts = r#"
[no-todo-comments]
"." = 2
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check specific paths
        let exit = cli::check::run_check(
            &["src".to_string(), "tests".to_string()],
            cli::OutputFormat::Human,
        );
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Check only src
        let exit = cli::check::run_check(&["src".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_e2e_jsonl_output_format() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");
        create_todo_rule(temp_dir.path());

        let src = temp_dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "// TODO: test\n").unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        let counts = r#"
[no-todo-comments]
"." = 1
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Check with JSONL format - should succeed
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Jsonl);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // List with JSONL format
        let list_exit = cli::list::run_list(cli::OutputFormat::Jsonl);
        assert_eq!(list_exit, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_e2e_tighten_with_violations_fails() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");
        create_todo_rule(temp_dir.path());

        fs::write(temp_dir.path().join("test.rs"), "// TODO: test\n").unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Set budget to 0 (below current count)
        let counts = r#"
[no-todo-comments]
"." = 0
"#;
        fs::write("ratchet-counts.toml", counts).unwrap();

        // Tighten should fail (violations exceed budget)
        let exit = cli::tighten::run_tighten(None, None);
        assert_eq!(exit, cli::common::EXIT_EXCEEDED);
    });
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_e2e_empty_project() {
    with_temp_dir(|_temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Check empty project - should succeed with warning
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_e2e_no_violations_found() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");
        create_todo_rule(temp_dir.path());

        // Create clean code
        let src = temp_dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();

        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = true
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Check should succeed (no violations)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);

        // Tighten should succeed but report no changes
        let tighten_exit = cli::tighten::run_tighten(None, None);
        assert_eq!(tighten_exit, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_e2e_rule_disabled_in_config() {
    with_temp_dir(|temp_dir| {
        // Initialize
        cli::init::run_init(false).expect("init should succeed");
        create_todo_rule(temp_dir.path());

        fs::write(
            temp_dir.path().join("test.rs"),
            "// TODO: this should be ignored\n",
        )
        .unwrap();

        // Disable the rule
        let config = r#"
[ratchet]
version = "1"
languages = ["rust"]
include = ["**/*.rs"]

[rules]
no-todo-comments = false
"#;
        fs::write("ratchet.toml", config).unwrap();

        // Check should succeed (rule disabled)
        let exit = cli::check::run_check(&[".".to_string()], cli::OutputFormat::Human);
        assert_eq!(exit, cli::common::EXIT_SUCCESS);
    });
}

#[test]
fn test_e2e_init_force_overwrites() {
    with_temp_dir(|temp_dir| {
        // First init
        cli::init::run_init(false).expect("init should succeed");

        // Modify config
        fs::write(temp_dir.path().join("ratchet.toml"), "modified content").unwrap();

        // Init without force should skip
        let result = cli::init::run_init(false).expect("init should succeed");
        assert!(result.skipped.contains(&"ratchet.toml".to_string()));

        let content = fs::read_to_string(temp_dir.path().join("ratchet.toml")).unwrap();
        assert_eq!(content, "modified content");

        // Init with force should overwrite
        let result = cli::init::run_init(true).expect("init should succeed");
        assert!(result.overwritten.contains(&"ratchet.toml".to_string()));

        let content = fs::read_to_string(temp_dir.path().join("ratchet.toml")).unwrap();
        assert!(content.contains("[ratchet]"));
    });
}
