//! Error types for Ratchet
//!
//! This module defines the error types used throughout Ratchet, following
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

/// Top-level error type for Ratchet
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
