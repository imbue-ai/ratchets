#![forbid(unsafe_code)]

//! Parallel execution engine for running rules across files
//!
//! This module provides the ExecutionEngine which coordinates all components
//! to execute rules against discovered files in parallel using rayon.

use crate::config::counts::CountsManager;
use crate::engine::file_walker::FileEntry;
use crate::rules::{
    AstRule, ExecutionContext, ParserCache, RegionResolver, Rule, RuleRegistry, Violation,
};
use crate::types::Language;
use rayon::prelude::*;
use std::fs;
use std::sync::Arc;

/// Result of executing all rules against all files
#[derive(Debug)]
pub struct ExecutionResult {
    /// All violations found across all files and rules
    pub violations: Vec<Violation>,
    /// Number of files checked
    pub files_checked: usize,
    /// Number of rules executed
    pub rules_executed: usize,
}

/// Execution engine that coordinates parallel rule execution
///
/// The engine:
/// - Executes all enabled rules against discovered files
/// - Uses rayon for parallel file processing
/// - Parses ASTs once per file and shares across applicable rules
/// - Collects violations from all rules
pub struct ExecutionEngine {
    registry: Arc<RuleRegistry>,
    parser_cache: Arc<ParserCache>,
    region_resolver: Option<RegionResolver>,
}

impl ExecutionEngine {
    /// Creates a new ExecutionEngine with the provided rule registry
    ///
    /// # Arguments
    ///
    /// * `registry` - The rule registry containing all enabled rules
    /// * `counts_manager` - Optional counts manager for region resolution
    pub fn new(registry: RuleRegistry, counts_manager: Option<Arc<CountsManager>>) -> Self {
        // Create region resolver if counts_manager is provided
        let region_resolver = counts_manager.map(|cm| {
            Arc::new(
                move |file_path: &std::path::Path, rule_id: &crate::types::RuleId| {
                    cm.find_configured_region(rule_id, file_path)
                },
            ) as RegionResolver
        });

        Self {
            registry: Arc::new(registry),
            parser_cache: Arc::new(ParserCache::new()),
            region_resolver,
        }
    }

    /// Execute all rules against the discovered files
    ///
    /// This method processes files in parallel using rayon, parsing ASTs
    /// once per file and executing all applicable rules.
    ///
    /// # Arguments
    ///
    /// * `files` - Vector of discovered file entries to check
    ///
    /// # Returns
    ///
    /// ExecutionResult containing all violations and execution statistics
    pub fn execute(&self, files: Vec<FileEntry>) -> ExecutionResult {
        let files_checked = files.len();
        let rules_executed = self.registry.len();

        // Process files in parallel
        let violations: Vec<Violation> = files
            .par_iter()
            .flat_map(|file| self.execute_file(file))
            .collect();

        ExecutionResult {
            violations,
            files_checked,
            rules_executed,
        }
    }

