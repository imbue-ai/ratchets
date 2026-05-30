//! Shared helpers for sculptor group AST rule validation tests.
//!
//! Each `tests/sculptor_group_*_tests.rs` integration-test binary includes
//! this module via `mod sculptor_common;` to avoid duplicating the
//! `AstRule` loading and matching scaffolding.

#![cfg(feature = "lang-python")]

use ratchets::rules::{AstRule, ExecutionContext, Rule};
use std::path::Path;

pub fn load_rule(name: &str) -> AstRule {
    let path = format!("builtin-ratchets/python/ast/{}.toml", name);
    AstRule::from_path(Path::new(&path))
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
