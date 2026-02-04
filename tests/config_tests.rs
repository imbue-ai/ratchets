//! Integration tests for configuration parsing
//!
//! This module contains integration tests that verify:
//! - Config loading from files
//! - CountsManager loading from files
//! - End-to-end parsing with various valid and invalid inputs
//! - Region inheritance resolution

use ratchets::config::{Config, CountsManager};
use ratchets::types::RuleId;
use std::path::{Path, PathBuf};

// Helper to get fixture path
fn fixture_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("config")
        .join(filename)
}

// ============================================================================
// Config (ratchets.toml) Integration Tests
// ============================================================================

#[test]
fn test_config_load_valid_minimal() {
    let path = fixture_path("valid_minimal.toml");
    let config = Config::load(&path).unwrap();

    assert_eq!(config.ratchets.version, "1");
    assert_eq!(config.ratchets.languages.len(), 1);
}

#[test]
fn test_config_load_valid_full() {
    let path = fixture_path("valid_full.toml");
    let config = Config::load(&path).unwrap();

    assert_eq!(config.ratchets.version, "1");
    assert_eq!(config.ratchets.languages.len(), 3);
    assert_eq!(config.ratchets.include.len(), 2);
    assert_eq!(config.ratchets.exclude.len(), 2);

    // Verify rules are parsed
    assert!(config.rules.builtin.len() >= 4);
    assert_eq!(config.rules.custom.len(), 2);
}

#[test]
fn test_config_load_invalid_version() {
    let path = fixture_path("invalid_version.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Unsupported configuration version"));
}

#[test]
fn test_config_load_invalid_missing_version() {
    let path = fixture_path("invalid_missing_version.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
}

#[test]
fn test_config_load_invalid_missing_languages() {
    let path = fixture_path("invalid_missing_languages.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg
            .contains("No languages configured. Add languages to ratchets.toml to start checking.")
    );
}

#[test]
fn test_config_load_invalid_empty_languages() {
    let path = fixture_path("invalid_empty_languages.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg
            .contains("No languages configured. Add languages to ratchets.toml to start checking.")
    );
}

#[test]
fn test_config_load_invalid_language_name() {
    let path = fixture_path("invalid_language_name.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
}

#[test]
fn test_config_load_invalid_glob_include() {
    let path = fixture_path("invalid_glob_include.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Invalid include glob pattern"));
}

#[test]
fn test_config_load_invalid_glob_exclude() {
    let path = fixture_path("invalid_glob_exclude.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Invalid exclude glob pattern"));
}

#[test]
fn test_config_load_invalid_rule_region_glob() {
    let path = fixture_path("invalid_rule_region_glob.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Invalid region glob pattern"));
}

#[test]
fn test_config_load_valid_output_jsonl() {
    let path = fixture_path("valid_output_jsonl.toml");
    let config = Config::load(&path).unwrap();

    assert_eq!(config.output.format, ratchets::config::OutputFormat::Jsonl);
    assert_eq!(config.output.color, ratchets::config::ColorOption::Never);
}

#[test]
fn test_config_load_valid_output_color_always() {
    let path = fixture_path("valid_output_color_always.toml");
    let config = Config::load(&path).unwrap();

    assert_eq!(config.output.color, ratchets::config::ColorOption::Always);
}

#[test]
fn test_config_load_nonexistent_file() {
    let path = fixture_path("nonexistent.toml");
    let result = Config::load(&path);

    assert!(result.is_err());
    // Should be an IO error
    match result.unwrap_err() {
        ratchets::error::ConfigError::Io(_) => {} // Expected
        other => panic!("Expected IO error, got: {:?}", other),
    }
}

// ============================================================================
// CountsManager (ratchet-counts.toml) Integration Tests
// ============================================================================

#[test]
fn test_counts_load_valid_empty() {
    let path = fixture_path("valid_counts_empty.toml");
    let manager = CountsManager::load(&path).unwrap();

    // Empty file should parse successfully
    let rule_id = RuleId::new("any-rule").unwrap();
    assert_eq!(manager.get_budget(&rule_id, Path::new("src/foo.rs")), 0);
}

#[test]
fn test_counts_load_valid_simple() {
    let path = fixture_path("valid_counts_simple.toml");
    let manager = CountsManager::load(&path).unwrap();

    let rule_id = RuleId::new("no-unwrap").unwrap();

    // Test root budget
    assert_eq!(manager.get_budget(&rule_id, Path::new("src/foo.rs")), 0);

    // Test explicit region
    assert_eq!(
        manager.get_budget(&rule_id, Path::new("src/legacy/bar.rs")),
        10
    );
}

#[test]
fn test_counts_load_valid_multiple() {
    let path = fixture_path("valid_counts_multiple.toml");
    let manager = CountsManager::load(&path).unwrap();

    // Test no-unwrap rule
    let no_unwrap = RuleId::new("no-unwrap").unwrap();
    assert_eq!(
        manager.get_budget(&no_unwrap, Path::new("src/legacy/parser/x.rs")),
        7
    );
    assert_eq!(
        manager.get_budget(&no_unwrap, Path::new("tests/test.rs")),
        50
    );

    // Test no-todo-comments rule
    let no_todo = RuleId::new("no-todo-comments").unwrap();
    assert_eq!(manager.get_budget(&no_todo, Path::new("src/main.rs")), 23);

    // Test my-company-rule
    let company_rule = RuleId::new("my-company-rule").unwrap();
    assert_eq!(
        manager.get_budget(&company_rule, Path::new("src/experimental/foo.rs")),
        5
    );
}

