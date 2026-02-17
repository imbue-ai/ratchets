#![forbid(unsafe_code)]

//! Core Rule trait and related types for defining and executing rules

use crate::types::{GlobPattern, Language, RegionPath, RuleId, Severity};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Type alias for the region resolver function
///
/// Given a file path and rule ID, returns the configured region for that file.
/// This allows rules to assign violations to the correct configured region
/// when executing.
pub type RegionResolver = Arc<dyn Fn(&Path, &RuleId) -> RegionPath + Send + Sync>;

/// A placeholder for AST types until tree-sitter is added
///
/// This allows the API to be defined without adding the tree-sitter dependency yet.
/// When tree-sitter is added, this will be replaced with `tree_sitter::Tree`.
#[derive(Debug)]
pub struct AstPlaceholder;

/// Context for resolving pattern references in rule definitions
///
/// This context contains pattern definitions from ratchets.toml that can be
/// referenced in rule files using @pattern_name syntax.
#[derive(Debug, Clone, Default)]
pub struct RuleContext {
    /// Map of pattern names to their glob patterns
    pub patterns: HashMap<String, Vec<GlobPattern>>,
}

impl RuleContext {
    /// Create a new RuleContext with the given patterns
    pub fn new(patterns: HashMap<String, Vec<GlobPattern>>) -> Self {
        Self { patterns }
    }

    /// Create an empty RuleContext
    pub fn empty() -> Self {
        Self::default()
    }
}

/// Execution context provided to rules when they execute
///
/// This contains all the information a rule needs to analyze a file.
pub struct ExecutionContext<'a> {
    /// Path to the file being analyzed
    pub file_path: &'a Path,

    /// Full text content of the file
    pub content: &'a str,

    /// Optional parsed AST (for AST-based rules)
    ///
    /// Currently uses a placeholder type. Will use `tree_sitter::Tree` when available.
    pub ast: Option<&'a AstPlaceholder>,

    /// Optional region resolver for mapping files to configured regions
    ///
    /// When Some, rules should use this to determine the region for violations.
    /// When None, rules should fall back to using the file's parent directory.
    pub region_resolver: Option<RegionResolver>,
}

impl fmt::Debug for ExecutionContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("file_path", &self.file_path)
            .field("content", &format!("<{} bytes>", self.content.len()))
            .field("ast", &self.ast.map(|_| "<AstPlaceholder>"))
            .field(
                "region_resolver",
                &self.region_resolver.as_ref().map(|_| "<RegionResolver>"),
            )
            .finish()
    }
}

impl<'a> ExecutionContext<'a> {
    /// Resolves the region for a violation
    ///
    /// If a region resolver is configured, uses it to find the configured region.
    /// Otherwise, falls back to using the file's parent directory.
    pub fn resolve_region(&self, rule_id: &RuleId) -> RegionPath {
        if let Some(ref resolver) = self.region_resolver {
            resolver(self.file_path, rule_id)
        } else {
            // Fallback: use parent directory
            self.file_path
                .parent()
                .map(|p| RegionPath::new(p.to_string_lossy()))
                .unwrap_or_else(|| RegionPath::new("."))
        }
    }
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
            region_resolver: None,
        };

        assert_eq!(ctx.file_path, path);
        assert_eq!(ctx.content, content);
        assert!(ctx.ast.is_some());
        assert!(ctx.region_resolver.is_none());
    }

    #[test]
    fn test_execution_context_without_ast() {
        let path = Path::new("test.rs");
        let content = "fn main() {}";

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: None,
            region_resolver: None,
        };

        assert_eq!(ctx.file_path, path);
        assert_eq!(ctx.content, content);
        assert!(ctx.ast.is_none());
    }

    #[test]
    fn test_resolve_region_without_resolver() {
        let path = Path::new("src/foo/bar.rs");
        let content = "fn main() {}";

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: None,
            region_resolver: None,
        };

        let rule_id = RuleId::new("test-rule").unwrap();
        let region = ctx.resolve_region(&rule_id);

        // Without resolver, should fall back to parent directory
        assert_eq!(region.as_str(), "src/foo");
    }

    #[test]
    fn test_resolve_region_with_resolver() {
        let path = Path::new("src/foo/bar.rs");
        let content = "fn main() {}";

        // Create a resolver that always returns a specific region
        let resolver: RegionResolver = Arc::new(|_path, _rule_id| RegionPath::new("custom/region"));

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: None,
            region_resolver: Some(resolver),
        };

        let rule_id = RuleId::new("test-rule").unwrap();
        let region = ctx.resolve_region(&rule_id);

        // With resolver, should use the resolved region
        assert_eq!(region.as_str(), "custom/region");
    }

    #[test]
    fn test_resolve_region_resolver_receives_correct_args() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let path = Path::new("src/test.rs");
        let content = "fn main() {}";

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        // Create a resolver that checks its arguments
        let resolver: RegionResolver = Arc::new(move |file_path, rule_id| {
            called_clone.store(true, Ordering::SeqCst);
            // Verify the arguments are passed correctly
            assert_eq!(file_path, Path::new("src/test.rs"));
            assert_eq!(rule_id.as_str(), "my-rule");
            RegionPath::new("resolved")
        });

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: None,
            region_resolver: Some(resolver),
        };

        let rule_id = RuleId::new("my-rule").unwrap();
        let region = ctx.resolve_region(&rule_id);

        assert!(called.load(Ordering::SeqCst));
        assert_eq!(region.as_str(), "resolved");
    }

    #[test]
    fn test_resolve_region_root_file() {
        let path = Path::new("main.rs");
        let content = "fn main() {}";

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: None,
            region_resolver: None,
        };

        let rule_id = RuleId::new("test-rule").unwrap();
        let region = ctx.resolve_region(&rule_id);

        // Root file's parent is empty, should fall back to "."
        assert_eq!(region.as_str(), ".");
    }

    #[test]
    fn test_execution_context_debug() {
        let path = Path::new("test.rs");
        let content = "fn main() {}";

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: None,
            region_resolver: None,
        };

        // Should not panic and should produce reasonable output
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("ExecutionContext"));
        assert!(debug_str.contains("test.rs"));
    }

    #[test]
    fn test_execution_context_debug_with_resolver() {
        let path = Path::new("test.rs");
        let content = "fn main() {}";

        let resolver: RegionResolver = Arc::new(|_path, _rule_id| RegionPath::new("."));

        let ctx = ExecutionContext {
            file_path: path,
            content,
            ast: None,
            region_resolver: Some(resolver),
        };

        // Should not panic and should produce reasonable output
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("ExecutionContext"));
        assert!(debug_str.contains("RegionResolver"));
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
            region_resolver: None,
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
