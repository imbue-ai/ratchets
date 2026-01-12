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

    #[test]
    fn test_rule_id_validation() {
        assert!(RuleId::new("valid-rule").is_some());
        assert!(RuleId::new("rule_123").is_some());
        assert!(RuleId::new("no-unwrap").is_some());
        assert!(RuleId::new("").is_none());
        assert!(RuleId::new("invalid rule").is_none());
        assert!(RuleId::new("invalid@rule").is_none());
    }

    #[test]
    fn test_region_path_normalization() {
        assert_eq!(RegionPath::new("").as_str(), ".");
        assert_eq!(RegionPath::new(".").as_str(), ".");
        assert_eq!(RegionPath::new("./").as_str(), ".");
        assert_eq!(RegionPath::new("/").as_str(), ".");
        assert_eq!(RegionPath::new("src").as_str(), "src");
        assert_eq!(RegionPath::new("src/").as_str(), "src");
        assert_eq!(RegionPath::new("./src").as_str(), "src");
        assert_eq!(RegionPath::new("src\\parser").as_str(), "src/parser");
        assert_eq!(RegionPath::new("src/parser/").as_str(), "src/parser");
    }

    #[test]
    fn test_glob_pattern() {
        let pattern = GlobPattern::new("**/*.rs");
        assert_eq!(pattern.as_str(), "**/*.rs");
    }

    #[test]
    fn test_type_derives() {
        // Verify all types implement Hash for use in HashMaps/HashSets
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
}
