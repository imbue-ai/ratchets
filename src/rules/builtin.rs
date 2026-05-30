#![forbid(unsafe_code)]

//! Built-in rules embedded at compile time
//!
//! This module provides access to built-in rules that are compiled into the binary
//! using `include_str!`. This ensures the binary is self-contained and can run
//! without external rule files.

use crate::error::RuleError;
use crate::rules::{AstRule, RegexRule, Rule};
use crate::types::RuleId;

/// Type alias for a list of rules with their IDs
type RuleList = Vec<(RuleId, Box<dyn Rule>)>;

/// Embedded built-in regex rule files
const BUILTIN_REGEX_RULES: &[(&str, &str)] = &[
    (
        "no-todo-comments",
        include_str!("../../builtin-ratchets/common/regex/no-todo-comments.toml"),
    ),
    (
        "no-fixme-comments",
        include_str!("../../builtin-ratchets/common/regex/no-fixme-comments.toml"),
    ),
];

/// Embedded built-in regex rule files for Python
#[cfg(feature = "lang-python")]
const BUILTIN_PYTHON_REGEX_RULES: &[(&str, &str)] = &[
    (
        "no-inline-imports",
        include_str!("../../builtin-ratchets/python/regex/no-inline-imports.toml"),
    ),
    (
        "no-relative-imports",
        include_str!("../../builtin-ratchets/python/regex/no-relative-imports.toml"),
    ),
    (
        "no-import-datetime",
        include_str!("../../builtin-ratchets/python/regex/no-import-datetime.toml"),
    ),
    (
        "no-asyncio-import",
        include_str!("../../builtin-ratchets/python/regex/no-asyncio-import.toml"),
    ),
    (
        "no-pandas-import",
        include_str!("../../builtin-ratchets/python/regex/no-pandas-import.toml"),
    ),
    (
        "no-dataclasses-import",
        include_str!("../../builtin-ratchets/python/regex/no-dataclasses-import.toml"),
    ),
    (
        "no-yaml-usage",
        include_str!("../../builtin-ratchets/python/regex/no-yaml-usage.toml"),
    ),
    (
        "no-namedtuple-usage",
        include_str!("../../builtin-ratchets/python/regex/no-namedtuple-usage.toml"),
    ),
    (
        "no-time-sleep",
        include_str!("../../builtin-ratchets/python/regex/no-time-sleep.toml"),
    ),
    (
        "no-click-echo",
        include_str!("../../builtin-ratchets/python/regex/no-click-echo.toml"),
    ),
    (
        "no-bare-generic-types",
        include_str!("../../builtin-ratchets/python/regex/no-bare-generic-types.toml"),
    ),
    (
        "no-typing-builtin-imports",
        include_str!("../../builtin-ratchets/python/regex/no-typing-builtin-imports.toml"),
    ),
    (
        "no-literal-multi-options",
        include_str!("../../builtin-ratchets/python/regex/no-literal-multi-options.toml"),
    ),
    (
        "no-init-docstrings",
        include_str!("../../builtin-ratchets/python/regex/no-init-docstrings.toml"),
    ),
    (
        "no-args-in-docstrings",
        include_str!("../../builtin-ratchets/python/regex/no-args-in-docstrings.toml"),
    ),
    (
        "no-returns-in-docstrings",
        include_str!("../../builtin-ratchets/python/regex/no-returns-in-docstrings.toml"),
    ),
    (
        "no-trailing-comments",
        include_str!("../../builtin-ratchets/python/regex/no-trailing-comments.toml"),
    ),
    (
        "no-num-prefix",
        include_str!("../../builtin-ratchets/python/regex/no-num-prefix.toml"),
    ),
    (
        "no-builtin-exception-raises",
        include_str!("../../builtin-ratchets/python/regex/no-builtin-exception-raises.toml"),
    ),
    (
        "no-fstring-logging",
        include_str!("../../builtin-ratchets/python/regex/no-fstring-logging.toml"),
    ),
    // Group A — ports of sculptor's plain-regex ratchet rules
    (
        "no-pytorch-lightning",
        include_str!("../../builtin-ratchets/python/regex/no-pytorch-lightning.toml"),
    ),
    (
        "no-logger-warning",
        include_str!("../../builtin-ratchets/python/regex/no-logger-warning.toml"),
    ),
    (
        "no-quarantine-import",
        include_str!("../../builtin-ratchets/python/regex/no-quarantine-import.toml"),
    ),
    (
        "no-quarantine-paths",
        include_str!("../../builtin-ratchets/python/regex/no-quarantine-paths.toml"),
    ),
    (
        "no-walrus-operator",
        include_str!("../../builtin-ratchets/python/regex/no-walrus-operator.toml"),
    ),
    (
        "no-ssh-subprocess",
        include_str!("../../builtin-ratchets/python/regex/no-ssh-subprocess.toml"),
    ),
    (
        "no-os-path-join",
        include_str!("../../builtin-ratchets/python/regex/no-os-path-join.toml"),
    ),
    (
        "no-todo-remove-comment",
        include_str!("../../builtin-ratchets/python/regex/no-todo-remove-comment.toml"),
    ),
    (
        "no-implicit-string-concat",
        include_str!("../../builtin-ratchets/python/regex/no-implicit-string-concat.toml"),
    ),
    (
        "no-builtin-hash",
        include_str!("../../builtin-ratchets/python/regex/no-builtin-hash.toml"),
    ),
    (
        "no-make-composite-seed",
        include_str!("../../builtin-ratchets/python/regex/no-make-composite-seed.toml"),
    ),
    (
        "no-numpy-default-rng",
        include_str!("../../builtin-ratchets/python/regex/no-numpy-default-rng.toml"),
    ),
    (
        "no-asyncio-run",
        include_str!("../../builtin-ratchets/python/regex/no-asyncio-run.toml"),
    ),
    (
        "no-logger-exception",
        include_str!("../../builtin-ratchets/python/regex/no-logger-exception.toml"),
    ),
    (
        "no-pydantic-model-copy-update",
        include_str!("../../builtin-ratchets/python/regex/no-pydantic-model-copy-update.toml"),
    ),
    (
        "no-tree-sitter-text-decode",
        include_str!("../../builtin-ratchets/python/regex/no-tree-sitter-text-decode.toml"),
    ),
    (
        "no-byte-index-source",
        include_str!("../../builtin-ratchets/python/regex/no-byte-index-source.toml"),
    ),
    (
        "no-mypy-ignore-errors",
        include_str!("../../builtin-ratchets/python/regex/no-mypy-ignore-errors.toml"),
    ),
    (
        "no-pyre-ignore",
        include_str!("../../builtin-ratchets/python/regex/no-pyre-ignore.toml"),
    ),
    (
        "no-pyre-fixme",
        include_str!("../../builtin-ratchets/python/regex/no-pyre-fixme.toml"),
    ),
    (
        "no-type-ignore",
        include_str!("../../builtin-ratchets/python/regex/no-type-ignore.toml"),
    ),
    // Group B — ports of sculptor's path-scoped regex ratchet rules
    (
        "no-sculptor-copytree",
        include_str!("../../builtin-ratchets/python/regex/no-sculptor-copytree.toml"),
    ),
    (
        "no-sculptor-subprocess",
        include_str!("../../builtin-ratchets/python/regex/no-sculptor-subprocess.toml"),
    ),
    (
        "no-integration-page-reload",
        include_str!("../../builtin-ratchets/python/regex/no-integration-page-reload.toml"),
    ),
    (
        "no-integration-non-testid-queries",
        include_str!("../../builtin-ratchets/python/regex/no-integration-non-testid-queries.toml"),
    ),
    (
        "no-integration-css-locators",
        include_str!("../../builtin-ratchets/python/regex/no-integration-css-locators.toml"),
    ),
    (
        "no-integration-type-method",
        include_str!("../../builtin-ratchets/python/regex/no-integration-type-method.toml"),
    ),
    (
        "no-integration-page-goto",
        include_str!("../../builtin-ratchets/python/regex/no-integration-page-goto.toml"),
    ),
    (
        "no-integration-page-evaluate",
        include_str!("../../builtin-ratchets/python/regex/no-integration-page-evaluate.toml"),
    ),
    (
        "no-integration-time-sleep",
        include_str!("../../builtin-ratchets/python/regex/no-integration-time-sleep.toml"),
    ),
];

