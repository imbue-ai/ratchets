#![forbid(unsafe_code)]

//! Regex-based rule implementation
//!
//! This module provides RegexRule, which matches text patterns in source files
//! using regular expressions.

use crate::error::RuleError;
use crate::rules::{ExecutionContext, Rule, Violation};
use crate::types::{GlobPattern, Language, RegionPath, RuleId, Severity};
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

/// TOML structure for regex rule definitions
///
/// This structure is deserialized from TOML files in ratchets/regex/ or
/// builtin-ratchets/regex/ directories.
#[derive(Debug, Deserialize)]
struct RegexRuleDefinition {
    rule: RuleSection,
    #[serde(rename = "match")]
    match_section: MatchSection,
}

#[derive(Debug, Deserialize)]
struct RuleSection {
    id: String,
    description: String,
    severity: Severity,
}

#[derive(Debug, Deserialize)]
struct MatchSection {
    pattern: String,
    #[serde(default)]
    languages: Option<Vec<Language>>,
    #[serde(default)]
    include: Option<Vec<GlobPattern>>,
    #[serde(default)]
    exclude: Option<Vec<GlobPattern>>,
}

/// A rule that matches text patterns using regular expressions
///
/// RegexRule compiles a regex pattern and executes it against file content,
/// reporting all matches as violations.
pub struct RegexRule {
    id: RuleId,
    description: String,
    severity: Severity,
    pattern: Regex,
    languages: Vec<Language>,
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl std::fmt::Debug for RegexRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegexRule")
            .field("id", &self.id)
            .field("description", &self.description)
            .field("severity", &self.severity)
            .field("pattern", &self.pattern.as_str())
            .field("languages", &self.languages)
            .field("include", &"<GlobSet>")
            .field("exclude", &"<GlobSet>")
            .finish()
    }
}

impl RegexRule {
    /// Parse a RegexRule from TOML content
    ///
    /// # Errors
    ///
    /// Returns `RuleError::InvalidDefinition` if:
    /// - TOML syntax is invalid
    /// - Required fields are missing
    /// - Rule ID is invalid
    /// - Regex pattern is invalid
    /// - Glob patterns are invalid
    pub fn from_toml(content: &str) -> Result<Self, RuleError> {
        // Parse TOML
        let def: RegexRuleDefinition = toml::from_str(content)
            .map_err(|e| RuleError::InvalidDefinition(format!("Failed to parse TOML: {}", e)))?;

        // Validate and create rule ID
        let id = RuleId::new(def.rule.id.clone()).ok_or_else(|| {
            RuleError::InvalidDefinition(format!("Invalid rule ID: {}", def.rule.id))
        })?;

        // Compile regex pattern
        let pattern = Regex::new(&def.match_section.pattern).map_err(|e| {
            RuleError::InvalidRegex(format!(
                "Failed to compile pattern '{}': {}",
                def.match_section.pattern, e
            ))
        })?;

        // Process languages (empty means all languages)
        let languages = def.match_section.languages.unwrap_or_default();

        // Build include GlobSet if specified
        let include = if let Some(patterns) = def.match_section.include {
            Some(build_globset(&patterns)?)
        } else {
            None
        };

        // Build exclude GlobSet if specified
        let exclude = if let Some(patterns) = def.match_section.exclude {
            Some(build_globset(&patterns)?)
        } else {
            None
        };

        Ok(RegexRule {
            id,
            description: def.rule.description,
            severity: def.rule.severity,
            pattern,
            languages,
            include,
            exclude,
        })
    }

    /// Parse a RegexRule from a TOML file path
    ///
    /// # Errors
    ///
    /// Returns `RuleError` if the file cannot be read or parsed.
    pub fn from_path(path: &Path) -> Result<Self, RuleError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            RuleError::InvalidDefinition(format!("Failed to read file {:?}: {}", path, e))
        })?;
        Self::from_toml(&content)
    }

    /// Check if this rule applies to the given file path
    fn applies_to_file(&self, file_path: &Path) -> bool {
        // Check exclude patterns first
        if let Some(ref exclude) = self.exclude
            && exclude.is_match(file_path)
        {
            return false;
        }

        // If include patterns are specified, check them
        if let Some(ref include) = self.include {
            include.is_match(file_path)
        } else {
            // No include patterns means match all files
            true
        }
    }
}

