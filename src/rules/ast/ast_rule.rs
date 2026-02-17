#![forbid(unsafe_code)]

//! AST-based rule implementation using tree-sitter queries
//!
//! This module provides AstRule, which uses tree-sitter queries to match
//! patterns in parsed abstract syntax trees.

use crate::error::RuleError;
use crate::rules::ast::ParserCache;
use crate::rules::{ExecutionContext, RegionResolver, Rule, RuleContext, Violation};
use crate::types::{GlobPattern, Language, RegionPath, RuleId, Severity};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::path::Path;
use tree_sitter::{Query, QueryCursor, Tree};

/// TOML structure for AST rule definitions
///
/// This structure is deserialized from TOML files in ratchets/ast/ or
/// builtin-ratchets/{language}/ast/ directories.
#[derive(Debug, Deserialize)]
struct AstRuleDefinition {
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
    query: String,
    language: Language,
    #[serde(default)]
    include: Option<GlobPatternList>,
    #[serde(default)]
    exclude: Option<GlobPatternList>,
    #[serde(default)]
    post_filter: Option<String>,
}

/// A rule that matches AST patterns using tree-sitter queries
///
/// AstRule compiles a tree-sitter query and executes it against parsed ASTs,
/// reporting matches at the @violation capture (or the first capture if @violation is not present).
pub struct AstRule {
    id: RuleId,
    description: String,
    severity: Severity,
    query_source: String,
    language: Language,
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
    post_filter: Option<PostFilter>,
}

/// Post-filter function for additional violation filtering
///
/// Some rules require filtering based on captured node text, which tree-sitter
/// queries cannot express (e.g., negative string matching).
#[derive(Debug, Clone, Copy)]
enum PostFilter {
    /// Filter out classes whose names end with "Exception" or "Error"
    ClassNameNotException,
}

impl std::fmt::Debug for AstRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AstRule")
            .field("id", &self.id)
            .field("description", &self.description)
            .field("severity", &self.severity)
            .field("query_source", &self.query_source)
            .field("language", &self.language)
            .field("include", &"<GlobSet>")
            .field("exclude", &"<GlobSet>")
            .field("post_filter", &self.post_filter)
            .finish()
    }
}

impl AstRule {
    /// Parse an AstRule from TOML content
    ///
    /// # Errors
    ///
    /// Returns `RuleError::InvalidDefinition` if:
    /// - TOML syntax is invalid
    /// - Required fields are missing
    /// - Rule ID is invalid
    /// - Glob patterns are invalid
    ///
    /// Returns `RuleError::InvalidQuery` if the tree-sitter query is invalid
    pub fn from_toml(content: &str) -> Result<Self, RuleError> {
        Self::from_toml_with_context(content, None)
    }

    /// Parse an AstRule from TOML content with pattern context
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
    /// - Glob patterns are invalid
    /// - A pattern reference is not found in the context
    ///
    /// Returns `RuleError::InvalidQuery` if the tree-sitter query is invalid
    pub fn from_toml_with_context(
        content: &str,
        ctx: Option<&RuleContext>,
    ) -> Result<Self, RuleError> {
        // Parse TOML
        let def: AstRuleDefinition = toml::from_str(content)
            .map_err(|e| RuleError::InvalidDefinition(format!("Failed to parse TOML: {}", e)))?;

        // Validate and create rule ID
        let id = RuleId::new(def.rule.id.clone()).ok_or_else(|| {
            RuleError::InvalidDefinition(format!("Invalid rule ID: {}", def.rule.id))
        })?;

        // Store query source for later compilation
        let query_source = def.match_section.query;

        // Validate the query can be compiled (we'll compile it fresh each time we need it)
        validate_query(&query_source, def.match_section.language)?;

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

        // Parse post_filter if specified
        let post_filter = if let Some(filter_name) = def.match_section.post_filter {
            Some(parse_post_filter(&filter_name)?)
        } else {
            None
        };

        Ok(AstRule {
            id,
            description: def.rule.description,
            severity: def.rule.severity,
            query_source,
            language: def.match_section.language,
            include,
            exclude,
            post_filter,
        })
    }