/// Embedded built-in regex rule files for TypeScript
#[cfg(feature = "lang-typescript")]
const BUILTIN_TYPESCRIPT_REGEX_RULES: &[(&str, &str)] = &[
    // Group B — ports of sculptor's path-scoped regex ratchet rules
    (
        "no-raw-html-button",
        include_str!("../../builtin-ratchets/typescript/regex/no-raw-html-button.toml"),
    ),
];

/// Embedded built-in AST rule files for Rust
#[cfg(feature = "lang-rust")]
const BUILTIN_AST_RUST_RULES: &[(&str, &str)] = &[
    (
        "no-unwrap",
        include_str!("../../builtin-ratchets/rust/ast/no-unwrap.toml"),
    ),
    (
        "no-panic",
        include_str!("../../builtin-ratchets/rust/ast/no-panic.toml"),
    ),
    (
        "no-expect",
        include_str!("../../builtin-ratchets/rust/ast/no-expect.toml"),
    ),
    (
        "rust-no-todo-comments",
        include_str!("../../builtin-ratchets/rust/ast/no-todo-comments.toml"),
    ),
    (
        "rust-no-fixme-comments",
        include_str!("../../builtin-ratchets/rust/ast/no-fixme-comments.toml"),
    ),
];

/// Embedded built-in AST rule files for Python
#[cfg(feature = "lang-python")]
const BUILTIN_AST_PYTHON_RULES: &[(&str, &str)] = &[
    (
        "no-bare-except",
        include_str!("../../builtin-ratchets/python/ast/no-bare-except.toml"),
    ),
    (
        "no-if-elif-without-else",
        include_str!("../../builtin-ratchets/python/ast/no-if-elif-without-else.toml"),
    ),
    (
        "no-inline-functions",
        include_str!("../../builtin-ratchets/python/ast/no-inline-functions.toml"),
    ),
    (
        "no-underscore-imports",
        include_str!("../../builtin-ratchets/python/ast/no-underscore-imports.toml"),
    ),
    (
        "no-init-in-non-exception-classes",
        include_str!("../../builtin-ratchets/python/ast/no-init-in-non-exception-classes.toml"),
    ),
    (
        "no-base-exception",
        include_str!("../../builtin-ratchets/python/ast/no-base-exception.toml"),
    ),
    (
        "no-broad-exception",
        include_str!("../../builtin-ratchets/python/ast/no-broad-exception.toml"),
    ),
    (
        "no-eval-usage",
        include_str!("../../builtin-ratchets/python/ast/no-eval-usage.toml"),
    ),
    (
        "no-exec-usage",
        include_str!("../../builtin-ratchets/python/ast/no-exec-usage.toml"),
    ),
    (
        "no-while-true",
        include_str!("../../builtin-ratchets/python/ast/no-while-true.toml"),
    ),
    (
        "no-global-keyword",
        include_str!("../../builtin-ratchets/python/ast/no-global-keyword.toml"),
    ),
    (
        "no-bare-print",
        include_str!("../../builtin-ratchets/python/ast/no-bare-print.toml"),
    ),
    (
        "python-no-todo-comments",
        include_str!("../../builtin-ratchets/python/ast/no-todo-comments.toml"),
    ),
    (
        "python-no-fixme-comments",
        include_str!("../../builtin-ratchets/python/ast/no-fixme-comments.toml"),
    ),
    // Group C — ports of sculptor's lookaround-based ratchet rules as tree-sitter queries
    (
        "no-bare-exit",
        include_str!("../../builtin-ratchets/python/ast/no-bare-exit.toml"),
    ),
    (
        "no-typing-cast",
        include_str!("../../builtin-ratchets/python/ast/no-typing-cast.toml"),
    ),
    (
        "no-unnumbered-pyre-ignore",
        include_str!("../../builtin-ratchets/python/ast/no-unnumbered-pyre-ignore.toml"),
    ),
    (
        "no-unnumbered-pyre-fixme",
        include_str!("../../builtin-ratchets/python/ast/no-unnumbered-pyre-fixme.toml"),
    ),
    (
        "no-unlabeled-type-ignore",
        include_str!("../../builtin-ratchets/python/ast/no-unlabeled-type-ignore.toml"),
    ),
    (
        "no-untyped-args-kwargs",
        include_str!("../../builtin-ratchets/python/ast/no-untyped-args-kwargs.toml"),
    ),
    (
        "classmethod-builder-naming",
        include_str!("../../builtin-ratchets/python/ast/classmethod-builder-naming.toml"),
    ),
    (
        "staticmethod-private-only",
        include_str!("../../builtin-ratchets/python/ast/staticmethod-private-only.toml"),
    ),
    (
        "attrs-decorator",
        include_str!("../../builtin-ratchets/python/ast/attrs-decorator.toml"),
    ),
    (
        "no-mutable-attr-in-frozen-dataclass",
        include_str!("../../builtin-ratchets/python/ast/no-mutable-attr-in-frozen-dataclass.toml"),
    ),
    // Group D — port of sculptor's bespoke match-exhaustiveness ratchet rule
    (
        "match-must-assert-never",
        include_str!("../../builtin-ratchets/python/ast/match-must-assert-never.toml"),
    ),
];

