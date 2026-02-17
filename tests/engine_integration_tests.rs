//! Integration tests for the execution engine
//!
//! This test suite verifies the complete execution pipeline:
//! - File discovery with FileWalker
//! - Parallel rule execution with ExecutionEngine
//! - Violation aggregation with ViolationAggregator
//! - Budget enforcement with CountsManager
//! - Region inheritance
//! - AST and regex rule interaction

use ratchets::config::counts::CountsManager;
use ratchets::engine::aggregator::ViolationAggregator;
use ratchets::engine::executor::ExecutionEngine;
use ratchets::engine::file_walker::{FileEntry, LanguageDetector};
use ratchets::rules::RuleRegistry;
use ratchets::types::{RegionPath, RuleId};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Helper to create a language detector for tests
fn test_detector() -> LanguageDetector {
    LanguageDetector::new()
}

/// Helper to create a test file with known content
fn create_test_file(dir: &Path, relative_path: &str, content: &str) -> PathBuf {
    let file_path = dir.join(relative_path);

    // Create parent directory if needed
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    fs::write(&file_path, content).unwrap();
    file_path
}

/// Helper to create a regex rule TOML file
fn create_regex_rule(dir: &Path, rule_id: &str, pattern: &str) -> PathBuf {
    let toml_content = format!(
        r#"
[rule]
id = "{}"
description = "Test rule"
severity = "warning"

[match]
pattern = "{}"
"#,
        rule_id, pattern
    );

    let rule_path = dir.join(format!("{}.toml", rule_id));
    fs::write(&rule_path, toml_content).unwrap();
    rule_path
}

#[test]
fn test_single_rule_single_file_within_budget() {
    let temp_dir = TempDir::new().unwrap();

    // Create test file
    let test_file = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        "// TODO: implement\nfn main() {}",
    );

    // Create rule
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Load rules
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    assert_eq!(registry.len(), 1);

    // Execute
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![FileEntry::new(test_file, &detector)];
    let result = engine.execute(files);

    // Verify execution
    assert_eq!(result.files_checked, 1);
    assert_eq!(result.rules_executed, 1);
    assert_eq!(result.violations.len(), 1);

    // Create budget using the actual region from the violation
    // The region will be the parent directory of the file
    let actual_region = result.violations[0].region.clone();

    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &actual_region, 1);

    // Aggregate and check budget
    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    assert!(
        agg_result.passed,
        "Should pass with 1 violation and budget of 1"
    );
    assert_eq!(agg_result.total_violations, 1);
    assert_eq!(agg_result.violations_over_budget, 0);
}

#[test]
fn test_single_rule_single_file_over_budget() {
    let temp_dir = TempDir::new().unwrap();

    // Create test file with 2 TODOs
    let test_file = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        "// TODO: implement\n// TODO: also this\nfn main() {}",
    );

    // Create rule
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Load and execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![FileEntry::new(test_file, &detector)];
    let result = engine.execute(files);

    assert_eq!(result.violations.len(), 2);

    // Create budget with only 1 allowed using actual region
    let actual_region = result.violations[0].region.clone();
    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &actual_region, 1);

    // Aggregate and check budget
    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    assert!(
        !agg_result.passed,
        "Should fail with 2 violations and budget of 1"
    );
    assert_eq!(agg_result.total_violations, 2);
    assert_eq!(agg_result.violations_over_budget, 1);
}