    /// Parse an AstRule from a TOML file path
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

    /// Execute the query with an actual tree-sitter tree
    ///
    /// This method performs the actual query execution against a parsed tree.
    /// It's called by the Rule trait's execute() method when a tree is available.
    ///
    /// # Parameters
    ///
    /// - `tree`: The parsed tree-sitter tree
    /// - `content`: The source code content
    /// - `file_path`: Path to the file being analyzed
    ///
    /// # Returns
    ///
    /// Vector of violations found by the query
    pub fn execute_with_tree(
        &self,
        tree: &Tree,
        content: &str,
        file_path: &Path,
        region_resolver: Option<&RegionResolver>,
    ) -> Vec<Violation> {
        // Compile the query
        let parser_cache = ParserCache::new();
        let parser = match parser_cache.get_parser(self.language) {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let tree_sitter_lang = match parser.language() {
            Some(lang) => lang,
            None => return vec![], // Parser not properly configured
        };

        let query = match Query::new(&tree_sitter_lang, &self.query_source) {
            Ok(q) => q,
            Err(_) => return vec![],
        };

        // Find the @violation capture index, or use 0 if not found
        let violation_capture_idx = query
            .capture_names()
            .iter()
            .position(|name| *name == "violation")
            .unwrap_or(0);

        // Execute query
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

        let mut violations = Vec::new();

        for match_result in matches {
            // Apply post-filter if specified
            if let Some(filter) = self.post_filter
                && !apply_post_filter(filter, &query, &match_result, content)
            {
                continue;
            }

            // Find the violation capture (or first capture if @violation doesn't exist)
            let capture = if let Some(capture) = match_result
                .captures
                .iter()
                .find(|c| c.index as usize == violation_capture_idx)
            {
                capture
            } else if let Some(first) = match_result.captures.first() {
                first
            } else {
                continue;
            };

            let node = capture.node;

            // Convert tree-sitter positions (0-indexed) to 1-indexed line/column
            let start_pos = node.start_position();
            let end_pos = node.end_position();

            let line = start_pos.row as u32 + 1;
            let column = start_pos.column as u32 + 1;
            let end_line = end_pos.row as u32 + 1;
            let end_column = end_pos.column as u32 + 1;

            // Extract snippet
            let snippet = content[node.byte_range()].to_string();

            // Determine region using resolver if available, else fall back to parent directory
            let region = if let Some(resolver) = region_resolver {
                resolver(file_path, &self.id)
            } else if let Some(parent) = file_path.parent() {
                RegionPath::new(parent.to_string_lossy().to_string())
            } else {
                RegionPath::new(".")
            };

            violations.push(Violation {
                rule_id: self.id.clone(),
                file: file_path.to_path_buf(),
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

/// Validate that a query can be compiled for the given language
fn validate_query(query_source: &str, language: Language) -> Result<(), RuleError> {
    let parser_cache = ParserCache::new();
    let parser = parser_cache
        .get_parser(language)
        .map_err(|e| RuleError::InvalidQuery(format!("Failed to get parser: {}", e)))?;

    let tree_sitter_lang = parser
        .language()
        .ok_or_else(|| RuleError::InvalidQuery("Parser language not configured".to_string()))?;

    Query::new(&tree_sitter_lang, query_source)
        .map_err(|e| RuleError::InvalidQuery(format!("Failed to compile query: {}", e)))?;

    Ok(())
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

/// Parse a post-filter name into a PostFilter enum
fn parse_post_filter(filter_name: &str) -> Result<PostFilter, RuleError> {
    match filter_name {
        "class_name_not_exception" => Ok(PostFilter::ClassNameNotException),
        _ => Err(RuleError::InvalidDefinition(format!(
            "Unknown post_filter: {}",
            filter_name
        ))),
    }
}

/// Apply a post-filter to a query match
///
/// Returns true if the match should be kept as a violation, false if it should be filtered out.
fn apply_post_filter(
    filter: PostFilter,
    query: &Query,
    match_result: &tree_sitter::QueryMatch,
    content: &str,
) -> bool {
    match filter {
        PostFilter::ClassNameNotException => {
            // Find the @class_name capture
            let class_name_idx = query
                .capture_names()
                .iter()
                .position(|name| *name == "class_name");

            if let Some(idx) = class_name_idx
                && let Some(capture) = match_result
                    .captures
                    .iter()
                    .find(|c| c.index as usize == idx)
            {
                let class_name = &content[capture.node.byte_range()];
                // Filter out (return false) if class name ends with Exception or Error
                if class_name.ends_with("Exception") || class_name.ends_with("Error") {
                    return false;
                }
            }
            true
        }
    }
}

impl Rule for AstRule {
    fn id(&self) -> &RuleId {
        &self.id
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn languages(&self) -> &[Language] {
        // AST rules are language-specific
        std::slice::from_ref(&self.language)
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn execute(&self, ctx: &ExecutionContext) -> Vec<Violation> {
        // Check if this rule applies to this file
        if !self.applies_to_file(ctx.file_path) {
            return vec![];
        }

        // For now, we need to parse the file ourselves since ExecutionContext.ast
        // uses AstPlaceholder. When AST integration is complete, we'll use ctx.ast.
        let parser_cache = ParserCache::new();
        let mut parser = match parser_cache.get_parser(self.language) {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let tree = match parser.parse(ctx.content, None) {
            Some(t) => t,
            None => return vec![],
        };

        // Execute the query using our stored query_source
        self.execute_with_tree(
            &tree,
            ctx.content,
            ctx.file_path,
            ctx.region_resolver.as_ref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_toml_simple() {
        let toml = r#"
[rule]
id = "test-ast-rule"
description = "Test AST rule"
severity = "error"

[match]
query = """
(identifier) @violation
"""
language = "rust"
"#;

        let rule = AstRule::from_toml(toml).unwrap();
        assert_eq!(rule.id.as_str(), "test-ast-rule");
        assert_eq!(rule.description, "Test AST rule");
        assert_eq!(rule.severity, Severity::Error);
        assert_eq!(rule.language, Language::Rust);
        assert!(rule.include.is_none());
        assert!(rule.exclude.is_none());
    }

    #[test]
    fn test_from_toml_with_globs() {
        let toml = r#"
[rule]
id = "src-only"
description = "Applies to src only"
severity = "info"

[match]
query = "(identifier) @violation"
language = "rust"
include = ["src/**"]
exclude = ["src/test/**"]
"#;

        let rule = AstRule::from_toml(toml).unwrap();
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
query = "(identifier) @violation"
language = "rust"
"#;

        let result = AstRule::from_toml(toml);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RuleError::InvalidDefinition(_)
        ));
    }

    #[test]
    fn test_from_toml_invalid_query() {
        let toml = r#"
[rule]
id = "bad-query"
description = "Test"
severity = "error"

[match]
query = "(unclosed"
language = "rust"
"#;

        let result = AstRule::from_toml(toml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RuleError::InvalidQuery(_)));
    }

    #[test]
    fn test_from_toml_missing_field() {
        let toml = r#"
[rule]
id = "incomplete"
description = "Test"

[match]
query = "(identifier) @violation"
"#;

        let result = AstRule::from_toml(toml);
        assert!(result.is_err());
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_execute_simple_match() {
        let toml = r#"
[rule]
id = "find-unwrap"
description = "Find unwrap calls"
severity = "error"

[match]
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
language = "rust"
"#;

        let rule = AstRule::from_toml(toml).unwrap();

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "fn main() { let x = Some(5).unwrap(); }",
            ast: None,
            region_resolver: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].snippet.contains("unwrap"));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_execute_multiple_matches() {
        let toml = r#"
[rule]
id = "find-unwrap"
description = "Find unwrap calls"
severity = "error"

[match]
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
language = "rust"
"#;

        let rule = AstRule::from_toml(toml).unwrap();

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "fn main() {\n    let x = Some(5).unwrap();\n    let y = Some(10).unwrap();\n}",
            ast: None,
            region_resolver: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 2);
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_execute_no_match() {
        let toml = r#"
[rule]
id = "find-unwrap"
description = "Find unwrap calls"
severity = "error"

[match]
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
language = "rust"
"#;

        let rule = AstRule::from_toml(toml).unwrap();

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "fn main() { let x = Some(5); }",
            ast: None,
            region_resolver: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 0);
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_execute_respects_include() {
        let toml = r#"
[rule]
id = "find-unwrap"
description = "Find unwrap in src only"
severity = "error"

[match]
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
language = "rust"
include = ["src/**"]
"#;

        let rule = AstRule::from_toml(toml).unwrap();

        // File in src/ should match
        let ctx = ExecutionContext {
            file_path: Path::new("src/main.rs"),
            content: "fn main() { Some(5).unwrap(); }",
            ast: None,
            region_resolver: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);

        // File outside src/ should not match
        let ctx = ExecutionContext {
            file_path: Path::new("tests/test.rs"),
            content: "fn test() { Some(5).unwrap(); }",
            ast: None,
            region_resolver: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 0);
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_execute_respects_exclude() {
        let toml = r#"
[rule]
id = "find-unwrap"
description = "Find unwrap except in tests"
severity = "error"

[match]
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
language = "rust"
exclude = ["tests/**"]
"#;

        let rule = AstRule::from_toml(toml).unwrap();

        // File in tests/ should not match
        let ctx = ExecutionContext {
            file_path: Path::new("tests/test.rs"),
            content: "fn test() { Some(5).unwrap(); }",
            ast: None,
            region_resolver: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 0);

        // File outside tests/ should match
        let ctx = ExecutionContext {
            file_path: Path::new("src/main.rs"),
            content: "fn main() { Some(5).unwrap(); }",
            ast: None,
            region_resolver: None,
        };
        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_violation_positions() {
        let toml = r#"
[rule]
id = "find-let"
description = "Find let statements"
severity = "info"

[match]
query = "(let_declaration) @violation"
language = "rust"
"#;

        let rule = AstRule::from_toml(toml).unwrap();

        let content = "fn main() {\n    let x = 5;\n}";
        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content,
            ast: None,
            region_resolver: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, 2);
        assert!(violations[0].column > 0);
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_query_without_violation_capture() {
        let toml = r#"
[rule]
id = "find-identifier"
description = "Find any identifier"
severity = "info"

[match]
query = "(identifier) @id"
language = "rust"
"#;

        let rule = AstRule::from_toml(toml).unwrap();

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "fn main() {}",
            ast: None,
            region_resolver: None,
        };

        let violations = rule.execute(&ctx);
        // Should find at least "main"
        assert!(!violations.is_empty());
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_execute_with_tree_direct() {
        let toml = r#"
[rule]
id = "find-unwrap"
description = "Find unwrap calls"
severity = "error"

[match]
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
language = "rust"
"#;

        let rule = AstRule::from_toml(toml).unwrap();

        // Parse the content
        let content = "fn main() { Some(5).unwrap(); }";
        let parser_cache = ParserCache::new();
        let mut parser = parser_cache.get_parser(Language::Rust).unwrap();
        let tree = parser.parse(content, None).unwrap();

        // Execute with tree
        let violations = rule.execute_with_tree(&tree, content, Path::new("test.rs"), None);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].snippet.contains("unwrap"));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_ast_rule_uses_configured_region() {
        use crate::rules::RegionResolver;
        use crate::types::RegionPath;
        use std::sync::Arc;

        let rule = AstRule::from_toml(
            r#"
[rule]
id = "test-rule"
description = "Test rule"
severity = "error"

[match]
query = "(call_expression) @violation"
language = "rust"
"#,
        )
        .unwrap();

        // Create a resolver that always returns "configured/region"
        let resolver: RegionResolver =
            Arc::new(|_path, _rule_id| RegionPath::new("configured/region"));

        let ctx = ExecutionContext {
            file_path: Path::new("src/deep/nested/file.rs"),
            content: "fn main() { foo(); }",
            ast: None,
            region_resolver: Some(resolver),
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].region.as_str(), "configured/region");
    }
}
