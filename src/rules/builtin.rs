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
];

/// Embedded built-in AST rule files for TypeScript
#[cfg(feature = "lang-typescript")]
const BUILTIN_AST_TYPESCRIPT_RULES: &[(&str, &str)] = &[(
    "no-any",
    include_str!("../../builtin-ratchets/typescript/ast/no-any.toml"),
)];

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

    // Load common regex rules
    for (rule_name, toml_content) in BUILTIN_REGEX_RULES {
        let rule = RegexRule::from_toml(toml_content).map_err(|e| {
            RuleError::InvalidDefinition(format!(
                "Failed to parse built-in regex rule '{}': {}",
                rule_name, e
            ))
        })?;

        let rule_id = rule.id().clone();
        rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
    }

    // Load Python regex rules
    #[cfg(feature = "lang-python")]
    {
        for (rule_name, toml_content) in BUILTIN_PYTHON_REGEX_RULES {
            let rule = RegexRule::from_toml(toml_content).map_err(|e| {
                RuleError::InvalidDefinition(format!(
                    "Failed to parse built-in Python regex rule '{}': {}",
                    rule_name, e
                ))
            })?;

            let rule_id = rule.id().clone();
            rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
        }
    }

    Ok(rules)
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

    // Load Rust AST rules
    #[cfg(feature = "lang-rust")]
    {
        for (rule_name, toml_content) in BUILTIN_AST_RUST_RULES {
            let rule = AstRule::from_toml_with_context(toml_content, Some(&rule_context)).map_err(
                |e| {
                    RuleError::InvalidDefinition(format!(
                        "Failed to parse built-in Rust AST rule '{}': {}",
                        rule_name, e
                    ))
                },
            )?;

            let rule_id = rule.id().clone();
            rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
        }
    }

    // Load Python AST rules
    #[cfg(feature = "lang-python")]
    {
        for (rule_name, toml_content) in BUILTIN_AST_PYTHON_RULES {
            let rule = AstRule::from_toml_with_context(toml_content, Some(&rule_context)).map_err(
                |e| {
                    RuleError::InvalidDefinition(format!(
                        "Failed to parse built-in Python AST rule '{}': {}",
                        rule_name, e
                    ))
                },
            )?;

            let rule_id = rule.id().clone();
            rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
        }
    }

    // Load TypeScript AST rules
    #[cfg(feature = "lang-typescript")]
    {
        for (rule_name, toml_content) in BUILTIN_AST_TYPESCRIPT_RULES {
            let rule = AstRule::from_toml_with_context(toml_content, Some(&rule_context)).map_err(
                |e| {
                    RuleError::InvalidDefinition(format!(
                        "Failed to parse built-in TypeScript AST rule '{}': {}",
                        rule_name, e
                    ))
                },
            )?;

            let rule_id = rule.id().clone();
            rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
        }
    }

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

        // The number of rules depends on which language features are enabled
        #[cfg(not(feature = "lang-python"))]
        assert_eq!(rules.len(), 2); // no-todo-comments and no-fixme-comments

        #[cfg(feature = "lang-python")]
        assert_eq!(rules.len(), 22); // 2 common + 20 Python regex rules

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
    }

    #[test]
    fn test_load_builtin_ast_rules() {
        let result = load_builtin_ast_rules();
        assert!(result.is_ok());

        let rules = result.unwrap();

        // The number of rules depends on which language features are enabled
        #[cfg(feature = "lang-rust")]
        assert!(rules.len() >= 3); // At least the 3 Rust rules

        #[cfg(all(
            feature = "lang-rust",
            not(feature = "lang-python"),
            not(feature = "lang-typescript")
        ))]
        assert_eq!(rules.len(), 3); // Exactly 3 if only Rust is enabled

        // Verify Rust rules are present when lang-rust feature is enabled
        #[cfg(feature = "lang-rust")]
        {
            let rule_ids: Vec<&str> = rules.iter().map(|(id, _)| id.as_str()).collect();
            assert!(rule_ids.contains(&"no-unwrap"));
            assert!(rule_ids.contains(&"no-panic"));
            assert!(rule_ids.contains(&"no-expect"));
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
