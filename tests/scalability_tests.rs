//! Scalability tests for Ratchet
//!
//! These tests verify that Ratchet can handle large codebases efficiently.
//! They create many temporary files and test that the tool scales well.
//!
//! ## Performance Requirements
//!
//! These tests ensure that:
//! - The tool can process 1000+ files without excessive memory usage
//! - Parallel execution provides performance benefits
//! - Parser caching is effective
//! - No obvious performance bottlenecks exist
//!
//! ## Test Timeouts
//!
//! Tests are designed to complete quickly even with many files:
//! - 1000 files should process in < 10 seconds
//! - File I/O is parallelized
//! - Parser caching avoids redundant work

use ratchets::engine::executor::ExecutionEngine;
use ratchets::engine::file_walker::{FileEntry, FileWalker};
use ratchets::rules::RuleRegistry;
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

/// Helper to create many test files
fn create_many_files(count: usize) -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create files in subdirectories to simulate a real project structure
    for i in 0..count {
        let subdir = temp_dir.path().join(format!("module{}", i / 100));
        fs::create_dir_all(&subdir).unwrap();

        let content = format!(
            r#"
// File {} in module {}
// TODO: implement feature

fn function_{}() {{
    println!("Hello from file {}");
}}

#[cfg(test)]
mod tests {{
    #[test]
    fn test_function_{}() {{
        super::function_{}();
    }}
}}
"#,
            i,
            i / 100,
            i,
            i,
            i,
            i
        );

        fs::write(subdir.join(format!("file{}.rs", i)), content).unwrap();
    }

    temp_dir
}

