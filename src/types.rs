#![forbid(unsafe_code)]

//! Core domain types for Ratchet
//!
//! This module defines the fundamental types used throughout the Ratchet system.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Programming languages supported by Ratchet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
}

/// Violation severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A validated rule identifier
///
/// Rule IDs must be non-empty and contain only alphanumeric characters, hyphens, and underscores.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RuleId(String);

impl RuleId {
    /// Creates a new RuleId, validating the input
    ///
    /// Returns None if the input is empty or contains invalid characters
    pub fn new(id: impl Into<String>) -> Option<Self> {
        let id = id.into();
        if id.is_empty() {
            return None;
        }
        if !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }
        Some(RuleId(id))
    }

    /// Returns the rule ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for RuleId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        RuleId::new(value).ok_or_else(|| "Invalid rule ID".to_string())
    }
}

impl From<RuleId> for String {
    fn from(rule_id: RuleId) -> Self {
        rule_id.0
    }
}

/// A normalized file system path for region identification
///
/// Paths are normalized to use forward slashes, have no trailing slash,
/// and the root is represented as ".".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RegionPath(String);

impl RegionPath {
    /// Creates a new RegionPath with normalization
    pub fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        let normalized = Self::normalize(path);
        RegionPath(normalized)
    }

    /// Normalizes a path:
    /// - Convert backslashes to forward slashes
    /// - Remove trailing slashes
    /// - Empty path or "." becomes "."
    /// - Remove "./" prefix
    fn normalize(mut path: String) -> String {
        // Convert backslashes to forward slashes
        path = path.replace('\\', "/");

        // Remove trailing slashes
        while path.ends_with('/') && path.len() > 1 {
            path.pop();
        }

        // Handle empty or root paths
        if path.is_empty() || path == "/" {
            return ".".to_string();
        }

        // Remove leading "./"
        if path.starts_with("./") {
            path = path[2..].to_string();
        }

        // If we removed everything, return "."
        if path.is_empty() {
            return ".".to_string();
        }

        path
    }

    /// Returns the region path as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RegionPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for RegionPath {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(RegionPath::new(value))
    }
}

impl From<RegionPath> for String {
    fn from(region_path: RegionPath) -> Self {
        region_path.0
    }
}

/// A glob pattern for file matching
///
/// This is a simple wrapper around a string that will be used with the `globset` crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GlobPattern(String);

impl GlobPattern {
    /// Creates a new GlobPattern
    pub fn new(pattern: impl Into<String>) -> Self {
        GlobPattern(pattern.into())
    }

    /// Returns the pattern as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GlobPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for GlobPattern {
    fn from(pattern: String) -> Self {
        GlobPattern(pattern)
    }
}

