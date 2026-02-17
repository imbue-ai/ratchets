#![forbid(unsafe_code)]

//! Regex-based rule implementation
//!
//! This module provides RegexRule, which matches text patterns in source files
//! using regular expressions.

use crate::error::RuleError;
use crate::rules::{ExecutionContext, Rule, RuleContext, Violation};
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
#[serde(untagged)]
enum GlobPatternList {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct MatchSection {
    pattern: String,
    #[serde(default)]
    languages: Option<Vec<Language>>,
    #[serde(default)]
    include: Option<GlobPatternList>,
    #[serde(default)]
    exclude: Option<GlobPatternList>,
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
        Self::from_toml_with_context(content, None)
    }

    /// Parse a RegexRule from TOML content with pattern context
    ///
    /// This method allows resolving pattern references (e.g., @python_tests) using
    /// the provided RuleContext.
    ///
    /// # Errors
    ///
    /// Returns `RuleError::InvalidDefinition` if:
    /// - TOML syntax is invalid
    /// - Required fields are missing
    /// - Rule ID is invalid
    /// - Regex pattern is invalid
    /// - Glob patterns are invalid
    /// - A pattern reference is not found in the context
    pub fn from_toml_with_context(
        content: &str,
        ctx: Option<&RuleContext>,
    ) -> Result<Self, RuleError> {
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
            Some(build_globset_with_context(&patterns, ctx)?)
        } else {
            None
        };

        // Build exclude GlobSet if specified
        let exclude = if let Some(patterns) = def.match_section.exclude {
            Some(build_globset_with_context(&patterns, ctx)?)
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

/// Build a GlobSet from a list of glob patterns or references
fn build_globset_with_context(
    pattern_list: &GlobPatternList,
    ctx: Option<&RuleContext>,
) -> Result<GlobSet, RuleError> {
    let mut builder = GlobSetBuilder::new();

    match pattern_list {
        GlobPatternList::Single(s) => {
            // Check if it's a reference
            if let Some(ref_name) = s.strip_prefix('@') {
                // Resolve the reference
                let patterns = resolve_pattern_reference(ref_name, ctx)?;
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
            } else {
                // It's a literal pattern
                let pattern = GlobPattern::new(s.clone());
                let glob = Glob::new(pattern.as_str()).map_err(|e| {
                    RuleError::InvalidDefinition(format!(
                        "Invalid glob pattern '{}': {}",
                        pattern.as_str(),
                        e
                    ))
                })?;
                builder.add(glob);
            }
        }
        GlobPatternList::Multiple(items) => {
            for item in items {
                // Check if it's a reference (starts with @)
                if let Some(ref_name) = item.strip_prefix('@') {
                    let patterns = resolve_pattern_reference(ref_name, ctx)?;
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
                } else {
                    // It's a literal pattern
                    let pattern = GlobPattern::new(item.clone());
                    let glob = Glob::new(pattern.as_str()).map_err(|e| {
                        RuleError::InvalidDefinition(format!(
                            "Invalid glob pattern '{}': {}",
                            pattern.as_str(),
                            e
                        ))
                    })?;
                    builder.add(glob);
                }
            }
        }
    }

    builder
        .build()
        .map_err(|e| RuleError::InvalidDefinition(format!("Failed to build GlobSet: {}", e)))
}

/// Resolve a pattern reference to its actual patterns
fn resolve_pattern_reference<'a>(
    ref_name: &str,
    ctx: Option<&'a RuleContext>,
) -> Result<&'a [GlobPattern], RuleError> {
    let ctx = ctx.ok_or_else(|| {
        RuleError::InvalidDefinition(format!(
            "Pattern reference '@{}' cannot be resolved: no pattern context provided",
            ref_name
        ))
    })?;

