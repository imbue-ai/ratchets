#![forbid(unsafe_code)]

//! Integration tests for regex rule execution
//!
//! These tests verify that regex rules work correctly with actual built-in rules
//! and fixture files.

use ratchets::rules::{ExecutionContext, RegexRule, Rule, RuleRegistry};
use ratchets::types::{RuleId, Severity};
use std::path::{Path, PathBuf};

/// Helper function to get the fixtures directory path
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rules")
}

/// Helper function to get the builtin rules directory path
fn builtin_rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("builtin-ratchets")
        .join("common")
        .join("regex")
}

/// Helper function to load a fixture file's content
fn load_fixture(filename: &str) -> String {
    let path = fixtures_dir().join(filename);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", filename, e))
}

/// Helper function to load a built-in rule
fn load_builtin_rule(rule_name: &str) -> RegexRule {
    let path = builtin_rules_dir().join(format!("{}.toml", rule_name));
    RegexRule::from_path(&path)
        .unwrap_or_else(|e| panic!("Failed to load built-in rule {}: {}", rule_name, e))
}

#[test]
fn test_todo_rule_finds_todos() {
    // Load the no-todo-comments built-in rule
    let rule = load_builtin_rule("no-todo-comments");

    // Verify rule metadata
    assert_eq!(rule.id().as_str(), "no-todo-comments");
    assert_eq!(rule.severity(), Severity::Warning);
    assert_eq!(rule.description(), "Disallow TODO comments in code");

    // Load fixture with TODO comments
    let content = load_fixture("todo_violations.rs");
    let file_path = fixtures_dir().join("todo_violations.rs");

    let ctx = ExecutionContext {
        file_path: &file_path,
        content: &content,
        ast: None,
        region_resolver: None,
    };

    // Execute rule
    let violations = rule.execute(&ctx);

    // Should find 5 TODO comments (TODO, todo, Todo, TODO, TODO)
    assert_eq!(violations.len(), 5, "Expected 5 TODO violations");

    // Verify first violation
    assert_eq!(violations[0].line, 3);
    assert_eq!(violations[0].snippet, "TODO");
    assert_eq!(violations[0].message, "Disallow TODO comments in code");

    // Verify case-insensitive matching
    let snippets: Vec<&str> = violations.iter().map(|v| v.snippet.as_str()).collect();
    assert!(snippets.contains(&"TODO"));
    assert!(snippets.contains(&"todo"));
    assert!(snippets.contains(&"Todo"));
}

#[test]
fn test_fixme_rule_finds_fixmes() {
    // Load the no-fixme-comments built-in rule
    let rule = load_builtin_rule("no-fixme-comments");

    // Verify rule metadata
    assert_eq!(rule.id().as_str(), "no-fixme-comments");
    assert_eq!(rule.severity(), Severity::Warning);
    assert_eq!(rule.description(), "Disallow FIXME comments in code");

    // Load fixture with FIXME comments
    let content = load_fixture("fixme_violations.py");
    let file_path = fixtures_dir().join("fixme_violations.py");

    let ctx = ExecutionContext {
        file_path: &file_path,
        content: &content,
        ast: None,
        region_resolver: None,
    };

    // Execute rule
    let violations = rule.execute(&ctx);

    // Should find 4 FIXME comments (FIXME, fixme, FixMe, FIXME)
    assert_eq!(violations.len(), 4, "Expected 4 FIXME violations");

    // Verify positions are on correct lines
    let lines: Vec<u32> = violations.iter().map(|v| v.line).collect();
    assert!(lines.contains(&4));
    assert!(lines.contains(&8));
    assert!(lines.contains(&10));
    assert!(lines.contains(&13));
}

#[test]
fn test_position_accuracy() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = "line1\n// TODO: fix\nline3";
    let file_path = Path::new("test.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);
    assert_eq!(violations.len(), 1);

    let v = &violations[0];
    // TODO starts at line 2, column 4 (after "// ")
    assert_eq!(v.line, 2, "Line should be 2");
    assert_eq!(v.column, 4, "Column should be 4 (1-indexed, after '// ')");

    // Verify snippet
    assert_eq!(v.snippet, "TODO");
}