#[test]
fn test_counts_load_inheritance_root_to_child() {
    let path = fixture_path("valid_counts_inheritance.toml");
    let manager = CountsManager::load(&path).unwrap();

    let rule_id = RuleId::new("no-unwrap").unwrap();

    // Test inheritance hierarchy: root -> src -> src/legacy -> src/legacy/parser
    // File at root level (non-src) should inherit from "." = 0
    assert_eq!(manager.get_budget(&rule_id, Path::new("README.md")), 0);

    // File in src should inherit from "src" = 100
    assert_eq!(manager.get_budget(&rule_id, Path::new("src/main.rs")), 100);

    // File in src/legacy should inherit from "src/legacy" = 50
    assert_eq!(
        manager.get_budget(&rule_id, Path::new("src/legacy/old.rs")),
        50
    );

    // File in src/legacy/parser should inherit from "src/legacy/parser" = 10
    assert_eq!(
        manager.get_budget(&rule_id, Path::new("src/legacy/parser/lexer.rs")),
        10
    );
}

#[test]
fn test_counts_load_inheritance_parent_to_nested_child() {
    let path = fixture_path("valid_counts_inheritance.toml");
    let manager = CountsManager::load(&path).unwrap();

    let rule_id = RuleId::new("no-unwrap").unwrap();

    // File in src/legacy/parser/sub should inherit from nearest parent "src/legacy/parser" = 10
    assert_eq!(
        manager.get_budget(&rule_id, Path::new("src/legacy/parser/sub/deep.rs")),
        10
    );

    // File in src/other (no explicit region) should inherit from "src" = 100
    assert_eq!(
        manager.get_budget(&rule_id, Path::new("src/other/file.rs")),
        100
    );
}

#[test]
fn test_counts_load_inheritance_most_specific_wins() {
    let path = fixture_path("valid_counts_inheritance.toml");
    let manager = CountsManager::load(&path).unwrap();

    let rule_id = RuleId::new("no-unwrap").unwrap();

    // Even though "src" = 100, "src/legacy/parser" = 10 is more specific and should win
    assert_eq!(
        manager.get_budget(&rule_id, Path::new("src/legacy/parser/ast.rs")),
        10
    );

    // Even though "src" = 100, "src/legacy" = 50 is more specific
    assert_eq!(
        manager.get_budget(&rule_id, Path::new("src/legacy/mod.rs")),
        50
    );
}

#[test]
fn test_counts_load_invalid_negative() {
    let path = fixture_path("invalid_counts_negative.toml");
    let result = CountsManager::load(&path);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("non-negative"));
}

#[test]
fn test_counts_load_invalid_non_integer() {
    let path = fixture_path("invalid_counts_non_integer.toml");
    let result = CountsManager::load(&path);

    assert!(result.is_err());
}

#[test]
fn test_counts_load_invalid_bad_rule_id() {
    let path = fixture_path("invalid_counts_bad_rule_id.toml");
    let result = CountsManager::load(&path);

    // File has spaces in rule ID which will cause TOML parse error
    assert!(result.is_err());
}

#[test]
fn test_counts_load_nonexistent_file() {
    let path = fixture_path("nonexistent_counts.toml");
    let result = CountsManager::load(&path);

    assert!(result.is_err());
    // Should be an IO error
    match result.unwrap_err() {
        ratchets::error::ConfigError::Io(_) => {} // Expected
        other => panic!("Expected IO error, got: {:?}", other),
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_counts_region_path_normalization() {
    // Test that region path normalization works in inheritance
    let path = fixture_path("valid_counts_inheritance.toml");
    let manager = CountsManager::load(&path).unwrap();

    let rule_id = RuleId::new("no-unwrap").unwrap();

    // These should all resolve to the same budget due to path normalization
    assert_eq!(
        manager.get_budget(&rule_id, Path::new("src/legacy/file.rs")),
        50
    );
}

#[test]
fn test_counts_unknown_rule_defaults_to_zero() {
    let path = fixture_path("valid_counts_simple.toml");
    let manager = CountsManager::load(&path).unwrap();

    // Rule not in counts file should default to 0 (strict enforcement)
    let unknown_rule = RuleId::new("unknown-rule").unwrap();
    assert_eq!(
        manager.get_budget(&unknown_rule, Path::new("src/foo.rs")),
        0
    );
}

#[test]
fn test_config_defaults_when_sections_omitted() {
    // Test that default values are applied when optional sections are omitted
    let path = fixture_path("valid_minimal.toml");
    let config = Config::load(&path).unwrap();

    // Output section should have defaults
    assert_eq!(config.output.format, ratchets::config::OutputFormat::Human);
    assert_eq!(config.output.color, ratchets::config::ColorOption::Auto);

    // Rules section should be empty by default
    assert_eq!(config.rules.builtin.len(), 0);
    assert_eq!(config.rules.custom.len(), 0);

    // Include should have default "**/*"
    assert_eq!(config.ratchets.include.len(), 1);
    assert_eq!(config.ratchets.include[0].as_str(), "**/*");

    // Exclude should be empty
    assert_eq!(config.ratchets.exclude.len(), 0);
}
