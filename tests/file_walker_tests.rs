//! Integration tests for file_walker module
//!
//! These tests use fixture directories in tests/fixtures/file_walker/
//! to verify file walking, gitignore support, pattern matching, and more.

use ratchets::engine::file_walker::{FileEntry, FileWalker};
use ratchets::types::{GlobPattern, Language};
use std::collections::HashSet;
use std::path::PathBuf;

/// Helper to get absolute path to a fixture directory
fn fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path.push("file_walker");
    path.push(name);
    path
}

/// Helper to collect files from a walker
fn collect_files(walker: FileWalker) -> Vec<FileEntry> {
    walker.walk().filter_map(Result::ok).collect::<Vec<_>>()
}

/// Helper to extract just filenames from paths for easier testing
fn extract_filenames(files: &[FileEntry]) -> HashSet<String> {
    files
        .iter()
        .filter_map(|f| {
            f.path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .collect()
}

#[test]
fn test_walk_basic_directory() {
    let fixture = fixture_path("basic");
    let walker = FileWalker::new(&fixture, &[], &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    // Should find at least 3 .rs files
    assert!(!files.is_empty(), "Should find files in basic fixture");

    let filenames = extract_filenames(&files);
    assert!(filenames.contains("main.rs"), "Should find main.rs");
    assert!(filenames.contains("lib.rs"), "Should find lib.rs");
    assert!(filenames.contains("test.rs"), "Should find test.rs");

    // All .rs files should be detected as Rust
    for file in &files {
        if file.path.extension().is_some_and(|ext| ext == "rs") {
            assert_eq!(
                file.language,
                Some(Language::Rust),
                "Rust files should be detected"
            );
        }
    }
}

#[test]
fn test_walk_with_gitignore() {
    let fixture = fixture_path("with_gitignore");
    let walker = FileWalker::new(&fixture, &[], &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    let filenames = extract_filenames(&files);

    // Should find main.rs
    assert!(
        filenames.contains("main.rs"),
        "Should find main.rs in with_gitignore fixture"
    );

    // Should NOT find files in target/ directory (ignored by .gitignore)
    assert!(
        !filenames.contains("app"),
        "Should not find files in target/ directory (gitignored)"
    );

    // Should NOT find .log files (ignored by .gitignore)
    assert!(
        !filenames.contains("app.log"),
        "Should not find .log files (gitignored)"
    );

    // Should find .gitignore itself
    assert!(
        filenames.contains(".gitignore"),
        "Should find .gitignore file"
    );
}

#[test]
fn test_walk_nested_directories() {
    let fixture = fixture_path("nested");
    let walker = FileWalker::new(&fixture, &[], &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    let filenames = extract_filenames(&files);

    // Should find files at all nesting levels
    assert!(filenames.contains("top.ts"), "Should find top.ts");
    assert!(filenames.contains("mid.py"), "Should find mid.py");
    assert!(filenames.contains("deep.rs"), "Should find deep.rs");

    // Verify language detection works at all levels
    let deep_rs = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "deep.rs"));
    assert!(deep_rs.is_some(), "Should find deep.rs");
    assert_eq!(
        deep_rs.unwrap().language,
        Some(Language::Rust),
        "deep.rs should be detected as Rust"
    );

    let mid_py = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "mid.py"));
    assert!(mid_py.is_some(), "Should find mid.py");
    assert_eq!(
        mid_py.unwrap().language,
        Some(Language::Python),
        "mid.py should be detected as Python"
    );

    let top_ts = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "top.ts"));
    assert!(top_ts.is_some(), "Should find top.ts");
    assert_eq!(
        top_ts.unwrap().language,
        Some(Language::TypeScript),
        "top.ts should be detected as TypeScript"
    );
}

