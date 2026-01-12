#![forbid(unsafe_code)]

//! Ratchet: Progressive lint enforcement for human and AI developers
//!
//! Ratchet is a progressive lint enforcement tool that allows codebases to contain
//! existing violations while preventing new ones.

pub mod cli;
pub mod config;
pub mod engine;
pub mod error;
pub mod output;
pub mod rules;

// Re-export error types for convenient access
pub use error::{ConfigError, RatchetError, RuleError};