impl From<&str> for GlobPattern {
    fn from(pattern: &str) -> Self {
        GlobPattern(pattern.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Language enum tests
    #[test]
    fn test_language_serde_serialization() {
        assert_eq!(serde_json::to_string(&Language::Rust).unwrap(), "\"rust\"");
        assert_eq!(
            serde_json::to_string(&Language::TypeScript).unwrap(),
            "\"typescript\""
        );
        assert_eq!(
            serde_json::to_string(&Language::JavaScript).unwrap(),
            "\"javascript\""
        );
        assert_eq!(
            serde_json::to_string(&Language::Python).unwrap(),
            "\"python\""
        );
        assert_eq!(serde_json::to_string(&Language::Go).unwrap(), "\"go\"");
    }

    #[test]
    fn test_language_serde_deserialization() {
        assert_eq!(
            serde_json::from_str::<Language>("\"rust\"").unwrap(),
            Language::Rust
        );
        assert_eq!(
            serde_json::from_str::<Language>("\"typescript\"").unwrap(),
            Language::TypeScript
        );
        assert_eq!(
            serde_json::from_str::<Language>("\"javascript\"").unwrap(),
            Language::JavaScript
        );
        assert_eq!(
            serde_json::from_str::<Language>("\"python\"").unwrap(),
            Language::Python
        );
        assert_eq!(
            serde_json::from_str::<Language>("\"go\"").unwrap(),
            Language::Go
        );
    }

    #[test]
    fn test_language_all_variants_exist() {
        // Ensure all 5 variants can be constructed
        let _rust = Language::Rust;
        let _typescript = Language::TypeScript;
        let _javascript = Language::JavaScript;
        let _python = Language::Python;
        let _go = Language::Go;
    }

    // Severity enum tests
    #[test]
    fn test_severity_serde_serialization() {
        assert_eq!(
            serde_json::to_string(&Severity::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&Severity::Warning).unwrap(),
            "\"warning\""
        );
        assert_eq!(serde_json::to_string(&Severity::Info).unwrap(), "\"info\"");
    }

    #[test]
    fn test_severity_serde_deserialization() {
        assert_eq!(
            serde_json::from_str::<Severity>("\"error\"").unwrap(),
            Severity::Error
        );
        assert_eq!(
            serde_json::from_str::<Severity>("\"warning\"").unwrap(),
            Severity::Warning
        );
        assert_eq!(
            serde_json::from_str::<Severity>("\"info\"").unwrap(),
            Severity::Info
        );
    }

    // RuleId validation tests
    #[test]
    fn test_rule_id_validation_valid() {
        assert!(RuleId::new("valid-rule").is_some());
        assert!(RuleId::new("rule_123").is_some());
        assert!(RuleId::new("no-unwrap").is_some());
        assert!(RuleId::new("a").is_some());
        assert!(RuleId::new("123").is_some());
        assert!(RuleId::new("UPPERCASE").is_some());
        assert!(RuleId::new("Mixed-Case_123").is_some());
    }

    #[test]
    fn test_rule_id_validation_invalid() {
        assert!(RuleId::new("").is_none());
        assert!(RuleId::new("invalid rule").is_none());
        assert!(RuleId::new("invalid@rule").is_none());
        assert!(RuleId::new("invalid.rule").is_none());
        assert!(RuleId::new("invalid/rule").is_none());
        assert!(RuleId::new("invalid\\rule").is_none());
    }

    #[test]
    fn test_rule_id_display() {
        let rule_id = RuleId::new("test-rule").unwrap();
        assert_eq!(rule_id.to_string(), "test-rule");
        assert_eq!(format!("{}", rule_id), "test-rule");
    }

    #[test]
    fn test_rule_id_as_str() {
        let rule_id = RuleId::new("my-rule").unwrap();
        assert_eq!(rule_id.as_str(), "my-rule");
    }

    #[test]
    fn test_rule_id_try_from() {
        let result = RuleId::try_from("valid-id".to_string());
        assert!(result.is_ok());

        let result = RuleId::try_from("invalid id".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_rule_id_into_string() {
        let rule_id = RuleId::new("test-id").unwrap();
        let s: String = rule_id.into();
        assert_eq!(s, "test-id");
    }

    #[test]
    fn test_rule_id_serde_serialization() {
        let rule_id = RuleId::new("my-rule").unwrap();
        let json = serde_json::to_string(&rule_id).unwrap();
        assert_eq!(json, "\"my-rule\"");
    }

    #[test]
    fn test_rule_id_serde_deserialization() {
        let rule_id: RuleId = serde_json::from_str("\"my-rule\"").unwrap();
        assert_eq!(rule_id.as_str(), "my-rule");

        let result = serde_json::from_str::<RuleId>("\"invalid rule\"");
        assert!(result.is_err());
    }

    // RegionPath normalization tests
    #[test]
    fn test_region_path_normalization_empty() {
        assert_eq!(RegionPath::new("").as_str(), ".");
        assert_eq!(RegionPath::new(".").as_str(), ".");
        assert_eq!(RegionPath::new("./").as_str(), ".");
        assert_eq!(RegionPath::new("/").as_str(), ".");
    }

    #[test]
    fn test_region_path_normalization_simple() {
        assert_eq!(RegionPath::new("src").as_str(), "src");
        assert_eq!(RegionPath::new("tests").as_str(), "tests");
    }

    #[test]
    fn test_region_path_normalization_trailing_slash() {
        assert_eq!(RegionPath::new("src/").as_str(), "src");
        assert_eq!(RegionPath::new("src/parser/").as_str(), "src/parser");
        assert_eq!(RegionPath::new("path///").as_str(), "path");
    }

    #[test]
    fn test_region_path_normalization_leading_dot_slash() {
        assert_eq!(RegionPath::new("./src").as_str(), "src");
        assert_eq!(RegionPath::new("./src/parser").as_str(), "src/parser");
    }

    #[test]
    fn test_region_path_normalization_backslashes() {
        assert_eq!(RegionPath::new("src\\parser").as_str(), "src/parser");
        assert_eq!(
            RegionPath::new("src\\parser\\ast").as_str(),
            "src/parser/ast"
        );
        assert_eq!(RegionPath::new("path\\to\\file").as_str(), "path/to/file");
    }

    #[test]
    fn test_region_path_normalization_mixed() {
        assert_eq!(RegionPath::new("./src\\").as_str(), "src");
        assert_eq!(RegionPath::new(".\\src/parser/").as_str(), "src/parser");
    }

    #[test]
    fn test_region_path_display() {
        let path = RegionPath::new("src/parser");
        assert_eq!(path.to_string(), "src/parser");
        assert_eq!(format!("{}", path), "src/parser");
    }

    #[test]
    fn test_region_path_as_str() {
        let path = RegionPath::new("my/path");
        assert_eq!(path.as_str(), "my/path");
    }

    #[test]
    fn test_region_path_try_from() {
        let result = RegionPath::try_from("src/parser".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "src/parser");
    }

    #[test]
    fn test_region_path_into_string() {
        let path = RegionPath::new("test/path");
        let s: String = path.into();
        assert_eq!(s, "test/path");
    }

    #[test]
    fn test_region_path_serde_serialization() {
        let path = RegionPath::new("src/parser");
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"src/parser\"");
    }

    #[test]
    fn test_region_path_serde_deserialization() {
        let path: RegionPath = serde_json::from_str("\"src/parser\"").unwrap();
        assert_eq!(path.as_str(), "src/parser");

        // Test that normalization happens during deserialization
        let path: RegionPath = serde_json::from_str("\"src\\\\parser\"").unwrap();
        assert_eq!(path.as_str(), "src/parser");
    }

    // GlobPattern tests
    #[test]
    fn test_glob_pattern_basic() {
        let pattern = GlobPattern::new("**/*.rs");
        assert_eq!(pattern.as_str(), "**/*.rs");
    }

    #[test]
    fn test_glob_pattern_display() {
        let pattern = GlobPattern::new("*.toml");
        assert_eq!(pattern.to_string(), "*.toml");
    }

    #[test]
    fn test_glob_pattern_from_string() {
        let pattern: GlobPattern = "test/**/*.rs".to_string().into();
        assert_eq!(pattern.as_str(), "test/**/*.rs");
    }

    #[test]
    fn test_glob_pattern_from_str() {
        let pattern: GlobPattern = "src/*.rs".into();
        assert_eq!(pattern.as_str(), "src/*.rs");
    }

    // Type derives tests
    #[test]
    fn test_type_derives_hash() {
        use std::collections::HashSet;

        let mut languages = HashSet::new();
        languages.insert(Language::Rust);
        languages.insert(Language::TypeScript);

        let mut severities = HashSet::new();
        severities.insert(Severity::Error);
        severities.insert(Severity::Warning);

        let mut rule_ids = HashSet::new();
        rule_ids.insert(RuleId::new("rule1").unwrap());
        rule_ids.insert(RuleId::new("rule2").unwrap());

        let mut region_paths = HashSet::new();
        region_paths.insert(RegionPath::new("src"));
        region_paths.insert(RegionPath::new("tests"));

        let mut glob_patterns = HashSet::new();
        glob_patterns.insert(GlobPattern::new("*.rs"));
        glob_patterns.insert(GlobPattern::new("*.toml"));
    }

    #[test]
    fn test_type_derives_clone() {
        let lang = Language::Rust;
        let _lang_clone = lang; // Copy types don't need clone

        let severity = Severity::Error;
        let _severity_clone = severity; // Copy types don't need clone

        let rule_id = RuleId::new("test").unwrap();
        let _rule_id_clone = rule_id.clone();

        let path = RegionPath::new("src");
        let _path_clone = path.clone();

        let pattern = GlobPattern::new("*.rs");
        let _pattern_clone = pattern.clone();
    }

    #[test]
    fn test_type_derives_partial_eq() {
        assert_eq!(Language::Rust, Language::Rust);
        assert_ne!(Language::Rust, Language::Python);

        assert_eq!(Severity::Error, Severity::Error);
        assert_ne!(Severity::Error, Severity::Warning);

        assert_eq!(RuleId::new("test").unwrap(), RuleId::new("test").unwrap());
        assert_ne!(RuleId::new("test1").unwrap(), RuleId::new("test2").unwrap());

        assert_eq!(RegionPath::new("src"), RegionPath::new("src"));
        assert_ne!(RegionPath::new("src"), RegionPath::new("tests"));

        assert_eq!(GlobPattern::new("*.rs"), GlobPattern::new("*.rs"));
        assert_ne!(GlobPattern::new("*.rs"), GlobPattern::new("*.toml"));
    }
}