/// Embedded built-in AST rule files for TypeScript
#[cfg(feature = "lang-typescript")]
const BUILTIN_AST_TYPESCRIPT_RULES: &[(&str, &str)] = &[(
    "no-any",
    include_str!("../../builtin-ratchets/typescript/ast/no-any.toml"),
)];

/// Parse `source` as embedded regex rules and append them to `rules`. `label`
/// is interpolated into the parse-error message verbatim.
fn extend_regex_rules(
    rules: &mut RuleList,
    source: &[(&str, &str)],
    label: &str,
) -> Result<(), RuleError> {
    for (rule_name, toml_content) in source {
        let rule = RegexRule::from_toml(toml_content).map_err(|e| {
            RuleError::InvalidDefinition(format!(
                "Failed to parse built-in {} rule '{}': {}",
                label, rule_name, e
            ))
        })?;
        let rule_id = rule.id().clone();
        rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
    }
    Ok(())
}

/// Load all built-in regex rules from embedded resources
///
/// Returns a vector of tuples containing (rule_id, boxed rule).
///
/// # Errors
///
/// Returns `RuleError` if:
/// - A TOML file cannot be parsed
/// - A rule definition is invalid
pub fn load_builtin_regex_rules() -> Result<RuleList, RuleError> {
    let mut rules = Vec::new();

    extend_regex_rules(&mut rules, BUILTIN_REGEX_RULES, "regex")?;

    #[cfg(feature = "lang-python")]
    extend_regex_rules(&mut rules, BUILTIN_PYTHON_REGEX_RULES, "Python regex")?;

    #[cfg(feature = "lang-typescript")]
    extend_regex_rules(
        &mut rules,
        BUILTIN_TYPESCRIPT_REGEX_RULES,
        "TypeScript regex",
    )?;

    Ok(rules)
}

