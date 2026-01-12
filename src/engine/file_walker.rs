//! File discovery and traversal with gitignore support
//!
//! This module provides gitignore-aware file walking with glob-based filtering
//! and automatic language detection from file extensions.

use crate::types::{GlobPattern, Language};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during file walking
#[derive(Debug, Error)]
pub enum FileWalkerError {
    #[error("Invalid glob pattern '{pattern}': {source}")]
    InvalidGlob {
        pattern: String,
        source: globset::Error,
    },

    #[error("Walk error: {0}")]
    Walk(#[from] ignore::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A discovered file with its detected language
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Absolute path to the file
    pub path: PathBuf,
    /// Detected language, None if extension is not recognized
    pub language: Option<Language>,
}

impl FileEntry {
    /// Creates a new FileEntry with language detection
    pub fn new(path: PathBuf) -> Self {
        let language = Self::detect_language(&path);
        Self { path, language }
    }

    /// Detects language from file extension
    fn detect_language(path: &Path) -> Option<Language> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext {
                "rs" => Some(Language::Rust),
                "ts" | "tsx" => Some(Language::TypeScript),
                "js" | "jsx" => Some(Language::JavaScript),
                "py" => Some(Language::Python),
                "go" => Some(Language::Go),
                _ => None,
            })
    }
}

/// Iterator over discovered files
pub struct FileWalker {
    walker: ignore::Walk,
    include_set: Option<globset::GlobSet>,
    exclude_set: Option<globset::GlobSet>,
}

impl FileWalker {
    /// Creates a new FileWalker
    ///
    /// # Arguments
    /// * `root` - Root directory to walk
    /// * `include` - Include patterns (empty means include all)
    /// * `exclude` - Exclude patterns (applied after include)
    ///
    /// # Returns
    /// A FileWalker that will iterate over matching files
    pub fn new(
        root: &Path,
        include: &[GlobPattern],
        exclude: &[GlobPattern],
    ) -> Result<Self, FileWalkerError> {
        let walker = WalkBuilder::new(root)
            .hidden(false) // Don't skip hidden files by default
            .git_ignore(true) // Respect .gitignore
            .build();

        let include_set = if include.is_empty() {
            None
        } else {
            Some(Self::build_globset(include)?)
        };

        // Always exclude .git directory, merging with user-provided excludes
        let mut exclude_patterns = Vec::from(exclude);
        exclude_patterns.push(GlobPattern::new("**/.git/**"));

        let exclude_set = Some(Self::build_globset(&exclude_patterns)?);

        Ok(Self {
            walker,
            include_set,
            exclude_set,
        })
    }