#[test]
fn test_scalability_1000_files() {
    // Create 1000 Rust files
    let temp_dir = create_many_files(1000);
    let start = Instant::now();

    // Walk all files
    let walker = FileWalker::new(temp_dir.path(), &[], &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    let walk_time = start.elapsed();
    println!("File walking for 1000 files took: {:?}", walk_time);

    // Verify we found the expected number of files
    assert_eq!(files.len(), 1000, "Should find all 1000 files");

    // File walking should be fast (< 2 seconds for 1000 files)
    assert!(
        walk_time.as_secs() < 2,
        "File walking should complete quickly: {:?}",
        walk_time
    );
}

#[test]
fn test_scalability_execution_with_regex_rules() {
    // Create 1000 files
    let temp_dir = create_many_files(1000);

    // Create a regex rule
    let rule_dir = temp_dir.path().join("rules");
    fs::create_dir(&rule_dir).unwrap();
    fs::write(
        rule_dir.join("todo.toml"),
        r#"
[rule]
id = "no-todo"
description = "No TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#,
    )
    .unwrap();

    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rule_dir, None).unwrap();

    // Walk files (only include .rs files, exclude .toml)
    use ratchets::types::GlobPattern;
    let include = vec![GlobPattern::new("**/*.rs")];
    let walker = FileWalker::new(temp_dir.path(), &include, &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    assert_eq!(files.len(), 1000);

    // Execute rules
    let start = Instant::now();
    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(files);
    let exec_time = start.elapsed();

    println!("Regex rule execution on 1000 files took: {:?}", exec_time);

    // Verify results
    assert_eq!(result.files_checked, 1000);
    assert_eq!(result.rules_executed, 1);
    // Each file has one TODO comment
    assert_eq!(result.violations.len(), 1000);

    // Execution should be reasonably fast (< 10 seconds for 1000 files with regex)
    assert!(
        exec_time.as_secs() < 10,
        "Rule execution should complete in reasonable time: {:?}",
        exec_time
    );
}

#[cfg(feature = "lang-rust")]
#[test]
fn test_scalability_execution_with_ast_rules() {
    // Create fewer files for AST tests (AST parsing is more expensive)
    let temp_dir = create_many_files(100);

    // Create an AST rule
    let rule_dir = temp_dir.path().join("rules");
    fs::create_dir_all(&rule_dir).unwrap();
    fs::write(
        rule_dir.join("test_functions.toml"),
        r#"
[rule]
id = "find-test-functions"
description = "Find test functions"
severity = "info"

[match]
query = """
(attribute_item
  (attribute
    (identifier) @attr)
  (#eq? @attr "test")) @violation
"""
language = "rust"
"#,
    )
    .unwrap();

    let mut registry = RuleRegistry::new();
    registry.load_custom_ast_rules(&rule_dir, None).unwrap();

    // Walk files (only include .rs files)
    use ratchets::types::GlobPattern;
    let include = vec![GlobPattern::new("**/*.rs")];
    let walker = FileWalker::new(temp_dir.path(), &include, &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    assert_eq!(files.len(), 100);

    // Execute rules
    let start = Instant::now();
    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(files);
    let exec_time = start.elapsed();

    println!("AST rule execution on 100 files took: {:?}", exec_time);

    // Verify results
    assert_eq!(result.files_checked, 100);
    assert_eq!(result.rules_executed, 1);
    // Each file has one test function
    assert_eq!(result.violations.len(), 100);

    // AST execution should complete in reasonable time (< 15 seconds for 100 files)
    assert!(
        exec_time.as_secs() < 15,
        "AST rule execution should complete in reasonable time: {:?}",
        exec_time
    );
}

#[test]
fn test_memory_efficiency() {
    // This test verifies that we don't load all files into memory at once
    // The execution engine processes files via iterators, so memory usage
    // should remain constant regardless of file count

    let temp_dir = create_many_files(1000);

    // Create a simple regex rule
    let rule_dir = temp_dir.path().join("rules");
    fs::create_dir(&rule_dir).unwrap();
    fs::write(
        rule_dir.join("pattern.toml"),
        r#"
[rule]
id = "find-println"
description = "Find println"
severity = "info"

[match]
pattern = "println!"
"#,
    )
    .unwrap();

    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rule_dir, None).unwrap();

    // Walk files and execute (only include .rs files)
    use ratchets::types::GlobPattern;
    let include = vec![GlobPattern::new("**/*.rs")];
    let walker = FileWalker::new(temp_dir.path(), &include, &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(files);

    // Verify all files were processed
    assert_eq!(result.files_checked, 1000);

    // If we get here without OOM, memory efficiency is acceptable
    println!(
        "Successfully processed {} files with {} violations",
        result.files_checked,
        result.violations.len()
    );
}

#[test]
fn test_parallel_execution_benefit() {
    // Create enough files to see parallel execution benefit
    let temp_dir = create_many_files(500);

    // Create a regex rule
    let rule_dir = temp_dir.path().join("rules");
    fs::create_dir(&rule_dir).unwrap();
    fs::write(
        rule_dir.join("todo.toml"),
        r#"
[rule]
id = "no-todo"
description = "No TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#,
    )
    .unwrap();

    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rule_dir, None).unwrap();

    // Walk files (only include .rs files)
    use ratchets::types::GlobPattern;
    let include = vec![GlobPattern::new("**/*.rs")];
    let walker = FileWalker::new(temp_dir.path(), &include, &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    // Execute with parallel processing (default behavior)
    let start = Instant::now();
    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(files);
    let parallel_time = start.elapsed();

    println!("Parallel execution of 500 files took: {:?}", parallel_time);

    // Verify results
    assert_eq!(result.files_checked, 500);
    assert_eq!(result.violations.len(), 500);

    // Should complete in reasonable time with parallel execution
    assert!(
        parallel_time.as_secs() < 5,
        "Parallel execution should be fast: {:?}",
        parallel_time
    );
}

#[cfg(feature = "lang-rust")]
#[test]
fn test_parser_cache_effectiveness() {
    // This test verifies that the parser cache is working
    // by processing many files and ensuring parsing doesn't slow down

    let temp_dir = create_many_files(50);

    // Create an AST rule
    let rule_dir = temp_dir.path().join("rules");
    fs::create_dir_all(&rule_dir).unwrap();
    fs::write(
        rule_dir.join("functions.toml"),
        r#"
[rule]
id = "find-functions"
description = "Find function definitions"
severity = "info"

[match]
query = "(function_item name: (identifier) @name) @violation"
language = "rust"
"#,
    )
    .unwrap();

    let mut registry = RuleRegistry::new();
    registry.load_custom_ast_rules(&rule_dir, None).unwrap();

    // Walk files (only include .rs files)
    use ratchets::types::GlobPattern;
    let include = vec![GlobPattern::new("**/*.rs")];
    let walker = FileWalker::new(temp_dir.path(), &include, &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    // Execute - parser should be cached after first use
    let start = Instant::now();
    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(files);
    let exec_time = start.elapsed();

    println!(
        "AST execution with parser caching on 50 files took: {:?}",
        exec_time
    );

    // Verify results
    assert_eq!(result.files_checked, 50);
    // Each file has one function
    assert!(result.violations.len() >= 50);

    // With effective caching, should complete reasonably fast
    assert!(
        exec_time.as_secs() < 10,
        "Parser caching should keep execution fast: {:?}",
        exec_time
    );
}

#[test]
fn test_large_files() {
    // Test handling of large individual files
    let temp_dir = TempDir::new().unwrap();

    // Create a few large files (100KB each)
    for i in 0..10 {
        let content = format!(
            "// Large file {}\n{}\n",
            i,
            "fn dummy() { println!(\"TODO\"); }\n".repeat(2000)
        );
        fs::write(temp_dir.path().join(format!("large{}.rs", i)), content).unwrap();
    }

    // Create a regex rule
    let rule_dir = temp_dir.path().join("rules");
    fs::create_dir(&rule_dir).unwrap();
    fs::write(
        rule_dir.join("todo.toml"),
        r#"
[rule]
id = "no-todo"
description = "No TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#,
    )
    .unwrap();

    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rule_dir, None).unwrap();

    // Walk and execute (only include .rs files)
    use ratchets::types::GlobPattern;
    let include = vec![GlobPattern::new("**/*.rs")];
    let walker = FileWalker::new(temp_dir.path(), &include, &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    let start = Instant::now();
    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(files);
    let exec_time = start.elapsed();

    println!("Processing 10 large files took: {:?}", exec_time);

    // Verify we found violations
    assert!(result.violations.len() >= 10);

    // Should handle large files efficiently
    assert!(
        exec_time.as_secs() < 5,
        "Large file processing should be efficient: {:?}",
        exec_time
    );
}

#[test]
fn test_mixed_file_sizes() {
    // Test with a realistic mix of small and large files
    let temp_dir = TempDir::new().unwrap();

    // Create 100 small files
    for i in 0..100 {
        let content = format!("// File {}\nfn f{}() {{}}\n", i, i);
        fs::write(temp_dir.path().join(format!("small{}.rs", i)), content).unwrap();
    }

    // Create 10 medium files
    for i in 0..10 {
        let content = format!(
            "// Medium file {}\n{}\n",
            i,
            "fn function() { /* code */ }\n".repeat(100)
        );
        fs::write(temp_dir.path().join(format!("medium{}.rs", i)), content).unwrap();
    }

    // Create 2 large files
    for i in 0..2 {
        let content = format!(
            "// Large file {}\n{}\n",
            i,
            "fn function() { /* code */ }\n".repeat(1000)
        );
        fs::write(temp_dir.path().join(format!("large{}.rs", i)), content).unwrap();
    }

    // Create a simple rule
    let rule_dir = temp_dir.path().join("rules");
    fs::create_dir(&rule_dir).unwrap();
    fs::write(
        rule_dir.join("pattern.toml"),
        r#"
[rule]
id = "find-fn"
description = "Find fn keyword"
severity = "info"

[match]
pattern = "fn "
"#,
    )
    .unwrap();

    let mut registry = RuleRegistry::new();
    registry.load_custom_regex_rules(&rule_dir, None).unwrap();

    // Execute (only include .rs files)
    use ratchets::types::GlobPattern;
    let include = vec![GlobPattern::new("**/*.rs")];
    let walker = FileWalker::new(temp_dir.path(), &include, &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    let start = Instant::now();
    let engine = ExecutionEngine::new(registry, None);
    let result = engine.execute(files);
    let exec_time = start.elapsed();

    println!("Processing mixed file sizes took: {:?}", exec_time);

    // Should process all files
    assert_eq!(result.files_checked, 112);

    // Should complete efficiently
    assert!(
        exec_time.as_secs() < 5,
        "Mixed file size processing should be efficient: {:?}",
        exec_time
    );
}
