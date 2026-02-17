//! File discovery and traversal with gitignore support
//!
//! This module provides gitignore-aware file walking with glob-based filtering
//! and automatic language detection using the ignore crate's TypesBuilder.

use crate::types::{GlobPattern, Language};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use ignore::types::{Types, TypesBuilder};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

/// Detects programming languages for files using the ignore crate's TypesBuilder.
///
/// This uses the well-maintained file type definitions from ripgrep, gaining
/// support for additional extensions like `.mts`, `.cts` for TypeScript,
/// `.vue`, `.cjs`, `.mjs` for JavaScript, and `.pyi` for Python.
#[derive(Clone)]
pub struct LanguageDetector {
    /// Map from Language to its Types matcher
    matchers: Arc<HashMap<Language, Types>>,
}

impl LanguageDetector {
    /// Creates a new LanguageDetector with matchers for all supported languages.
    ///
    /// If building a matcher for a language fails, that language is logged and skipped.
    pub fn new() -> Self {
        let mut matchers = HashMap::new();

        for lang in Language::all() {
            let type_name = lang.ignore_type_name();
            let mut builder = TypesBuilder::new();
            builder.add_defaults();
            builder.select(type_name);

            match builder.build() {
                Ok(types) => {
                    matchers.insert(lang, types);
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to build language detector for {}: {}",
                        type_name, e
                    );
                }
            }
        }

        Self {
            matchers: Arc::new(matchers),
        }
    }

    /// Detects the language of a file based on its path.
    ///
    /// Returns the first matching language, or None if no language matches.
    pub fn detect(&self, path: &Path) -> Option<Language> {
        for lang in Language::all() {
            if let Some(types) = self.matchers.get(&lang)
                && types.matched(path, false).is_whitelist()
            {
                return Some(lang);
            }
        }
        None
    }
}

impl Default for LanguageDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LanguageDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanguageDetector")
            .field("languages", &self.matchers.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Reason why a file was skipped
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// File did not match include patterns
    ExcludedByPattern,
    /// File has no recognized language
    NoMatchingLanguage,
    /// File is not a regular file (e.g., directory, symlink)
    NotAFile,
}

/// Result of file walking - either a file to scan or a skipped file
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalkResult {
    /// File to be scanned
    File(FileEntry),
    /// File that was skipped with reason
    Skipped { path: PathBuf, reason: SkipReason },
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
    /// Creates a new FileEntry with language detection using the provided detector.
    pub fn new(path: PathBuf, detector: &LanguageDetector) -> Self {
        let language = detector.detect(&path);
        Self { path, language }
    }

    /// Creates a new FileEntry with a pre-determined language.
    ///
    /// This is useful when the language has already been detected.
    pub fn with_language(path: PathBuf, language: Option<Language>) -> Self {
        Self { path, language }
    }
}