    /// Builds a GlobSet from patterns
    fn build_globset(patterns: &[GlobPattern]) -> Result<globset::GlobSet, FileWalkerError> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            let glob = Glob::new(pattern.as_str()).map_err(|e| FileWalkerError::InvalidGlob {
                pattern: pattern.as_str().to_string(),
                source: e,
            })?;
            builder.add(glob);
        }
        builder.build().map_err(|e| FileWalkerError::InvalidGlob {
            pattern: "<globset>".to_string(),
            source: e,
        })
    }

    /// Walks the directory tree and returns an iterator over matching files
    pub fn walk(self) -> impl Iterator<Item = Result<FileEntry, FileWalkerError>> {
        let include_set = self.include_set;
        let exclude_set = self.exclude_set;

        self.walker.filter_map(move |result| {
            match result {
                Ok(entry) => {
                    // Only process files (not directories)
                    if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                        return None;
                    }

                    let path = entry.path();

                    // Apply include/exclude filters
                    // If include patterns are specified, path must match at least one
                    if let Some(ref include_set) = include_set
                        && !include_set.is_match(path)
                    {
                        return None;
                    }

                    // If path matches any exclude pattern, reject it
                    if let Some(ref exclude_set) = exclude_set
                        && exclude_set.is_match(path)
                    {
                        return None;
                    }

                    Some(Ok(FileEntry::new(path.to_path_buf())))
                }
                Err(e) => Some(Err(FileWalkerError::Walk(e))),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_language_rust() {
        let path = PathBuf::from("test.rs");
        let entry = FileEntry::new(path.clone());
        assert_eq!(entry.language, Some(Language::Rust));
        assert_eq!(entry.path, path);
    }

    #[test]
    fn test_detect_language_typescript() {
        let ts_path = PathBuf::from("test.ts");
        let ts_entry = FileEntry::new(ts_path);
        assert_eq!(ts_entry.language, Some(Language::TypeScript));

        let tsx_path = PathBuf::from("test.tsx");
        let tsx_entry = FileEntry::new(tsx_path);
        assert_eq!(tsx_entry.language, Some(Language::TypeScript));
    }

    #[test]
    fn test_detect_language_javascript() {
        let js_path = PathBuf::from("test.js");
        let js_entry = FileEntry::new(js_path);
        assert_eq!(js_entry.language, Some(Language::JavaScript));

        let jsx_path = PathBuf::from("test.jsx");
        let jsx_entry = FileEntry::new(jsx_path);
        assert_eq!(jsx_entry.language, Some(Language::JavaScript));
    }

    #[test]
    fn test_detect_language_python() {
        let path = PathBuf::from("test.py");
        let entry = FileEntry::new(path);
        assert_eq!(entry.language, Some(Language::Python));
    }

    #[test]
    fn test_detect_language_go() {
        let path = PathBuf::from("test.go");
        let entry = FileEntry::new(path);
        assert_eq!(entry.language, Some(Language::Go));
    }

    #[test]
    fn test_detect_language_unknown() {
        let path = PathBuf::from("test.txt");
        let entry = FileEntry::new(path);
        assert_eq!(entry.language, None);
    }

    #[test]
    fn test_detect_language_no_extension() {
        let path = PathBuf::from("Makefile");
        let entry = FileEntry::new(path);
        assert_eq!(entry.language, None);
    }

    #[test]
    fn test_file_entry_equality() {
        let entry1 = FileEntry::new(PathBuf::from("test.rs"));
        let entry2 = FileEntry::new(PathBuf::from("test.rs"));
        assert_eq!(entry1, entry2);

        let entry3 = FileEntry::new(PathBuf::from("other.rs"));
        assert_ne!(entry1, entry3);
    }

    #[test]
    fn test_build_globset_valid() {
        let patterns = vec![GlobPattern::new("*.rs"), GlobPattern::new("src/**/*.rs")];
        let result = FileWalker::build_globset(&patterns);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_globset_invalid() {
        let patterns = vec![GlobPattern::new("[invalid")];
        let result = FileWalker::build_globset(&patterns);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_walker_new() {
        let root = Path::new(".");
        let include = vec![GlobPattern::new("*.rs")];
        let exclude = vec![GlobPattern::new("target/**")];

        let walker = FileWalker::new(root, &include, &exclude);
        assert!(walker.is_ok());
    }

    #[test]
    fn test_file_walker_empty_patterns() {
        let root = Path::new(".");
        let walker = FileWalker::new(root, &[], &[]);
        assert!(walker.is_ok());
    }

    #[test]
    fn test_walk_basic() {
        // Create a temporary directory with some test files
        let temp_dir = std::env::temp_dir().join("ratchet_test_walk_basic");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

        fs::write(temp_dir.join("test.rs"), "fn main() {}").expect("Failed to write test.rs");
        fs::write(temp_dir.join("test.txt"), "hello").expect("Failed to write test.txt");

        let walker = FileWalker::new(&temp_dir, &[], &[]).expect("Failed to create walker");
        let files: Vec<_> = walker.walk().collect();

        // Should find at least the two files we created
        assert!(files.len() >= 2);

        // Check that all results are Ok
        for result in &files {
            assert!(result.is_ok());
        }

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_walk_with_include_filter() {
        let temp_dir = std::env::temp_dir().join("ratchet_test_walk_include");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

        fs::write(temp_dir.join("test.rs"), "fn main() {}").expect("Failed to write test.rs");
        fs::write(temp_dir.join("test.txt"), "hello").expect("Failed to write test.txt");

        let include = vec![GlobPattern::new("*.rs")];
        let walker = FileWalker::new(&temp_dir, &include, &[]).expect("Failed to create walker");
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        // Should only find .rs files
        assert!(
            files
                .iter()
                .all(|f| f.path.extension().is_some_and(|ext| ext == "rs"))
        );
        assert!(!files.is_empty());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
