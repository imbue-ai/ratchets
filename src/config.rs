//! Configuration file parsing and validation

pub mod counts;
pub mod ratchet_toml;

pub use counts::{CountsManager, RegionTree};
pub use ratchet_toml::{
    ColorOption, Config, OutputConfig, OutputFormat, RuleSettings, RulesConfig,
};