#[test]
fn test_multiple_rules_multiple_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple test files
    let file1 = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        "// TODO: implement\nfn main() {}",
    );
    let file2 = create_test_file(
        temp_dir.path(),
        "src/lib.rs",
        "// FIXME: broken\nfn lib() {}",
    );
    let file3 = create_test_file(
        temp_dir.path(),
        "tests/test.rs",
        "// TODO: add test\n// FIXME: test fails\nfn test() {}",
    );

    // Create multiple rules
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");
    create_regex_rule(&rules_dir, "no-fixme", "FIXME");

    // Load and execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    assert_eq!(registry.len(), 2);

    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![
        FileEntry::new(file1, &detector),
        FileEntry::new(file2, &detector),
        FileEntry::new(file3, &detector),
    ];
    let result = engine.execute(files);

    // Verify execution
    assert_eq!(result.files_checked, 3);
    assert_eq!(result.rules_executed, 2);
    // 2 TODOs + 2 FIXMEs = 4 total
    assert_eq!(result.violations.len(), 4);

    // Extract actual regions from violations for proper budget setting
    let src_region = result
        .violations
        .iter()
        .find(|v| v.file.to_string_lossy().contains("src/"))
        .map(|v| v.region.clone())
        .unwrap();
    let test_region = result
        .violations
        .iter()
        .find(|v| v.file.to_string_lossy().contains("tests/"))
        .map(|v| v.region.clone())
        .unwrap();

    // Create budget
    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &src_region, 0);
    counts.set_count(&RuleId::new("no-todo").unwrap(), &test_region, 10);
    counts.set_count(&RuleId::new("no-fixme").unwrap(), &src_region, 10);
    counts.set_count(&RuleId::new("no-fixme").unwrap(), &test_region, 10);

    // Aggregate
    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    // Should fail because no-todo has budget 0 in src, but has 1 violation
    assert!(!agg_result.passed);
    assert_eq!(agg_result.total_violations, 4);
}

#[test]
fn test_region_specific_budgets() {
    let temp_dir = TempDir::new().unwrap();

    // Create files in different regions
    let src_file = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        "// TODO: clean code\nfn main() {}",
    );
    let legacy_file = create_test_file(
        temp_dir.path(),
        "src/legacy/old.rs",
        "// TODO: fix\n// TODO: refactor\n// TODO: cleanup\nfn old() {}",
    );

    // Create rule
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Load and execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![
        FileEntry::new(src_file, &detector),
        FileEntry::new(legacy_file, &detector),
    ];
    let result = engine.execute(files);

    assert_eq!(result.violations.len(), 4); // 1 in src + 3 in src/legacy

    // Extract actual regions from violations
    let src_region = result
        .violations
        .iter()
        .find(|v| v.file.to_string_lossy().contains("src/main.rs"))
        .map(|v| v.region.clone())
        .unwrap();
    let legacy_region = result
        .violations
        .iter()
        .find(|v| v.file.to_string_lossy().contains("src/legacy/"))
        .map(|v| v.region.clone())
        .unwrap();

    // Create budget with strict src, lenient legacy
    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &src_region, 0);
    counts.set_count(&RuleId::new("no-todo").unwrap(), &legacy_region, 5);

    // Aggregate
    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    // Should fail because src/main.rs has 1 violation with budget 0
    assert!(!agg_result.passed);

    // Check individual statuses - we should have 2 groups
    assert_eq!(agg_result.statuses.len(), 2);

    let src_status = agg_result
        .statuses
        .iter()
        .find(|s| s.region == src_region)
        .unwrap();
    assert_eq!(src_status.actual_count, 1);
    assert_eq!(src_status.budget, 0);
    assert!(!src_status.passed);

    let legacy_status = agg_result
        .statuses
        .iter()
        .find(|s| s.region == legacy_region)
        .unwrap();
    assert_eq!(legacy_status.actual_count, 3);
    assert_eq!(legacy_status.budget, 5);
    assert!(legacy_status.passed);
}

#[test]
fn test_empty_file_set() {
    let temp_dir = TempDir::new().unwrap();

    // Create rule but no files
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Load and execute with empty file list
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(vec![]);

    // Should succeed with no violations
    assert_eq!(result.files_checked, 0);
    assert_eq!(result.rules_executed, 1);
    assert_eq!(result.violations.len(), 0);

    // Aggregate should pass
    let counts = CountsManager::new();
    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    assert!(agg_result.passed);
    assert_eq!(agg_result.total_violations, 0);
}

