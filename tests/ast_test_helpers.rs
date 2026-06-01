//! Shared helpers for Python AST builtin rule validation tests.
//!
//! Each Python AST rule validation integration-test binary includes this
//! module via `mod ast_test_helpers;` to avoid duplicating the `AstRule`
//! loading and matching scaffolding.

#![cfg(feature = "lang-python")]

use ratchets::GlobPattern;
use ratchets::rules::{AstRule, ExecutionContext, Rule, RuleContext};
use std::collections::HashMap;
use std::path::Path;

pub fn load_rule(name: &str) -> AstRule {
    let path = format!("builtin-ratchets/python/ast/{}.toml", name);
    AstRule::from_path(Path::new(&path))
        .unwrap_or_else(|e| panic!("Failed to load rule {}: {}", path, e))
}

/// Load an AST rule with the same `@python_tests` pattern context that the
/// production registry provides. Use this for rules whose TOML references
/// `@python_tests` in `include` or `exclude`.
#[allow(dead_code)]
pub fn load_rule_with_python_tests(name: &str) -> AstRule {
    let path = format!("builtin-ratchets/python/ast/{}.toml", name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read rule {}: {}", path, e));

    let mut patterns = HashMap::new();
    patterns.insert(
        "python_tests".to_string(),
        vec![
            GlobPattern::new("**/test_*.py".to_string()),
            GlobPattern::new("**/*_test.py".to_string()),
            GlobPattern::new("**/tests/**".to_string()),
        ],
    );
    let ctx = RuleContext::new(patterns);
    AstRule::from_toml_with_context(&content, Some(&ctx))
        .unwrap_or_else(|e| panic!("Failed to load rule {}: {}", path, e))
}

pub fn matches(rule: &AstRule, src: &str) -> usize {
    let ctx = ExecutionContext {
        file_path: Path::new("t.py"),
        content: src,
        ast: None,
        region_resolver: None,
    };
    rule.execute(&ctx).len()
}

pub fn expect_match(rule: &AstRule, src: &str, label: &str) {
    let n = matches(rule, src);
    assert!(
        n > 0,
        "[{}] expected match for: {:?}, got {} violations",
        label,
        src,
        n
    );
}

pub fn expect_no_match(rule: &AstRule, src: &str, label: &str) {
    let n = matches(rule, src);
    assert_eq!(
        n, 0,
        "[{}] expected NO match for: {:?}, got {} violations",
        label, src, n
    );
}
