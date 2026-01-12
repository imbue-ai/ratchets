//! CLI argument parsing and command dispatch

pub mod args;
pub mod bump;
pub mod check;
pub mod init;
pub mod tighten;

// Re-export types for convenient access
pub use args::{Cli, ColorChoice, Command, OutputFormat};
