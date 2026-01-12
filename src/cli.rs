//! CLI argument parsing and command dispatch

pub mod args;
pub mod check;
pub mod init;

// Re-export types for convenient access
pub use args::{Cli, ColorChoice, Command, OutputFormat};