#[test]
fn test_walk_multi_language() {
    let fixture = fixture_path("multi_lang");
    let walker = FileWalker::new(&fixture, &[], &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    let filenames = extract_filenames(&files);

    // Should find all language files
    assert!(filenames.contains("main.rs"), "Should find Rust file");
    assert!(filenames.contains("app.py"), "Should find Python file");
    assert!(
        filenames.contains("index.ts"),
        "Should find TypeScript file"
    );
    assert!(filenames.contains("util.js"), "Should find JavaScript file");

    // Verify each language is detected correctly
    let rust_file = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "main.rs"));
    assert_eq!(
        rust_file.unwrap().language,
        Some(Language::Rust),
        "main.rs should be Rust"
    );

    let python_file = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "app.py"));
    assert_eq!(
        python_file.unwrap().language,
        Some(Language::Python),
        "app.py should be Python"
    );

    let typescript_file = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "index.ts"));
    assert_eq!(
        typescript_file.unwrap().language,
        Some(Language::TypeScript),
        "index.ts should be TypeScript"
    );

    let javascript_file = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "util.js"));
    assert_eq!(
        javascript_file.unwrap().language,
        Some(Language::JavaScript),
        "util.js should be JavaScript"
    );
}

#[test]
fn test_walk_include_pattern() {
    let fixture = fixture_path("nested");
    let include = vec![GlobPattern::new("**/*.rs")];
    let walker = FileWalker::new(&fixture, &include, &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    // Should only find .rs files
    assert!(!files.is_empty(), "Should find at least one .rs file");

    for file in &files {
        assert_eq!(
            file.path.extension().and_then(|s| s.to_str()),
            Some("rs"),
            "All files should have .rs extension with include pattern"
        );
        assert_eq!(
            file.language,
            Some(Language::Rust),
            "All matched files should be Rust"
        );
    }

    let filenames = extract_filenames(&files);
    assert!(filenames.contains("deep.rs"), "Should find deep.rs");
    assert!(!filenames.contains("mid.py"), "Should not find .py files");
    assert!(!filenames.contains("top.ts"), "Should not find .ts files");
}

#[test]
fn test_walk_exclude_pattern() {
    let fixture = fixture_path("basic");
    // Use a pattern that excludes files named test.rs specifically
    let exclude = vec![GlobPattern::new("**/test.rs")];
    let walker = FileWalker::new(&fixture, &[], &exclude).expect("Failed to create walker");
    let files = collect_files(walker);

    let filenames = extract_filenames(&files);

    // Should find src files but not test.rs
    assert!(filenames.contains("main.rs"), "Should find main.rs");
    assert!(filenames.contains("lib.rs"), "Should find lib.rs");
    assert!(
        !filenames.contains("test.rs"),
        "Should not find test.rs (excluded by pattern)"
    );
}

#[test]
fn test_walk_combined_patterns() {
    let fixture = fixture_path("nested");
    let include = vec![GlobPattern::new("**/*.{rs,py}")];
    let exclude = vec![GlobPattern::new("**/a/b/**")];
    let walker = FileWalker::new(&fixture, &include, &exclude).expect("Failed to create walker");
    let files = collect_files(walker);

    let filenames = extract_filenames(&files);

    // Should find Python file at mid level
    assert!(filenames.contains("mid.py"), "Should find mid.py");

    // Should NOT find deep.rs (excluded by path) or top.ts (not in include)
    assert!(
        !filenames.contains("deep.rs"),
        "Should not find deep.rs (in excluded path a/b/)"
    );
    assert!(
        !filenames.contains("top.ts"),
        "Should not find top.ts (not in include pattern)"
    );
}

#[test]
fn test_walk_empty_directory() {
    use std::fs;

    // Create a temporary empty directory
    let temp_dir = std::env::temp_dir().join("ratchet_test_walk_empty");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let walker = FileWalker::new(&temp_dir, &[], &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    // Should successfully handle empty directory
    assert!(files.is_empty(), "Empty directory should yield no files");

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_walk_nonexistent_path() {
    let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist");
    let walker = FileWalker::new(&nonexistent, &[], &[]);

    // Walker creation should succeed, but walking will yield no results
    // (the ignore crate handles nonexistent paths gracefully)
    assert!(walker.is_ok(), "Walker should be created successfully");

    let files = collect_files(walker.expect("Walker should be ok"));
    assert!(
        files.is_empty(),
        "Walking nonexistent path should yield no files"
    );
}

#[test]
fn test_symlink_handling() {
    let fixture = fixture_path("symlinks");

    // Check if symlink was created (may not be supported on all systems)
    let symlink_path = fixture.join("link_to_file.rs");
    if !symlink_path.exists() {
        // Skip test if symlinks aren't supported
        return;
    }

    let walker = FileWalker::new(&fixture, &[], &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    let filenames = extract_filenames(&files);

    // Should find the actual file
    assert!(
        filenames.contains("actual_file.rs"),
        "Should find actual_file.rs"
    );

    // The behavior with symlinks depends on the ignore crate's default settings
    // By default, it follows symlinks, so we should find the symlink too
    // (or at least the actual file through it)
    assert!(!files.is_empty(), "Should find at least the actual file");
}

#[test]
fn test_walk_respects_file_types_only() {
    let fixture = fixture_path("basic");
    let walker = FileWalker::new(&fixture, &[], &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    // All entries should be files, not directories
    for file in &files {
        assert!(
            file.path.is_file(),
            "All entries should be files, not directories: {:?}",
            file.path
        );
    }
}

#[test]
fn test_invalid_glob_pattern() {
    let fixture = fixture_path("basic");

    // Test with invalid glob pattern
    let invalid_include = vec![GlobPattern::new("[invalid")];
    let result = FileWalker::new(&fixture, &invalid_include, &[]);

    assert!(result.is_err(), "Invalid glob pattern should return error");
}

#[test]
fn test_multiple_include_patterns() {
    let fixture = fixture_path("nested");
    let include = vec![GlobPattern::new("**/*.rs"), GlobPattern::new("**/*.py")];
    let walker = FileWalker::new(&fixture, &include, &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    let filenames = extract_filenames(&files);

    // Should find both Rust and Python files
    assert!(filenames.contains("deep.rs"), "Should find .rs file");
    assert!(filenames.contains("mid.py"), "Should find .py file");

    // Should NOT find TypeScript files
    assert!(!filenames.contains("top.ts"), "Should not find .ts file");
}

#[test]
fn test_multiple_exclude_patterns() {
    let fixture = fixture_path("multi_lang");
    let exclude = vec![
        GlobPattern::new("**/python/**"),
        GlobPattern::new("**/javascript/**"),
    ];
    let walker = FileWalker::new(&fixture, &[], &exclude).expect("Failed to create walker");
    let files = collect_files(walker);

    let filenames = extract_filenames(&files);

    // Should find Rust and TypeScript files
    assert!(filenames.contains("main.rs"), "Should find Rust file");
    assert!(
        filenames.contains("index.ts"),
        "Should find TypeScript file"
    );

    // Should NOT find Python or JavaScript files
    assert!(!filenames.contains("app.py"), "Should not find Python file");
    assert!(
        !filenames.contains("util.js"),
        "Should not find JavaScript file"
    );
}

#[test]
fn test_language_detection_edge_cases() {
    use std::fs;

    // Create temporary directory with edge case files
    let temp_dir = std::env::temp_dir().join("ratchet_test_walk_edge_cases");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    // File with no extension
    fs::write(temp_dir.join("Makefile"), "test").expect("Failed to write Makefile");

    // File with unknown extension
    fs::write(temp_dir.join("file.unknown"), "test").expect("Failed to write file.unknown");

    // File with double extension
    fs::write(temp_dir.join("file.test.rs"), "test").expect("Failed to write file.test.rs");

    let walker = FileWalker::new(&temp_dir, &[], &[]).expect("Failed to create walker");
    let files = collect_files(walker);

    // Find each file and check language detection
    let makefile = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "Makefile"));
    assert!(makefile.is_some(), "Should find Makefile");
    assert_eq!(
        makefile.unwrap().language,
        None,
        "Makefile should have no language"
    );

    let unknown = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "file.unknown"));
    assert!(unknown.is_some(), "Should find file.unknown");
    assert_eq!(
        unknown.unwrap().language,
        None,
        "Unknown extension should have no language"
    );

    let double_ext = files
        .iter()
        .find(|f| f.path.file_name().is_some_and(|n| n == "file.test.rs"));
    assert!(double_ext.is_some(), "Should find file.test.rs");
    assert_eq!(
        double_ext.unwrap().language,
        Some(Language::Rust),
        "file.test.rs should be detected as Rust (last extension)"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