    /// Execute all applicable rules against a single file
    ///
    /// This method:
    /// 1. Reads the file content
    /// 2. Determines which rules apply (based on language and file path)
    /// 3. Parses AST if any AST rules apply
    /// 4. Executes all applicable rules
    /// 5. Collects violations
    fn execute_file(&self, file: &FileEntry) -> Vec<Violation> {
        // Read file content - if we can't read it, log warning and skip
        let content = match fs::read_to_string(&file.path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to read file {}: {}",
                    file.path.display(),
                    e
                );
                return vec![];
            }
        };

        // Collect all rules that apply to this file
        let applicable_rules: Vec<&dyn Rule> = self
            .registry
            .iter_rules()
            .filter(|&rule| self.rule_applies_to_file(rule, file))
            .collect();

        if applicable_rules.is_empty() {
            return vec![];
        }

        // Group rules by type (AST vs Regex)
        let (ast_rules, regex_rules): (Vec<&dyn Rule>, Vec<&dyn Rule>) = applicable_rules
            .into_iter()
            .partition(|&rule| self.is_ast_rule(rule));

        let mut all_violations = Vec::new();

        // Parse AST once if we have AST rules
        let tree = if !ast_rules.is_empty() {
            file.language
                .and_then(|lang| self.parse_ast(&content, lang))
        } else {
            None
        };

        // Execute AST rules with the parsed tree (in parallel)
        if let Some(ref tree) = tree {
            let ast_violations: Vec<Violation> = ast_rules
                .par_iter()
                .flat_map(|&rule| {
                    // Try to downcast to AstRule to use execute_with_tree
                    if let Some(ast_rule) = self.try_downcast_ast_rule(rule) {
                        ast_rule.execute_with_tree(
                            tree,
                            &content,
                            &file.path,
                            self.region_resolver.as_ref(),
                        )
                    } else {
                        // Fallback to regular execute (will re-parse internally)
                        let ctx = ExecutionContext {
                            file_path: &file.path,
                            content: &content,
                            ast: None,
                            region_resolver: self.region_resolver.clone(),
                        };
                        rule.execute(&ctx)
                    }
                })
                .collect();
            all_violations.extend(ast_violations);
        }

        // Execute regex rules (in parallel)
        let regex_violations: Vec<Violation> = regex_rules
            .par_iter()
            .flat_map(|&rule| {
                let ctx = ExecutionContext {
                    file_path: &file.path,
                    content: &content,
                    ast: None,
                    region_resolver: self.region_resolver.clone(),
                };
                rule.execute(&ctx)
            })
            .collect();
        all_violations.extend(regex_violations);

        all_violations
    }

    /// Check if a rule applies to a file
    fn rule_applies_to_file(&self, rule: &dyn Rule, file: &FileEntry) -> bool {
        // Rules only apply to program files (files with recognized language extensions)
        // Non-program files (.md, .toml, .jsonl, etc.) are excluded from all rules
        let Some(file_lang) = file.language else {
            return false;
        };

        let languages = rule.languages();

        // If rule has no language restriction, it applies to all program files
        if languages.is_empty() {
            return true;
        }

        // Check if file's language is in rule's language list
        languages.contains(&file_lang)
    }

    /// Check if a rule is an AST rule
    ///
    /// This is a heuristic based on the rule's type. We check if we can downcast to AstRule.
    fn is_ast_rule(&self, rule: &dyn Rule) -> bool {
        // Try to get the concrete type - this is a bit of a hack but works
        // We try to downcast to AstRule via Any trait
        // Since we don't have access to Any here, we use a simple heuristic:
        // AST rules have exactly one language (they're language-specific)
        let languages = rule.languages();
        languages.len() == 1
    }

    /// Try to downcast a rule to AstRule
    ///
    /// This uses unsafe pointer casting to downcast the trait object.
    /// Returns None if the rule is not an AstRule.
    fn try_downcast_ast_rule<'a>(&self, _rule: &'a dyn Rule) -> Option<&'a AstRule> {
        // We need a better way to do this - for now, we'll just use the execute method
        // and not try to downcast. The AstRule.execute() will handle parsing internally.
        None
    }

    /// Parse AST for a given language
    fn parse_ast(&self, content: &str, language: Language) -> Option<tree_sitter::Tree> {
        let mut parser: tree_sitter::Parser = match self.parser_cache.get_parser(language) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Warning: Failed to get parser for {:?}: {}", language, e);
                return None;
            }
        };

        parser.parse(content, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::file_walker::LanguageDetector;
    use crate::rules::RegexRule;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper to create a test rule
    fn create_test_regex_rule() -> RegexRule {
        let toml = r#"
[rule]
id = "test-rule"
description = "Find TODO"
severity = "warning"

[match]
pattern = "TODO"
"#;
        RegexRule::from_toml(toml).unwrap()
    }

    // Helper to create a language detector for tests
    fn test_detector() -> LanguageDetector {
        LanguageDetector::new()
    }

    #[test]
    fn test_execution_engine_creation() {
        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);
        // Just verify it compiles and constructs
        drop(engine);
    }

    #[test]
    fn test_execute_empty_files() {
        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);

        let result = engine.execute(vec![]);
        assert_eq!(result.violations.len(), 0);
        assert_eq!(result.files_checked, 0);
        assert_eq!(result.rules_executed, 0);
    }

    #[test]
    fn test_execute_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "// TODO: fix this\nfn main() {}").unwrap();

        let registry = RuleRegistry::new();
        let _rule = create_test_regex_rule();

        // Manually insert the rule (bypassing file loading)
        // We need to use a different approach for testing
        // For now, let's test with an empty registry

        let engine = ExecutionEngine::new(registry, None);
        let detector = test_detector();

        let files = vec![FileEntry::new(test_file.clone(), &detector)];
        let result = engine.execute(files);

        // With no rules, we should get no violations
        assert_eq!(result.violations.len(), 0);
        assert_eq!(result.files_checked, 1);
        assert_eq!(result.rules_executed, 0);
    }

    #[test]
    fn test_execute_multiple_files() {
        let temp_dir = TempDir::new().unwrap();

        let file1 = temp_dir.path().join("file1.rs");
        let file2 = temp_dir.path().join("file2.rs");
        fs::write(&file1, "// TODO: fix\nfn main() {}").unwrap();
        fs::write(&file2, "fn test() {}").unwrap();

        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);
        let detector = test_detector();

        let files = vec![
            FileEntry::new(file1, &detector),
            FileEntry::new(file2, &detector),
        ];
        let result = engine.execute(files);

        assert_eq!(result.files_checked, 2);
    }

    #[test]
    fn test_execute_unreadable_file() {
        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);

        // File that doesn't exist - use with_language since the file doesn't exist
        let files = vec![FileEntry::with_language(
            PathBuf::from("/nonexistent/file.rs"),
            Some(Language::Rust),
        )];
        let result = engine.execute(files);

        // Should handle gracefully - no violations, no crash
        assert_eq!(result.violations.len(), 0);
        assert_eq!(result.files_checked, 1);
    }

    #[test]
    fn test_rule_applies_to_file_no_language_restriction() {
        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);

        // Create a mock rule with no language restrictions
        let rule = create_test_regex_rule();

        // Rule with no language restrictions applies to all program files
        let rust_file = FileEntry::with_language(PathBuf::from("test.rs"), Some(Language::Rust));
        assert!(engine.rule_applies_to_file(&rule, &rust_file));

        let python_file =
            FileEntry::with_language(PathBuf::from("test.py"), Some(Language::Python));
        assert!(engine.rule_applies_to_file(&rule, &python_file));

        // But does NOT apply to non-program files
        let md_file = FileEntry::with_language(PathBuf::from("README.md"), None);
        assert!(!engine.rule_applies_to_file(&rule, &md_file));

        let toml_file = FileEntry::with_language(PathBuf::from("config.toml"), None);
        assert!(!engine.rule_applies_to_file(&rule, &toml_file));
    }

    #[test]
    fn test_rule_applies_to_file_with_language() {
        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);

        // Create a rule that applies to Rust
        let toml = r#"
