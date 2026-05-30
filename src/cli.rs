//! CLI argument parsing and command dispatch

pub mod args;
pub mod bump;
pub mod check;
pub mod common;
pub mod git_diff;
pub mod init;
pub mod list;
pub mod merge_driver;
pub mod tighten;

// Re-export types for convenient access
pub use args::{Cli, ColorChoice, Command, OutputFormat};