#[test]
fn test_files_with_no_violations() {
    let temp_dir = TempDir::new().unwrap();

    // Create files with no violations
    let file1 = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        "fn main() { println!(\"Hello\"); }",
    );
    let file2 = create_test_file(temp_dir.path(), "src/lib.rs", "pub fn lib() { }");

    // Create rule looking for TODO
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Load and execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![
        FileEntry::new(file1, &detector),
        FileEntry::new(file2, &detector),
    ];
    let result = engine.execute(files);

    // Should find no violations
    assert_eq!(result.files_checked, 2);
    assert_eq!(result.rules_executed, 1);
    assert_eq!(result.violations.len(), 0);

    // Aggregate should pass even with strict budget
    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &RegionPath::new("."), 0);

    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    assert!(agg_result.passed);
    assert_eq!(agg_result.total_violations, 0);
}

#[test]
fn test_parallel_execution_deterministic() {
    let temp_dir = TempDir::new().unwrap();

    // Create many files to ensure parallel execution
    let mut files = Vec::new();
    let detector = test_detector();
    for i in 0..50 {
        let file = create_test_file(
            temp_dir.path(),
            &format!("src/file{}.rs", i),
            &format!("// TODO: file {}\nfn test{}() {{}}", i, i),
        );
        files.push(FileEntry::new(file, &detector));
    }

    // Create rule
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Execute multiple times by reloading the registry each time
    let mut registry1 = RuleRegistry::new();
    registry1.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine1 = ExecutionEngine::new(registry1, None);
    let result1 = engine1.execute(files.clone());

    let mut registry2 = RuleRegistry::new();
    registry2.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine2 = ExecutionEngine::new(registry2, None);
    let result2 = engine2.execute(files.clone());

    let mut registry3 = RuleRegistry::new();
    registry3.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine3 = ExecutionEngine::new(registry3, None);
    let result3 = engine3.execute(files);

    // All runs should find the same violations
    assert_eq!(result1.violations.len(), 50);
    assert_eq!(result2.violations.len(), 50);
    assert_eq!(result3.violations.len(), 50);

    // Sort violations by file path for comparison
    let mut v1 = result1.violations;
    let mut v2 = result2.violations;
    let mut v3 = result3.violations;

    v1.sort_by(|a, b| a.file.cmp(&b.file));
    v2.sort_by(|a, b| a.file.cmp(&b.file));
    v3.sort_by(|a, b| a.file.cmp(&b.file));

    // Should be identical
    assert_eq!(v1, v2);
    assert_eq!(v2, v3);
}

#[test]
fn test_parallel_execution_no_race_conditions() {
    let temp_dir = TempDir::new().unwrap();

    // Create files that would be processed by different threads
    let mut files = Vec::new();
    let detector = test_detector();
    for i in 0..100 {
        let file = create_test_file(
            temp_dir.path(),
            &format!("src/module{}/file{}.rs", i / 10, i),
            &format!("// TODO: {}\nfn f{}() {{}}", i, i),
        );
        files.push(FileEntry::new(file, &detector));
    }

    // Create rule
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Load and execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(files);

    // Should process all files correctly
    assert_eq!(result.files_checked, 100);
    assert_eq!(result.violations.len(), 100);

    // Each violation should have correct file path
    for violation in &result.violations {
        assert!(violation.file.to_string_lossy().contains("src/module"));
        assert_eq!(violation.line, 1); // All TODOs are on line 1
    }
}

#[cfg(feature = "lang-rust")]
#[test]
fn test_ast_and_regex_rules_together() {
    let temp_dir = TempDir::new().unwrap();

    // Create a file with both types of violations
    let test_file = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        r#"
// TODO: refactor
fn main() {
    let x = Some(42);
    x.unwrap(); // AST violation
}
"#,
    );

    // Create regex rule
    let regex_dir = temp_dir.path().join("regex_rules");
    fs::create_dir(&regex_dir).unwrap();
    create_regex_rule(&regex_dir, "no-todo", "TODO");

    // Create AST rule
    let ast_dir = temp_dir.path().join("ast_rules");
    fs::create_dir(&ast_dir).unwrap();
    let ast_rule_toml = r#"
