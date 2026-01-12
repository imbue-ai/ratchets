#![forbid(unsafe_code)]

//! Rule definitions and registry

mod regex_rule;
mod registry;
mod rule;

// Re-export core types
pub use regex_rule::RegexRule;
pub use registry::RuleRegistry;
pub use rule::{AstPlaceholder, ExecutionContext, Rule, Violation};
