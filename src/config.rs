//! Configuration file parsing and validation

pub mod counts;
pub mod ratchet_toml;
pub mod sets;

pub use counts::{CountsManager, RegionTree};
pub use ratchet_toml::{
    ColorOption, Config, OutputConfig, OutputFormat, RuleSettings, RulesConfig,
};
pub use sets::{RatchetSet, ResolveError, SetRegistry};