[rule]
id = "no-unwrap"
description = "No unwrap calls"
severity = "error"

[match]
language = "rust"
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
"#;
    fs::write(ast_dir.join("no-unwrap.toml"), ast_rule_toml).unwrap();

    // Load both types of rules
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&regex_dir, None).unwrap();
    registry.load_custom_ast_rules(&ast_dir, None).unwrap();
    assert_eq!(registry.len(), 2);

    // Execute
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![FileEntry::new(test_file, &detector)];
    let result = engine.execute(files);

    // Should find both violations
    assert_eq!(result.files_checked, 1);
    assert_eq!(result.rules_executed, 2);
    assert_eq!(
        result.violations.len(),
        2,
        "Should find both TODO and unwrap violations"
    );

    // Verify we have both rule IDs
    let rule_ids: Vec<_> = result
        .violations
        .iter()
        .map(|v| v.rule_id.as_str())
        .collect();
    assert!(rule_ids.contains(&"no-todo"));
    assert!(rule_ids.contains(&"no-unwrap"));
}

#[test]
fn test_budget_enforcement_pass() {
    let temp_dir = TempDir::new().unwrap();

    // Create file with 2 violations
    let test_file = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        "// TODO: one\n// TODO: two\nfn main() {}",
    );

    // Create rule
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![FileEntry::new(test_file, &detector)];
    let result = engine.execute(files);

    assert_eq!(result.violations.len(), 2);

    // Set budget to exactly 2 (should pass) using actual region
    let actual_region = result.violations[0].region.clone();
    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &actual_region, 2);

    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    assert!(agg_result.passed, "Should pass when actual == budget");
    assert_eq!(agg_result.total_violations, 2);
    assert_eq!(agg_result.violations_over_budget, 0);
}

#[test]
fn test_budget_enforcement_fail() {
    let temp_dir = TempDir::new().unwrap();

    // Create file with 3 violations
    let test_file = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        "// TODO: one\n// TODO: two\n// TODO: three\nfn main() {}",
    );

    // Create rule
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![FileEntry::new(test_file, &detector)];
    let result = engine.execute(files);

    assert_eq!(result.violations.len(), 3);

    // Set budget to 2 (should fail) using actual region
    let actual_region = result.violations[0].region.clone();
    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &actual_region, 2);

    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    assert!(!agg_result.passed, "Should fail when actual > budget");
    assert_eq!(agg_result.total_violations, 3);
    assert_eq!(agg_result.violations_over_budget, 1);
}

#[test]
fn test_region_inheritance_chain() {
    let temp_dir = TempDir::new().unwrap();

    // Create files in nested regions
    let root_file = create_test_file(temp_dir.path(), "main.rs", "// TODO: root\nfn main() {}");
    let src_file = create_test_file(temp_dir.path(), "src/lib.rs", "// TODO: src\nfn lib() {}");
    let nested_file = create_test_file(
        temp_dir.path(),
        "src/legacy/parser/old.rs",
        "// TODO: nested\nfn old() {}",
    );

    // Create rule
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");

    // Execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![
        FileEntry::new(root_file, &detector),
        FileEntry::new(src_file, &detector),
        FileEntry::new(nested_file, &detector),
    ];
    let result = engine.execute(files);

    assert_eq!(result.violations.len(), 3);

    // Extract actual regions
    let root_region = result
        .violations
        .iter()
        .find(|v| v.file.file_name().unwrap() == "main.rs")
        .map(|v| v.region.clone())
        .unwrap();
    let src_region = result
        .violations
        .iter()
        .find(|v| v.file.to_string_lossy().contains("src/lib.rs"))
        .map(|v| v.region.clone())
        .unwrap();
    let nested_region = result
        .violations
        .iter()
        .find(|v| v.file.to_string_lossy().contains("src/legacy/parser/"))
        .map(|v| v.region.clone())
        .unwrap();

    // For testing region inheritance, we need to set budgets using the parent directories
    // The CountsManager.get_budget() will handle inheritance from parent to child paths
    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &root_region, 0);
    counts.set_count(&RuleId::new("no-todo").unwrap(), &src_region, 10);
    counts.set_count(&RuleId::new("no-todo").unwrap(), &nested_region, 10);

    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    // Should fail because root has 1 violation with budget 0
    assert!(!agg_result.passed);

    // Verify the root region status failed
    let root_status = agg_result
        .statuses
        .iter()
        .find(|s| s.region == root_region)
        .unwrap();
    assert_eq!(root_status.budget, 0);
    assert!(!root_status.passed);
}

