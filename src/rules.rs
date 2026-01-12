#![forbid(unsafe_code)]

//! Rule definitions and registry

mod rule;

// Re-export core types
pub use rule::{AstPlaceholder, ExecutionContext, Rule, Violation};