#[test]
fn test_position_accuracy_first_line() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = "TODO: first line";
    let file_path = Path::new("test.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);
    assert_eq!(violations.len(), 1);

    let v = &violations[0];
    // TODO starts at line 1, column 1
    assert_eq!(v.line, 1, "Line should be 1");
    assert_eq!(v.column, 1, "Column should be 1");
    assert_eq!(v.snippet, "TODO");
}

#[test]
fn test_multiline_content() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = load_fixture("multiline.ts");
    let file_path = fixtures_dir().join("multiline.ts");

    let ctx = ExecutionContext {
        file_path: &file_path,
        content: &content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);

    // Should find TODOs on lines 2, 14, and 20
    assert_eq!(violations.len(), 3, "Expected 3 TODO violations");
    assert_eq!(violations[0].line, 2);
    assert_eq!(violations[1].line, 14);
    assert_eq!(violations[2].line, 20);
}

#[test]
fn test_case_insensitive_matching() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = "// TODO uppercase\n// todo lowercase\n// Todo mixedcase\n// tOdO weird";
    let file_path = Path::new("test.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);

    // All four should match due to (?i) flag
    assert_eq!(
        violations.len(),
        4,
        "Expected 4 violations (case-insensitive)"
    );

    // Verify each line
    assert_eq!(violations[0].line, 1);
    assert_eq!(violations[1].line, 2);
    assert_eq!(violations[2].line, 3);
    assert_eq!(violations[3].line, 4);
}

#[test]
fn test_snippet_extraction_accuracy() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = "prefix TODO suffix";
    let file_path = Path::new("test.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);
    assert_eq!(violations.len(), 1);

    // Snippet should be exactly "TODO", not "prefix TODO suffix"
    assert_eq!(violations[0].snippet, "TODO");
    assert_eq!(violations[0].column, 8); // "prefix " is 7 chars, TODO starts at position 8
}

#[test]
fn test_no_violations_returns_empty() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = load_fixture("no_violations.rs");
    let file_path = fixtures_dir().join("no_violations.rs");

    let ctx = ExecutionContext {
        file_path: &file_path,
        content: &content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);

    // Clean file should have no violations
    assert_eq!(violations.len(), 0, "Expected no violations in clean file");
}

#[test]
fn test_multiple_violations_same_file() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = load_fixture("complex.js");
    let file_path = fixtures_dir().join("complex.js");

    let ctx = ExecutionContext {
        file_path: &file_path,
        content: &content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);

    // complex.js has TODO on lines 2, 7, 14, 16, and 17 (5 total)
    assert!(violations.len() >= 4, "Expected at least 4 TODO violations");

    // Verify they are on different lines
    let lines: Vec<u32> = violations.iter().map(|v| v.line).collect();
    assert!(lines.contains(&2));
    assert!(lines.contains(&7));

    // Verify all snippets match TODO (case-insensitive variants)
    for v in &violations {
        assert!(
            v.snippet.to_uppercase() == "TODO",
            "All snippets should be TODO variants"
        );
    }
}

#[test]
fn test_registry_with_builtin_rules() {
    let mut registry = RuleRegistry::new();

    // Load built-in rules
    let builtin_dir = builtin_rules_dir();
    registry
        .load_builtin_regex_rules(&builtin_dir)
        .expect("Failed to load built-in rules");

    // Should have at least 2 rules (no-todo-comments and no-fixme-comments)
    assert!(registry.len() >= 2, "Expected at least 2 built-in rules");

    // Verify specific rules exist
    let todo_rule_id = RuleId::new("no-todo-comments").unwrap();
    let fixme_rule_id = RuleId::new("no-fixme-comments").unwrap();

    assert!(
        registry.get_rule(&todo_rule_id).is_some(),
        "no-todo-comments rule should be loaded"
    );
    assert!(
        registry.get_rule(&fixme_rule_id).is_some(),
        "no-fixme-comments rule should be loaded"
    );
}