#[test]
fn test_aggregation_groups_by_rule_and_region() {
    let temp_dir = TempDir::new().unwrap();

    // Create files in different regions with different rules
    let src1 = create_test_file(temp_dir.path(), "src/a.rs", "// TODO: a\nfn a() {}");
    let src2 = create_test_file(
        temp_dir.path(),
        "src/b.rs",
        "// TODO: b\n// FIXME: b\nfn b() {}",
    );
    let tests = create_test_file(temp_dir.path(), "tests/t.rs", "// TODO: t\nfn t() {}");

    // Create multiple rules
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();
    create_regex_rule(&rules_dir, "no-todo", "TODO");
    create_regex_rule(&rules_dir, "no-fixme", "FIXME");

    // Execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![
        FileEntry::new(src1, &detector),
        FileEntry::new(src2, &detector),
        FileEntry::new(tests, &detector),
    ];
    let result = engine.execute(files);

    // Should find: 3 TODOs (2 in src, 1 in tests) + 1 FIXME (in src)
    assert_eq!(result.violations.len(), 4);

    // Extract actual regions from violations
    let src_region = result
        .violations
        .iter()
        .find(|v| v.file.to_string_lossy().contains("src/"))
        .map(|v| v.region.clone())
        .unwrap();
    let test_region = result
        .violations
        .iter()
        .find(|v| v.file.to_string_lossy().contains("tests/"))
        .map(|v| v.region.clone())
        .unwrap();

    // Set budgets
    let mut counts = CountsManager::new();
    counts.set_count(&RuleId::new("no-todo").unwrap(), &src_region, 10);
    counts.set_count(&RuleId::new("no-todo").unwrap(), &test_region, 10);
    counts.set_count(&RuleId::new("no-fixme").unwrap(), &src_region, 10);

    let aggregator = ViolationAggregator::new(counts);
    let agg_result = aggregator.aggregate(result.violations);

    // Should have 3 groups: (no-todo, src), (no-todo, tests), (no-fixme, src)
    assert_eq!(agg_result.statuses.len(), 3);
    assert!(agg_result.passed);

    // Verify grouping using actual regions
    let todo_src = agg_result
        .statuses
        .iter()
        .find(|s| s.rule_id.as_str() == "no-todo" && s.region == src_region)
        .unwrap();
    assert_eq!(todo_src.actual_count, 2);

    let todo_tests = agg_result
        .statuses
        .iter()
        .find(|s| s.rule_id.as_str() == "no-todo" && s.region == test_region)
        .unwrap();
    assert_eq!(todo_tests.actual_count, 1);

    let fixme_src = agg_result
        .statuses
        .iter()
        .find(|s| s.rule_id.as_str() == "no-fixme" && s.region == src_region)
        .unwrap();
    assert_eq!(fixme_src.actual_count, 1);
}

