//! Rule execution engine and violation aggregation

pub mod executor;
pub mod file_walker;

pub use executor::{ExecutionEngine, ExecutionResult};
