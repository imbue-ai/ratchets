#![forbid(unsafe_code)]

//! Rule definitions and registry

mod ast;
mod builtin;
mod regex_rule;
mod registry;
mod rule;

// Re-export core types
pub use ast::{AstRule, ParserCache};
pub use builtin::{load_builtin_ast_rules, load_builtin_regex_rules};
pub use regex_rule::RegexRule;
pub use registry::RuleRegistry;
pub use rule::{AstPlaceholder, ExecutionContext, RegionResolver, Rule, RuleContext, Violation};