#[test]
fn test_registry_execute_all_rules() {
    let mut registry = RuleRegistry::new();
    let builtin_dir = builtin_rules_dir();
    registry
        .load_builtin_regex_rules(&builtin_dir)
        .expect("Failed to load built-in rules");

    // Load fixture with both TODO and FIXME
    let content = load_fixture("complex.js");
    let file_path = fixtures_dir().join("complex.js");

    let ctx = ExecutionContext {
        file_path: &file_path,
        content: &content,
        ast: None,
        region_resolver: None,
    };

    // Execute all rules
    let mut all_violations = Vec::new();
    for rule in registry.iter_rules() {
        let mut violations = rule.execute(&ctx);
        all_violations.append(&mut violations);
    }

    // Should find both TODO and FIXME violations
    assert!(
        all_violations.len() >= 7,
        "Expected at least 7 total violations (TODOs + FIXMEs)"
    );

    // Verify we have violations from both rules
    let todo_violations: Vec<_> = all_violations
        .iter()
        .filter(|v| v.rule_id.as_str() == "no-todo-comments")
        .collect();
    let fixme_violations: Vec<_> = all_violations
        .iter()
        .filter(|v| v.rule_id.as_str() == "no-fixme-comments")
        .collect();

    assert!(!todo_violations.is_empty(), "Should have TODO violations");
    assert!(!fixme_violations.is_empty(), "Should have FIXME violations");
}

#[test]
fn test_rule_severity() {
    let rule = load_builtin_rule("no-todo-comments");

    // Built-in rules should have Warning severity
    assert_eq!(rule.severity(), Severity::Warning);
}

#[test]
fn test_rule_languages() {
    let rule = load_builtin_rule("no-todo-comments");

    // Built-in regex rules should apply to all languages (empty list)
    assert!(
        rule.languages().is_empty(),
        "Built-in regex rules should apply to all languages"
    );
}

#[test]
fn test_end_line_and_column() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = "TODO";
    let file_path = Path::new("test.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);
    assert_eq!(violations.len(), 1);

    let v = &violations[0];
    // "TODO" is 4 characters, starting at (1,1), ending at (1,5)
    assert_eq!(v.line, 1);
    assert_eq!(v.column, 1);
    assert_eq!(v.end_line, 1);
    assert_eq!(v.end_column, 5); // Position after last character
}

#[test]
fn test_violation_file_path() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = "TODO: test";
    let file_path = fixtures_dir().join("test.rs");

    let ctx = ExecutionContext {
        file_path: &file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);
    assert_eq!(violations.len(), 1);

    // Verify file path is preserved correctly
    assert_eq!(violations[0].file, file_path);
}

#[test]
fn test_multiple_rules_same_content() {
    let todo_rule = load_builtin_rule("no-todo-comments");
    let fixme_rule = load_builtin_rule("no-fixme-comments");

    let content = "// TODO: fix\n// FIXME: also fix";
    let file_path = Path::new("test.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    // Execute both rules
    let todo_violations = todo_rule.execute(&ctx);
    let fixme_violations = fixme_rule.execute(&ctx);

    // Each rule should find its own pattern
    assert_eq!(todo_violations.len(), 1);
    assert_eq!(fixme_violations.len(), 1);

    // Verify rule IDs are different
    assert_eq!(todo_violations[0].rule_id.as_str(), "no-todo-comments");
    assert_eq!(fixme_violations[0].rule_id.as_str(), "no-fixme-comments");
}

#[test]
fn test_word_boundary_matching() {
    let rule = load_builtin_rule("no-todo-comments");

    // The pattern uses \b word boundaries, so "ATODO" or "TODOZ" should not match
    let content = "ATODO TODOZ XTODOX TODO";
    let file_path = Path::new("test.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);

    // Only the standalone "TODO" should match
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].column, 20); // Position of standalone TODO (1-indexed)
}

#[test]
fn test_empty_file() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = "";
    let file_path = Path::new("empty.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);

    // Empty file should have no violations
    assert_eq!(violations.len(), 0);
}

#[test]
fn test_only_whitespace() {
    let rule = load_builtin_rule("no-todo-comments");
    let content = "   \n\n   \n";
    let file_path = Path::new("whitespace.rs");

    let ctx = ExecutionContext {
        file_path,
        content,
        ast: None,
        region_resolver: None,
    };

    let violations = rule.execute(&ctx);

    // Whitespace-only file should have no violations
    assert_eq!(violations.len(), 0);
}