#[test]
fn test_no_rules_loaded() {
    let temp_dir = TempDir::new().unwrap();

    // Create test file
    let test_file = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        "// TODO: this should not be detected\nfn main() {}",
    );

    // Create engine with empty registry
    let registry = RuleRegistry::new();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![FileEntry::new(test_file, &detector)];
    let result = engine.execute(files);

    // Should process file but find no violations
    assert_eq!(result.files_checked, 1);
    assert_eq!(result.rules_executed, 0);
    assert_eq!(result.violations.len(), 0);
}

#[test]
fn test_file_without_matching_rules() {
    let temp_dir = TempDir::new().unwrap();

    // Create Python file
    let test_file = create_test_file(
        temp_dir.path(),
        "src/script.py",
        "# TODO: python file\ndef main():\n    pass",
    );

    // Create Rust-specific rule (won't match Python file)
    let rules_dir = temp_dir.path().join("rules");
    fs::create_dir(&rules_dir).unwrap();

    let rust_only_toml = r#"
[rule]
id = "rust-only"
description = "Rust only rule"
severity = "warning"

[match]
pattern = "TODO"
languages = ["rust"]
"#;
    fs::write(rules_dir.join("rust-only.toml"), rust_only_toml).unwrap();

    // Execute
    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rules_dir, None).unwrap();
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![FileEntry::new(test_file, &detector)];
    let result = engine.execute(files);

    // Should process file but find no violations (rule doesn't apply to Python)
    assert_eq!(result.files_checked, 1);
    assert_eq!(result.rules_executed, 1);
    assert_eq!(result.violations.len(), 0);
}

#[cfg(feature = "lang-rust")]
#[test]
fn test_ast_caching_single_file() {
    let temp_dir = TempDir::new().unwrap();

    // Create file with multiple AST violations
    let test_file = create_test_file(
        temp_dir.path(),
        "src/main.rs",
        r#"
fn main() {
    let a = Some(1).unwrap();
    let b = Some(2).expect("msg");
    panic!("oh no");
}
"#,
    );

    // Create multiple AST rules
    let ast_dir = temp_dir.path().join("ast_rules");
    fs::create_dir(&ast_dir).unwrap();

    let no_unwrap = r#"
[rule]
id = "no-unwrap"
description = "No unwrap"
severity = "error"

[match]
language = "rust"
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
"#;
    fs::write(ast_dir.join("no-unwrap.toml"), no_unwrap).unwrap();

    let no_expect = r#"
[rule]
id = "no-expect"
description = "No expect"
severity = "error"

[match]
language = "rust"
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "expect")) @violation
"""
"#;
    fs::write(ast_dir.join("no-expect.toml"), no_expect).unwrap();

    let no_panic = r#"
[rule]
id = "no-panic"
description = "No panic"
severity = "error"

[match]
language = "rust"
query = """
(macro_invocation
  macro: (identifier) @macro_name
  (#eq? @macro_name "panic")) @violation
"""
"#;
    fs::write(ast_dir.join("no-panic.toml"), no_panic).unwrap();

    // Load all AST rules
    let mut registry = RuleRegistry::new();
    registry.load_custom_ast_rules(&ast_dir, None).unwrap();
    assert_eq!(registry.len(), 3);

    // Execute - the engine should parse the AST once and reuse it for all 3 rules
    let engine = ExecutionEngine::new(registry, None);
    let detector = test_detector();
    let files = vec![FileEntry::new(test_file, &detector)];
    let result = engine.execute(files);

    // Should find all 3 violations (1 unwrap, 1 expect, 1 panic)
    assert_eq!(result.files_checked, 1);
    assert_eq!(result.rules_executed, 3);
    assert_eq!(
        result.violations.len(),
        3,
        "Should find unwrap, expect, and panic"
    );

    // Verify all three rule types were detected
    let rule_ids: Vec<_> = result
        .violations
        .iter()
        .map(|v| v.rule_id.as_str())
        .collect();
    assert!(rule_ids.contains(&"no-unwrap"));
    assert!(rule_ids.contains(&"no-expect"));
    assert!(rule_ids.contains(&"no-panic"));
}
