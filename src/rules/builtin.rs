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
        include_str!("../../builtin-ratchets/regex/no-todo-comments.toml"),
    ),
    (
        "no-fixme-comments",
        include_str!("../../builtin-ratchets/regex/no-fixme-comments.toml"),
    ),
];

/// Embedded built-in AST rule files for Rust
#[cfg(feature = "lang-rust")]
const BUILTIN_AST_RUST_RULES: &[(&str, &str)] = &[
    (
        "no-unwrap",
        include_str!("../../builtin-ratchets/ast/rust/no-unwrap.toml"),
    ),
    (
        "no-panic",
        include_str!("../../builtin-ratchets/ast/rust/no-panic.toml"),
    ),
    (
        "no-expect",
        include_str!("../../builtin-ratchets/ast/rust/no-expect.toml"),
    ),
];

/// Embedded built-in AST rule files for Python
#[cfg(feature = "lang-python")]
const BUILTIN_AST_PYTHON_RULES: &[(&str, &str)] = &[(
    "no-bare-except",
    include_str!("../../builtin-ratchets/ast/python/no-bare-except.toml"),
)];

/// Embedded built-in AST rule files for TypeScript
#[cfg(feature = "lang-typescript")]
const BUILTIN_AST_TYPESCRIPT_RULES: &[(&str, &str)] = &[(
    "no-any",
    include_str!("../../builtin-ratchets/ast/typescript/no-any.toml"),
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
    let mut rules = Vec::new();

    // Load Rust AST rules
    #[cfg(feature = "lang-rust")]
    {
        for (rule_name, toml_content) in BUILTIN_AST_RUST_RULES {
            let rule = AstRule::from_toml(toml_content).map_err(|e| {
                RuleError::InvalidDefinition(format!(
                    "Failed to parse built-in Rust AST rule '{}': {}",
                    rule_name, e
                ))
            })?;

            let rule_id = rule.id().clone();
            rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
        }
    }

    // Load Python AST rules
    #[cfg(feature = "lang-python")]
    {
        for (rule_name, toml_content) in BUILTIN_AST_PYTHON_RULES {
            let rule = AstRule::from_toml(toml_content).map_err(|e| {
                RuleError::InvalidDefinition(format!(
                    "Failed to parse built-in Python AST rule '{}': {}",
                    rule_name, e
                ))
            })?;

            let rule_id = rule.id().clone();
            rules.push((rule_id, Box::new(rule) as Box<dyn Rule>));
        }
    }

    // Load TypeScript AST rules
    #[cfg(feature = "lang-typescript")]
    {
        for (rule_name, toml_content) in BUILTIN_AST_TYPESCRIPT_RULES {
            let rule = AstRule::from_toml(toml_content).map_err(|e| {
                RuleError::InvalidDefinition(format!(
                    "Failed to parse built-in TypeScript AST rule '{}': {}",
                    rule_name, e
                ))
            })?;

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
        assert_eq!(rules.len(), 2); // no-todo-comments and no-fixme-comments

        // Check that rule IDs are correct
        let rule_ids: Vec<&str> = rules.iter().map(|(id, _)| id.as_str()).collect();
        assert!(rule_ids.contains(&"no-todo-comments"));
        assert!(rule_ids.contains(&"no-fixme-comments"));
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