/// Build a GlobSet from a list of glob patterns
fn build_globset(patterns: &[GlobPattern]) -> Result<GlobSet, RuleError> {
    let mut builder = GlobSetBuilder::new();

    for pattern in patterns {
        let glob = Glob::new(pattern.as_str()).map_err(|e| {
            RuleError::InvalidDefinition(format!(
                "Invalid glob pattern '{}': {}",
                pattern.as_str(),
                e
            ))
        })?;
        builder.add(glob);
    }

    builder
        .build()
        .map_err(|e| RuleError::InvalidDefinition(format!("Failed to build GlobSet: {}", e)))
}

/// Compute line start offsets for efficient line/column conversion
///
/// Returns a vector where each element is the byte offset of the start of a line.
/// Line 0 starts at offset 0.
fn compute_line_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, c) in content.char_indices() {
        if c == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Convert byte offset to line and column numbers (1-indexed)
///
/// Uses binary search on precomputed line offsets for efficiency.
fn offset_to_line_col(offset: usize, line_offsets: &[usize]) -> (u32, u32) {
    // Binary search for the line containing this offset
    let line_idx = line_offsets
        .partition_point(|&o| o <= offset)
        .saturating_sub(1);

    let line = (line_idx + 1) as u32; // 1-indexed
    let col = (offset - line_offsets[line_idx] + 1) as u32; // 1-indexed

    (line, col)
}

impl Rule for RegexRule {
    fn id(&self) -> &RuleId {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn languages(&self) -> &[Language] {
        &self.languages
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn execute(&self, ctx: &ExecutionContext) -> Vec<Violation> {
        // Check if this rule applies to this file
        if !self.applies_to_file(ctx.file_path) {
            return vec![];
        }

        // Precompute line offsets for efficient position calculation
        let line_offsets = compute_line_offsets(ctx.content);

        // Find all matches
        let mut violations = Vec::new();

        for match_result in self.pattern.find_iter(ctx.content) {
            let match_start = match_result.start();
            let match_end = match_result.end();

            // Extract snippet
            let snippet = ctx.content[match_start..match_end].to_string();

            // Calculate line/column positions
            let (line, column) = offset_to_line_col(match_start, &line_offsets);
            let (end_line, end_column) = offset_to_line_col(match_end, &line_offsets);

            // Determine region from file path
            let region = if let Some(parent) = ctx.file_path.parent() {
                RegionPath::new(parent.to_string_lossy().to_string())
            } else {
                RegionPath::new(".")
            };

            violations.push(Violation {
                rule_id: self.id.clone(),
                file: ctx.file_path.to_path_buf(),
                line,
                column,
                end_line,
                end_column,
                snippet,
                message: self.description.clone(),
                region,
            });
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_toml_simple() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Test description"
severity = "error"

[match]
pattern = "\\bTODO\\b"
"#;

        let rule = RegexRule::from_toml(toml).unwrap();
        assert_eq!(rule.id.as_str(), "test-rule");
        assert_eq!(rule.description, "Test description");
        assert_eq!(rule.severity, Severity::Error);
        assert!(rule.languages.is_empty());
        assert!(rule.include.is_none());
        assert!(rule.exclude.is_none());
    }

    #[test]
    fn test_from_toml_with_languages() {
        let toml = r#"
[rule]
id = "js-rule"
description = "JavaScript specific"
severity = "warning"

[match]
pattern = "console\\.log"
languages = ["javascript", "typescript"]
"#;

        let rule = RegexRule::from_toml(toml).unwrap();
        assert_eq!(rule.languages.len(), 2);
        assert!(rule.languages.contains(&Language::JavaScript));
        assert!(rule.languages.contains(&Language::TypeScript));
    }

    #[test]
    fn test_from_toml_with_globs() {
        let toml = r#"
[rule]
id = "src-only"
description = "Applies to src only"
severity = "info"

[match]
pattern = "test"
include = ["src/**"]
exclude = ["src/test/**"]
"#;

        let rule = RegexRule::from_toml(toml).unwrap();
        assert!(rule.include.is_some());
        assert!(rule.exclude.is_some());
    }

    #[test]
    fn test_from_toml_invalid_rule_id() {
        let toml = r#"
[rule]
id = "invalid rule!"
description = "Test"
severity = "error"

[match]
pattern = "test"
"#;

        let result = RegexRule::from_toml(toml);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RuleError::InvalidDefinition(_)
        ));
    }

    #[test]
    fn test_from_toml_invalid_regex() {
        let toml = r#"
[rule]
id = "bad-regex"
description = "Test"
severity = "error"

[match]
pattern = "[unclosed"
"#;

        let result = RegexRule::from_toml(toml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RuleError::InvalidRegex(_)));
    }

    #[test]
    fn test_from_toml_missing_field() {
        let toml = r#"
[rule]
id = "incomplete"
description = "Test"

[match]
pattern = "test"
"#;

        let result = RegexRule::from_toml(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_line_offsets() {
        let content = "line1\nline2\nline3";
        let offsets = compute_line_offsets(content);
        assert_eq!(offsets, vec![0, 6, 12]);
    }

    #[test]
    fn test_compute_line_offsets_empty() {
        let content = "";
        let offsets = compute_line_offsets(content);
        assert_eq!(offsets, vec![0]);
    }

    #[test]
    fn test_compute_line_offsets_single_line() {
        let content = "single line";
        let offsets = compute_line_offsets(content);
        assert_eq!(offsets, vec![0]);
    }

    #[test]
    fn test_offset_to_line_col() {
        let content = "line1\nline2\nline3";
        let offsets = compute_line_offsets(content);

        // First character
        assert_eq!(offset_to_line_col(0, &offsets), (1, 1));

        // Last character of first line
        assert_eq!(offset_to_line_col(4, &offsets), (1, 5));

        // Newline character
        assert_eq!(offset_to_line_col(5, &offsets), (1, 6));

        // First character of second line
        assert_eq!(offset_to_line_col(6, &offsets), (2, 1));

        // First character of third line
        assert_eq!(offset_to_line_col(12, &offsets), (3, 1));
    }

    #[test]
    fn test_execute_simple_match() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Find TODO"
severity = "warning"

[match]
pattern = "TODO"
"#;

        let rule = RegexRule::from_toml(toml).unwrap();

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "// TODO: fix this\nfn main() {}",
            ast: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 1);
        assert_eq!(violations[0].snippet, "TODO");
    }

    #[test]
    fn test_execute_multiple_matches() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Find TODO"
severity = "warning"

[match]
pattern = "TODO"
"#;

        let rule = RegexRule::from_toml(toml).unwrap();

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "// TODO: fix\n// TODO: also fix\nfn main() {}",
            ast: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].line, 1);
        assert_eq!(violations[1].line, 2);
    }

    #[test]
    fn test_execute_no_match() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Find TODO"