/// Iterator over discovered files
pub struct FileWalker {
    walker: ignore::Walk,
    include_set: Option<globset::GlobSet>,
    exclude_set: Option<globset::GlobSet>,
    verbose: bool,
    language_detector: LanguageDetector,
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
        Self::with_verbose(root, include, exclude, false)
    }

    /// Creates a new FileWalker with verbose mode option
    ///
    /// # Arguments
    /// * `root` - Root directory to walk
    /// * `include` - Include patterns (empty means include all)
    /// * `exclude` - Exclude patterns (applied after include)
    /// * `verbose` - If true, report skipped files
    ///
    /// # Returns
    /// A FileWalker that will iterate over matching files
    pub fn with_verbose(
        root: &Path,
        include: &[GlobPattern],
        exclude: &[GlobPattern],
        verbose: bool,
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

        let language_detector = LanguageDetector::new();

        Ok(Self {
            walker,
            include_set,
            exclude_set,
            verbose,
            language_detector,
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
        self.walk_with_skip_info()
            .filter_map(|result| match result {
                Ok(WalkResult::File(file)) => Some(Ok(file)),
                Ok(WalkResult::Skipped { .. }) => None,
                Err(e) => Some(Err(e)),
            })
    }

    /// Walks the directory tree and returns an iterator with skip information
    pub fn walk_with_skip_info(self) -> impl Iterator<Item = Result<WalkResult, FileWalkerError>> {
        let include_set = self.include_set;
        let exclude_set = self.exclude_set;
        let verbose = self.verbose;
        let language_detector = self.language_detector;

        self.walker.filter_map(move |result| {
            match result {
                Ok(entry) => {
                    // Only process files (not directories)
                    if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                        if verbose {
                            return Some(Ok(WalkResult::Skipped {
                                path: entry.path().to_path_buf(),
                                reason: SkipReason::NotAFile,
                            }));
                        } else {
                            return None;
                        }
                    }

                    let path = entry.path();

                    // Apply include/exclude filters
                    // If include patterns are specified, path must match at least one
                    if let Some(ref include_set) = include_set
                        && !include_set.is_match(path)
                    {
                        if verbose {
                            return Some(Ok(WalkResult::Skipped {
                                path: path.to_path_buf(),
                                reason: SkipReason::ExcludedByPattern,
                            }));
                        } else {
                            return None;
                        }
                    }

                    // If path matches any exclude pattern, reject it
                    if let Some(ref exclude_set) = exclude_set
                        && exclude_set.is_match(path)
                    {
                        if verbose {
                            return Some(Ok(WalkResult::Skipped {
                                path: path.to_path_buf(),
                                reason: SkipReason::ExcludedByPattern,
                            }));
                        } else {
                            return None;
                        }
                    }

                    // Create FileEntry and check if it has a recognized language
                    let file_entry = FileEntry::new(path.to_path_buf(), &language_detector);

                    // Filter out non-program files (no recognized language)
                    if file_entry.language.is_none() {
                        if verbose {
                            return Some(Ok(WalkResult::Skipped {
                                path: path.to_path_buf(),
                                reason: SkipReason::NoMatchingLanguage,
                            }));
                        } else {
                            return None;
                        }
                    }

                    Some(Ok(WalkResult::File(file_entry)))
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
    fn test_language_detector_new() {
        let detector = LanguageDetector::new();
        // Should have matchers for all languages
        assert!(detector.matchers.len() >= 5);
    }

    #[test]
    fn test_language_detector_detect_rust() {
        let detector = LanguageDetector::new();
        assert_eq!(detector.detect(Path::new("test.rs")), Some(Language::Rust));
    }

    #[test]
    fn test_language_detector_detect_typescript() {
        let detector = LanguageDetector::new();
        assert_eq!(
            detector.detect(Path::new("test.ts")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            detector.detect(Path::new("test.tsx")),
            Some(Language::TypeScript)
        );
        // Additional extensions supported by ignore crate
        assert_eq!(
            detector.detect(Path::new("test.mts")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            detector.detect(Path::new("test.cts")),
            Some(Language::TypeScript)
        );
    }

    #[test]
    fn test_language_detector_detect_javascript() {
        let detector = LanguageDetector::new();
        assert_eq!(
            detector.detect(Path::new("test.js")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            detector.detect(Path::new("test.jsx")),
            Some(Language::JavaScript)
        );
        // Additional extensions supported by ignore crate
        assert_eq!(
            detector.detect(Path::new("test.mjs")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            detector.detect(Path::new("test.cjs")),
            Some(Language::JavaScript)
        );
    }

    #[test]
    fn test_language_detector_detect_python() {
        let detector = LanguageDetector::new();
        assert_eq!(
            detector.detect(Path::new("test.py")),
            Some(Language::Python)
        );
        // Additional extensions supported by ignore crate
        assert_eq!(
            detector.detect(Path::new("test.pyi")),
            Some(Language::Python)
        );
    }

    #[test]
    fn test_language_detector_detect_go() {
        let detector = LanguageDetector::new();
        assert_eq!(detector.detect(Path::new("test.go")), Some(Language::Go));
    }

    #[test]
    fn test_language_detector_detect_unknown() {
        let detector = LanguageDetector::new();
        assert_eq!(detector.detect(Path::new("test.txt")), None);
    }

    #[test]
    fn test_language_detector_detect_no_extension() {
        let detector = LanguageDetector::new();
        assert_eq!(detector.detect(Path::new("Makefile")), None);
    }

    #[test]
    fn test_file_entry_new_with_detector() {
        let detector = LanguageDetector::new();
        let path = PathBuf::from("test.rs");
        let entry = FileEntry::new(path.clone(), &detector);
        assert_eq!(entry.language, Some(Language::Rust));
        assert_eq!(entry.path, path);
    }

    #[test]
    fn test_file_entry_with_language() {
        let path = PathBuf::from("test.rs");
        let entry = FileEntry::with_language(path.clone(), Some(Language::Rust));
        assert_eq!(entry.language, Some(Language::Rust));
        assert_eq!(entry.path, path);
    }

    #[test]
    fn test_file_entry_equality() {
        let detector = LanguageDetector::new();
        let entry1 = FileEntry::new(PathBuf::from("test.rs"), &detector);
        let entry2 = FileEntry::new(PathBuf::from("test.rs"), &detector);
        assert_eq!(entry1, entry2);

        let entry3 = FileEntry::new(PathBuf::from("other.rs"), &detector);
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

        // Should find only the .rs file - .txt is filtered out (no recognized language)
        assert_eq!(files.len(), 1);

        // Assert all results are Ok first
        assert!(
            files.iter().all(|r| r.is_ok()),
            "All walk results should be Ok"
        );
        // Then check file properties using filter_map to extract Ok values
        for file in files.iter().filter_map(|r| r.as_ref().ok()) {
            assert!(file.language.is_some(), "All files should have a language");
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