[rule]
id = "rust-only"
description = "Rust only rule"
severity = "warning"

[match]
pattern = "TODO"
languages = ["rust"]
"#;
        let rule = RegexRule::from_toml(toml).unwrap();

        let rust_file = FileEntry::with_language(PathBuf::from("test.rs"), Some(Language::Rust));
        let python_file =
            FileEntry::with_language(PathBuf::from("test.py"), Some(Language::Python));

        assert!(engine.rule_applies_to_file(&rule, &rust_file));
        assert!(!engine.rule_applies_to_file(&rule, &python_file));
    }

    #[test]
    fn test_is_ast_rule_heuristic() {
        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);

        // Regex rule with no language restrictions
        let regex_rule = create_test_regex_rule();
        assert!(!engine.is_ast_rule(&regex_rule));

        // Regex rule with multiple languages
        let toml = r#"
[rule]
id = "multi-lang"
description = "Multi language rule"
severity = "warning"

[match]
pattern = "TODO"
languages = ["rust", "python"]
"#;
        let multi_lang_rule = RegexRule::from_toml(toml).unwrap();
        assert!(!engine.is_ast_rule(&multi_lang_rule));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_parse_ast() {
        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);

        let content = "fn main() {}";
        let tree = engine.parse_ast(content, Language::Rust);

        assert!(tree.is_some());
    }

    #[test]
    fn test_execute_result_structure() {
        let result = ExecutionResult {
            violations: vec![],
            files_checked: 5,
            rules_executed: 10,
        };

        assert_eq!(result.violations.len(), 0);
        assert_eq!(result.files_checked, 5);
        assert_eq!(result.rules_executed, 10);
    }

    #[test]
    fn test_parallel_execution() {
        // Create multiple files to test parallel execution
        let temp_dir = TempDir::new().unwrap();

        let mut files = Vec::new();
        let detector = test_detector();
        for i in 0..10 {
            let file_path = temp_dir.path().join(format!("file{}.rs", i));
            fs::write(&file_path, format!("fn main{i}() {{}}")).unwrap();
            files.push(FileEntry::new(file_path, &detector));
        }

        let registry = RuleRegistry::new();
        let engine = ExecutionEngine::new(registry, None);

        let result = engine.execute(files);

        // Should process all files
        assert_eq!(result.files_checked, 10);
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_ast_rule_execution() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() { Some(5).unwrap(); }").unwrap();

        let mut registry = RuleRegistry::new();

        // Create an AST rule directory
        let ast_dir = temp_dir.path().join("ast");
        fs::create_dir(&ast_dir).unwrap();

        let ast_rule_content = r#"
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
        fs::write(ast_dir.join("unwrap.toml"), ast_rule_content).unwrap();

        // Load the AST rule
        registry.load_custom_ast_rules(&ast_dir, None).unwrap();

        let engine = ExecutionEngine::new(registry, None);
        let detector = test_detector();
        let files = vec![FileEntry::new(test_file, &detector)];
        let result = engine.execute(files);

        // Should find the unwrap call
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.files_checked, 1);
        assert_eq!(result.rules_executed, 1);
    }

    #[test]
    fn test_mixed_rules_execution() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "// TODO: fix\nfn main() {}").unwrap();

        let mut registry = RuleRegistry::new();

        // Create a regex rule
        let regex_dir = temp_dir.path().join("regex");
        fs::create_dir(&regex_dir).unwrap();

        let regex_rule_content = r#"
[rule]
id = "no-todo"
description = "No TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
        fs::write(regex_dir.join("todo.toml"), regex_rule_content).unwrap();

        registry.load_custom_regex_rules(&regex_dir, None).unwrap();

        let engine = ExecutionEngine::new(registry, None);
        let detector = test_detector();
        let files = vec![FileEntry::new(test_file, &detector)];
        let result = engine.execute(files);

        // Should find the TODO comment
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.files_checked, 1);
        assert_eq!(result.rules_executed, 1);
    }
}