    ctx.patterns
        .get(ref_name)
        .map(|v| v.as_slice())
        .ok_or_else(|| {
            RuleError::InvalidDefinition(format!("Unknown pattern reference: @{}", ref_name))
        })
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
            region_resolver: None,
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
            region_resolver: None,
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
            region_resolver: None,
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
            region_resolver: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);

        // File outside src/ should not match
        let ctx = ExecutionContext {
            file_path: Path::new("tests/test.rs"),
            content: "// TODO: fix",
            ast: None,
            region_resolver: None,
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
            region_resolver: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 0);

        // File outside tests/ should match
        let ctx = ExecutionContext {
            file_path: Path::new("src/main.rs"),
            content: "// TODO: fix",
            ast: None,
            region_resolver: None,
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
            region_resolver: None,
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
            region_resolver: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].line, 3);
        assert_eq!(violations[1].line, 5);
    }

    #[test]
    fn test_pattern_reference_single() {
        use crate::rules::RuleContext;
        use crate::types::GlobPattern;
        use std::collections::HashMap;

        let toml = r#"
[rule]
id = "test-rule"
description = "Test with pattern reference"
severity = "error"

[match]
pattern = "TODO"
exclude = "@test_files"
"#;

        // Create a context with test_files pattern
        let mut patterns = HashMap::new();
        patterns.insert(
            "test_files".to_string(),
            vec![
                GlobPattern::new("**/test_*.py"),
                GlobPattern::new("**/*_test.py"),
            ],
        );
        let ctx = RuleContext::new(patterns);

        let rule = RegexRule::from_toml_with_context(toml, Some(&ctx)).unwrap();

        // File matching the pattern reference should be excluded
        let ctx_test = ExecutionContext {
            file_path: Path::new("test_foo.py"),
            content: "// TODO: fix this",
            ast: None,
            region_resolver: None,
        };
        let violations = rule.execute(&ctx_test);
        assert_eq!(violations.len(), 0);

        // File not matching should be included
        let ctx_normal = ExecutionContext {
            file_path: Path::new("main.py"),
            content: "// TODO: fix this",
            ast: None,
            region_resolver: None,
        };
        let violations = rule.execute(&ctx_normal);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pattern_reference_mixed() {
        use crate::rules::RuleContext;
        use crate::types::GlobPattern;
        use std::collections::HashMap;

        let toml = r#"
[rule]
id = "test-rule"
description = "Test with mixed patterns"
severity = "error"

[match]
pattern = "TODO"
exclude = ["@test_files", "build/**"]
"#;

        let mut patterns = HashMap::new();
        patterns.insert(
            "test_files".to_string(),
            vec![GlobPattern::new("**/test_*.py")],
        );
        let ctx = RuleContext::new(patterns);

        let rule = RegexRule::from_toml_with_context(toml, Some(&ctx)).unwrap();

        // Test reference match
        let ctx1 = ExecutionContext {
            file_path: Path::new("test_foo.py"),
            content: "// TODO: fix this",
            ast: None,
            region_resolver: None,
        };
        assert_eq!(rule.execute(&ctx1).len(), 0);

        // Test literal match
        let ctx2 = ExecutionContext {
            file_path: Path::new("build/main.py"),
            content: "// TODO: fix this",
            ast: None,
            region_resolver: None,
        };
        assert_eq!(rule.execute(&ctx2).len(), 0);

        // Test non-match
        let ctx3 = ExecutionContext {
            file_path: Path::new("src/main.py"),
            content: "// TODO: fix this",
            ast: None,
            region_resolver: None,
        };
        assert_eq!(rule.execute(&ctx3).len(), 1);
    }

    #[test]
    fn test_pattern_reference_not_found() {
        use crate::rules::RuleContext;

        let toml = r#"
[rule]
id = "test-rule"
description = "Test with unknown pattern"
severity = "error"

[match]
pattern = "TODO"
exclude = "@unknown_pattern"
"#;

        let ctx = RuleContext::empty();
        let result = RegexRule::from_toml_with_context(toml, Some(&ctx));

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown pattern reference")
        );
    }

    #[test]
    fn test_pattern_reference_no_context() {
        let toml = r#"
[rule]
id = "test-rule"
description = "Test with pattern reference but no context"
severity = "error"

[match]
pattern = "TODO"
exclude = "@test_files"
"#;

        let result = RegexRule::from_toml_with_context(toml, None);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no pattern context provided")
        );
    }

    #[test]
    fn test_backward_compatibility() {
        // Regular patterns should work without context
        let toml = r#"
[rule]
id = "test-rule"
description = "Test backward compatibility"
severity = "error"

[match]
pattern = "TODO"
exclude = ["**/tests/**"]
"#;

        let result = RegexRule::from_toml(toml);
        assert!(result.is_ok());
    }
}
