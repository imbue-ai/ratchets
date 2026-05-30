//! Validation tests for Group D sculptor rules (bespoke match-exhaustiveness).
//!
//! Sculptor's `MatchCaseRatchetTest` enforces that every Python `match` block
//! ends with `case _ as <var>: assert_never(<var>)`. With tree-sitter the rule
//! collapses to a query on `match_statement` plus a `#not-match?` predicate.
//! These tests cover the match_examples / non_match_examples from sculptor's
//! ratchet_rules.py for the `match_without_wildcard_or_assert_never` rule.

#![cfg(feature = "lang-python")]

use ratchets::rules::{AstRule, ExecutionContext, Rule};
use std::path::Path;

fn load_rule(name: &str) -> AstRule {
    let path = format!("builtin-ratchets/python/ast/{}.toml", name);
    AstRule::from_path(Path::new(&path))
        .unwrap_or_else(|e| panic!("Failed to load rule {}: {}", path, e))
}

fn matches(rule: &AstRule, src: &str) -> usize {
    let ctx = ExecutionContext {
        file_path: Path::new("t.py"),
        content: src,
        ast: None,
        region_resolver: None,
    };
    rule.execute(&ctx).len()
}

fn expect_match(rule: &AstRule, src: &str, label: &str) {
    let n = matches(rule, src);
    assert!(
        n > 0,
        "[{}] expected match for: {:?}, got {} violations",
        label,
        src,
        n
    );
}

fn expect_no_match(rule: &AstRule, src: &str, label: &str) {
    let n = matches(rule, src);
    assert_eq!(
        n, 0,
        "[{}] expected NO match for: {:?}, got {} violations",
        label, src, n
    );
}

// --------------------------------------------------------------------------
// match-must-assert-never (sculptor: match_without_wildcard_or_assert_never)
// --------------------------------------------------------------------------

#[test]
fn match_must_assert_never_match_examples() {
    let rule = load_rule("match-must-assert-never");
    // From sculptor's match_examples:
    expect_match(
        &rule,
        "match value:\n    case Type1():\n        pass\n    case Type2():\n        pass\n",
        "two cases no wildcard",
    );
    expect_match(
        &rule,
        "match x:\n    case 1:\n        do_one()\n    case 2:\n        do_two()\n",
        "literal cases no wildcard",
    );
    expect_match(
        &rule,
        "match value:\n    case Type1():\n        pass\n    case _:\n        pass\n",
        "wildcard without assert_never",
    );
}

#[test]
fn match_must_assert_never_non_match_examples() {
    let rule = load_rule("match-must-assert-never");
    // From sculptor's non_match_examples — the only active one:
    expect_no_match(
        &rule,
        "match value:\n    case Type1():\n        pass\n    case _ as unreachable:\n        assert_never(unreachable)\n",
        "wildcard with assert_never",
    );
}