/// Parse `source` as embedded AST rules under `rule_context` and append them
/// to `rules`. `label` is interpolated into the parse-error message verbatim.
fn extend_ast_rules(
    rules: &mut RuleList,
    source: &[(&str, &str)],
    rule_context: &crate::rules::RuleContext,
    label: &str,
) -> Result<(), RuleError> {
    for (rule_name, toml_content) in source {
        let rule =
            AstRule::from_toml_with_context(toml_content, Some(rule_context)).map_err(|e| {
                RuleError::InvalidDefinition(format!(
                    "Failed to parse built-in {} rule '{}': {}",
                    label, rule_name, e
                ))
            })?;
        let rule_id = rule.id().clone();
        rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
    }
    Ok(())
}

/// Load all built-in AST rules from embedded resources
///
/// Returns a vector of tuples containing (rule_id, boxed rule).
///
/// # Errors
///
/// Returns `RuleError` if:
/// - A TOML file cannot be parsed
/// - A rule definition is invalid
/// - A tree-sitter query is invalid
pub fn load_builtin_ast_rules() -> Result<RuleList, RuleError> {
    use crate::rules::RuleContext;
    use crate::types::GlobPattern;
    use std::collections::HashMap;

    let mut rules = Vec::new();

    // Create a default pattern context for builtin rules
    let mut patterns = HashMap::new();

    // Define python_tests pattern for Python AST rules
    #[cfg(feature = "lang-python")]
    {
        patterns.insert(
            "python_tests".to_string(),
            vec![
                GlobPattern::new("**/test_*.py".to_string()),
                GlobPattern::new("**/*_test.py".to_string()),
                GlobPattern::new("**/tests/**".to_string()),
            ],
        );
    }

    let rule_context = RuleContext { patterns };

    #[cfg(feature = "lang-rust")]
    extend_ast_rules(
        &mut rules,
        BUILTIN_AST_RUST_RULES,
        &rule_context,
        "Rust AST",
    )?;

    #[cfg(feature = "lang-python")]
    extend_ast_rules(
        &mut rules,
        BUILTIN_AST_PYTHON_RULES,
        &rule_context,
        "Python AST",
    )?;

    #[cfg(feature = "lang-typescript")]
    extend_ast_rules(
        &mut rules,
        BUILTIN_AST_TYPESCRIPT_RULES,
        &rule_context,
        "TypeScript AST",
    )?;

    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_builtin_regex_rules() {
        let result = load_builtin_regex_rules();
        assert!(result.is_ok());

        let rules = result.unwrap();

        // Derive the expected count from the same constants the loader iterates.
        // This keeps the assertion in sync as new TOMLs are registered, while
        // still catching cases where a rule is registered but fails to load.
        let mut expected = BUILTIN_REGEX_RULES.len();
        #[cfg(feature = "lang-python")]
        {
            expected += BUILTIN_PYTHON_REGEX_RULES.len();
        }
        #[cfg(feature = "lang-typescript")]
        {
            expected += BUILTIN_TYPESCRIPT_REGEX_RULES.len();
        }
        assert_eq!(rules.len(), expected);

        // Check that rule IDs are correct
        let rule_ids: Vec<&str> = rules.iter().map(|(id, _)| id.as_str()).collect();
        assert!(rule_ids.contains(&"no-todo-comments"));
        assert!(rule_ids.contains(&"no-fixme-comments"));

        // Verify Python rules are present when lang-python feature is enabled
        #[cfg(feature = "lang-python")]
        {
            assert!(rule_ids.contains(&"no-fstring-logging"));
            // no-broad-exception, no-base-exception, no-eval-usage, and no-exec-usage moved to AST rules
            assert!(!rule_ids.contains(&"no-broad-exception"));
            assert!(!rule_ids.contains(&"no-base-exception"));
            assert!(!rule_ids.contains(&"no-eval-usage"));
            assert!(!rule_ids.contains(&"no-exec-usage"));
        }

        // Verify TypeScript regex rules are present when lang-typescript feature is enabled
        #[cfg(feature = "lang-typescript")]
        {
            assert!(rule_ids.contains(&"no-raw-html-button"));
        }
    }

    #[test]
    fn test_load_builtin_ast_rules() {
        let result = load_builtin_ast_rules();
        assert!(result.is_ok());

        let rules = result.unwrap();

        // Derive the expected count from the same constants the loader iterates.
        // This keeps the assertion in sync as new TOMLs are registered, while
        // still catching cases where a rule is registered but fails to load.
        let mut expected = 0;
        #[cfg(feature = "lang-rust")]
        {
            expected += BUILTIN_AST_RUST_RULES.len();
        }
        #[cfg(feature = "lang-python")]
        {
            expected += BUILTIN_AST_PYTHON_RULES.len();
        }
        #[cfg(feature = "lang-typescript")]
        {
            expected += BUILTIN_AST_TYPESCRIPT_RULES.len();
        }
        assert_eq!(rules.len(), expected);

        // Verify Rust rules are present when lang-rust feature is enabled
        #[cfg(feature = "lang-rust")]
        {
            let rule_ids: Vec<&str> = rules.iter().map(|(id, _)| id.as_str()).collect();
            assert!(rule_ids.contains(&"no-unwrap"));
            assert!(rule_ids.contains(&"no-panic"));
            assert!(rule_ids.contains(&"no-expect"));
            assert!(rule_ids.contains(&"rust-no-todo-comments"));
            assert!(rule_ids.contains(&"rust-no-fixme-comments"));
        }

        // Verify Python rules are present when lang-python feature is enabled
        #[cfg(feature = "lang-python")]
        {
            let rule_ids: Vec<&str> = rules.iter().map(|(id, _)| id.as_str()).collect();
            assert!(rule_ids.contains(&"no-bare-except"));
            assert!(rule_ids.contains(&"no-if-elif-without-else"));
            assert!(rule_ids.contains(&"no-inline-functions"));
            assert!(rule_ids.contains(&"no-underscore-imports"));
            assert!(rule_ids.contains(&"no-init-in-non-exception-classes"));
            assert!(rule_ids.contains(&"no-base-exception"));
            assert!(rule_ids.contains(&"no-broad-exception"));
            assert!(rule_ids.contains(&"no-eval-usage"));
            assert!(rule_ids.contains(&"no-exec-usage"));
            assert!(rule_ids.contains(&"no-while-true"));
            assert!(rule_ids.contains(&"no-global-keyword"));
            assert!(rule_ids.contains(&"no-bare-print"));
            assert!(rule_ids.contains(&"python-no-todo-comments"));
            assert!(rule_ids.contains(&"python-no-fixme-comments"));
        }

        // Verify TypeScript rules are present when lang-typescript feature is enabled
        #[cfg(feature = "lang-typescript")]
        {
            let rule_ids: Vec<&str> = rules.iter().map(|(id, _)| id.as_str()).collect();
            assert!(rule_ids.contains(&"no-any"));
        }
    }

    #[test]
    fn test_builtin_regex_rules_are_valid() {
        let rules = load_builtin_regex_rules().unwrap();

        // Verify each rule has a valid ID and can be accessed
        for (rule_id, rule) in rules {
            assert_eq!(rule.id(), &rule_id);
            assert!(!rule.description().is_empty());
        }
    }

    #[test]
    fn test_builtin_ast_rules_are_valid() {
        let rules = load_builtin_ast_rules().unwrap();

        // Verify each rule has a valid ID and can be accessed
        for (rule_id, rule) in rules {
            assert_eq!(rule.id(), &rule_id);
            assert!(!rule.description().is_empty());
        }
    }
}