severity = "warning"

[match]
pattern = "TODO"
"#;

        let rule = RegexRule::from_toml(toml).unwrap();

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "fn main() { println!(\"Hello\"); }",
            ast: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_execute_respects_include() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Find TODO in src only"
severity = "warning"

[match]
pattern = "TODO"
include = ["src/**"]
"#;

        let rule = RegexRule::from_toml(toml).unwrap();

        // File in src/ should match
        let ctx = ExecutionContext {
            file_path: Path::new("src/main.rs"),
            content: "// TODO: fix",
            ast: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);

        // File outside src/ should not match
        let ctx = ExecutionContext {
            file_path: Path::new("tests/test.rs"),
            content: "// TODO: fix",
            ast: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_execute_respects_exclude() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Find TODO except in tests"
severity = "warning"

[match]
pattern = "TODO"
exclude = ["tests/**"]
"#;

        let rule = RegexRule::from_toml(toml).unwrap();

        // File in tests/ should not match
        let ctx = ExecutionContext {
            file_path: Path::new("tests/test.rs"),
            content: "// TODO: fix",
            ast: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 0);

        // File outside tests/ should match
        let ctx = ExecutionContext {
            file_path: Path::new("src/main.rs"),
            content: "// TODO: fix",
            ast: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_case_insensitive_pattern() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Find TODO case-insensitive"
severity = "warning"

[match]
pattern = "(?i)\\bTODO\\b"
"#;

        let rule = RegexRule::from_toml(toml).unwrap();

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "// TODO: fix\n// todo: also\n// Todo: and this",
            ast: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 3);
    }

    #[test]
    fn test_multiline_content() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Find pattern"
severity = "warning"

[match]
pattern = "FIXME"
"#;

        let rule = RegexRule::from_toml(toml).unwrap();

        let content = "line 1\nline 2\nFIXME here\nline 4\nFIXME again\n";
        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content,
            ast: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].line, 3);
        assert_eq!(violations[1].line, 5);
    }
}
