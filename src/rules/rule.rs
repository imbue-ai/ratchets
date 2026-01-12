#![forbid(unsafe_code)]

//! Core Rule trait and related types for defining and executing rules

use crate::types::{Language, RegionPath, RuleId, Severity};
use std::path::{Path, PathBuf};

/// A placeholder for AST types until tree-sitter is added
///
/// This allows the API to be defined without adding the tree-sitter dependency yet.
/// When tree-sitter is added, this will be replaced with `tree_sitter::Tree`.
#[derive(Debug)]
pub struct AstPlaceholder;

/// Execution context provided to rules when they execute
///
/// This contains all the information a rule needs to analyze a file.
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    /// Path to the file being analyzed
    pub file_path: &'a Path,

    /// Full text content of the file
    pub content: &'a str,

    /// Optional parsed AST (for AST-based rules)
    ///
    /// Currently uses a placeholder type. Will use `tree_sitter::Tree` when available.
    pub ast: Option<&'a AstPlaceholder>,
}

/// A single code violation detected by a rule
///
/// This structure captures all information needed to report and serialize a violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// ID of the rule that detected this violation
    pub rule_id: RuleId,

    /// File path where the violation was found
    pub file: PathBuf,

    /// Line number where violation starts (1-indexed)
    pub line: u32,

    /// Column number where violation starts (1-indexed)
    pub column: u32,

    /// Line number where violation ends (1-indexed)
    pub end_line: u32,

    /// Column number where violation ends (1-indexed)
    pub end_column: u32,

    /// Code snippet at the violation location
    pub snippet: String,

    /// Human-readable message describing the violation
    pub message: String,

    /// Region path for aggregation and budget tracking
    pub region: RegionPath,
}

/// Trait that all rules must implement
///
/// Rules are responsible for analyzing source code and detecting violations.
/// The trait is `Send + Sync` to enable parallel execution across files.
pub trait Rule: Send + Sync {
    /// Returns the unique identifier for this rule
    fn id(&self) -> &RuleId;

    /// Returns a human-readable description of what this rule checks
    fn description(&self) -> &str;

    /// Returns the languages this rule applies to
    fn languages(&self) -> &[Language];

    /// Returns the severity level of violations from this rule
    fn severity(&self) -> Severity;

    /// Executes the rule against the provided context
    ///
    /// Returns a vector of all violations found in the file.
    /// Returns an empty vector if no violations are found.
    fn execute(&self, ctx: &ExecutionContext) -> Vec<Violation>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_violation_construction() {
        let rule_id = RuleId::new("test-rule").unwrap();
        let region = RegionPath::new("src");

        let violation = Violation {
            rule_id: rule_id.clone(),
            file: PathBuf::from("src/test.rs"),
            line: 10,
            column: 5,
            end_line: 10,
            end_column: 15,
            snippet: ".unwrap()".to_string(),
            message: "Test violation".to_string(),
            region: region.clone(),
        };

        assert_eq!(violation.rule_id, rule_id);
        assert_eq!(violation.line, 10);
        assert_eq!(violation.column, 5);
        assert_eq!(violation.snippet, ".unwrap()");
    }

    #[test]
    fn test_violation_clone() {
        let rule_id = RuleId::new("test-rule").unwrap();
        let region = RegionPath::new("src");

        let violation = Violation {
            rule_id,
            file: PathBuf::from("src/test.rs"),
            line: 10,
            column: 5,
            end_line: 10,
            end_column: 15,
            snippet: ".unwrap()".to_string(),
            message: "Test violation".to_string(),
            region,
        };

        let cloned = violation.clone();
        assert_eq!(violation, cloned);
    }

    #[test]
    fn test_execution_context_construction() {
        let path = Path::new("test.rs");
        let content = "fn main() {}";
        let ast_placeholder = AstPlaceholder;

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: Some(&ast_placeholder),
        };

        assert_eq!(ctx.file_path, path);
        assert_eq!(ctx.content, content);
        assert!(ctx.ast.is_some());
    }

    #[test]
    fn test_execution_context_without_ast() {
        let path = Path::new("test.rs");
        let content = "fn main() {}";

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: None,
        };

        assert_eq!(ctx.file_path, path);
        assert_eq!(ctx.content, content);
        assert!(ctx.ast.is_none());
    }

    // Mock rule for testing trait implementation
    struct MockRule {
        rule_id: RuleId,
        description: String,
        languages: Vec<Language>,
        severity: Severity,
    }

    impl Rule for MockRule {
        fn id(&self) -> &RuleId {
            &self.rule_id
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

        fn execute(&self, _ctx: &ExecutionContext) -> Vec<Violation> {
            vec![]
        }
    }

    #[test]
    fn test_rule_trait_implementation() {
        let rule = MockRule {
            rule_id: RuleId::new("test-rule").unwrap(),
            description: "Test description".to_string(),
            languages: vec![Language::Rust, Language::Python],
            severity: Severity::Error,
        };

        assert_eq!(rule.id().as_str(), "test-rule");
        assert_eq!(rule.description(), "Test description");
        assert_eq!(rule.languages(), &[Language::Rust, Language::Python]);
        assert_eq!(rule.severity(), Severity::Error);

        let ctx = ExecutionContext {
            file_path: Path::new("test.rs"),
            content: "fn main() {}",
            ast: None,
        };

        let violations = rule.execute(&ctx);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_rule_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        // This test ensures that types implementing Rule are Send + Sync
        assert_send::<Box<dyn Rule>>();
        assert_sync::<Box<dyn Rule>>();
    }
}
