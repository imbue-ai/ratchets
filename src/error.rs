//! Error types for Ratchets
//!
//! This module defines the error types used throughout Ratchets, following
//! a hierarchical structure with specific error variants for different
//! error categories.

use std::path::PathBuf;

/// Configuration-related errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Invalid configuration syntax
    #[error("Invalid configuration syntax: {0}")]
    InvalidSyntax(String),

    /// Missing required configuration field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Invalid configuration value
    #[error("Invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },

    /// Failed to read config file
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse TOML
    #[error("Failed to parse TOML: {0}")]
    Parse(#[from] toml::de::Error),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    Validation(String),
}

/// Rule-related errors
#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    /// Invalid rule definition
    #[error("Invalid rule definition: {0}")]
    InvalidDefinition(String),

    /// Rule not found
    #[error("Rule not found: {0}")]
    NotFound(String),

    /// Invalid regex pattern
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(String),

    /// Invalid tree-sitter query
    #[error("Invalid tree-sitter query: {0}")]
    InvalidQuery(String),
}

/// Top-level error type for Ratchets
#[derive(Debug, thiserror::Error)]
pub enum RatchetError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Rule error
    #[error("Rule error: {0}")]
    Rule(#[from] RuleError),

    /// Parse error in source file
    #[error("Parse error in {file}: {message}")]
    Parse { file: PathBuf, message: String },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display_invalid_syntax() {
        let err = ConfigError::InvalidSyntax("unexpected token".to_string());
        assert_eq!(
            err.to_string(),
            "Invalid configuration syntax: unexpected token"
        );
    }

    #[test]
    fn test_config_error_display_missing_field() {
        let err = ConfigError::MissingField("timeout".to_string());
        assert_eq!(err.to_string(), "Missing required field: timeout");
    }

    #[test]
    fn test_config_error_display_invalid_value() {
        let err = ConfigError::InvalidValue {
            field: "max_retries".to_string(),
            message: "must be positive".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid value for max_retries: must be positive"
        );
    }

    #[test]
    fn test_rule_error_display_invalid_definition() {
        let err = RuleError::InvalidDefinition("missing pattern".to_string());
        assert_eq!(err.to_string(), "Invalid rule definition: missing pattern");
    }

    #[test]
    fn test_rule_error_display_not_found() {
        let err = RuleError::NotFound("no-unwrap".to_string());
        assert_eq!(err.to_string(), "Rule not found: no-unwrap");
    }

    #[test]
    fn test_rule_error_display_invalid_regex() {
        let err = RuleError::InvalidRegex("[unclosed".to_string());
        assert_eq!(err.to_string(), "Invalid regex pattern: [unclosed");
    }

    #[test]
    fn test_rule_error_display_invalid_query() {
        let err = RuleError::InvalidQuery("syntax error".to_string());
        assert_eq!(err.to_string(), "Invalid tree-sitter query: syntax error");
    }

    #[test]
    fn test_ratchet_error_display_config() {
        let config_err = ConfigError::MissingField("rules".to_string());
        let err = RatchetError::Config(config_err);
        assert_eq!(
            err.to_string(),
            "Configuration error: Missing required field: rules"
        );
    }

    #[test]
    fn test_ratchet_error_display_rule() {
        let rule_err = RuleError::NotFound("some-rule".to_string());
        let err = RatchetError::Rule(rule_err);
        assert_eq!(err.to_string(), "Rule error: Rule not found: some-rule");
    }

    #[test]
    fn test_ratchet_error_display_parse() {
        let err = RatchetError::Parse {
            file: PathBuf::from("src/main.rs"),
            message: "unexpected EOF".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Parse error in src/main.rs: unexpected EOF"
        );
    }

    #[test]
    fn test_ratchet_error_from_config_error() {
        let config_err = ConfigError::InvalidSyntax("test".to_string());
        let _ratchet_err: RatchetError = config_err.into();
        // Verify that the From trait works by successful compilation
    }

    #[test]
    fn test_ratchet_error_from_rule_error() {
        let rule_err = RuleError::InvalidRegex("test".to_string());
        let _ratchet_err: RatchetError = rule_err.into();
        // Verify that the From trait works by successful compilation
    }

    #[test]
    fn test_ratchet_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let _ratchet_err: RatchetError = io_err.into();
        // Verify that the From trait works by successful compilation
    }
}
