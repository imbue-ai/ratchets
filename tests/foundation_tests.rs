//! Integration tests for Ratchet foundation types
//!
//! This module contains integration tests for the error types and domain types
//! defined in the Ratchet library.

use ratchet::error::{ConfigError, RatchetError, RuleError};
use ratchet::types::{GlobPattern, Language, RegionPath, RuleId, Severity};
use std::path::PathBuf;

// Error integration tests

#[test]
fn test_error_hierarchy_config_to_ratchet() {
    let config_err = ConfigError::InvalidSyntax("bad syntax".to_string());
    let ratchet_err: RatchetError = config_err.into();

    match ratchet_err {
        RatchetError::Config(_) => {} // Expected
        _ => panic!("Expected RatchetError::Config variant"),
    }
}

#[test]
fn test_error_hierarchy_rule_to_ratchet() {
    let rule_err = RuleError::NotFound("missing-rule".to_string());
    let ratchet_err: RatchetError = rule_err.into();

    match ratchet_err {
        RatchetError::Rule(_) => {} // Expected
        _ => panic!("Expected RatchetError::Rule variant"),
    }
}

#[test]
fn test_error_hierarchy_io_to_ratchet() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let ratchet_err: RatchetError = io_err.into();

    match ratchet_err {
        RatchetError::Io(_) => {} // Expected
        _ => panic!("Expected RatchetError::Io variant"),
    }
}

#[test]
fn test_parse_error_contains_file_path() {
    let err = RatchetError::Parse {
        file: PathBuf::from("/path/to/file.rs"),
        message: "syntax error".to_string(),
    };

    let err_string = err.to_string();
    assert!(err_string.contains("/path/to/file.rs"));
    assert!(err_string.contains("syntax error"));
}

#[test]
fn test_config_error_variants_display() {
    let invalid_syntax = ConfigError::InvalidSyntax("test".to_string());
    assert!(
        invalid_syntax
            .to_string()
            .contains("Invalid configuration syntax")
    );

    let missing_field = ConfigError::MissingField("field".to_string());
    assert!(missing_field.to_string().contains("Missing required field"));

    let invalid_value = ConfigError::InvalidValue {
        field: "timeout".to_string(),
        message: "negative".to_string(),
    };
    assert!(invalid_value.to_string().contains("Invalid value"));
}

#[test]
fn test_rule_error_variants_display() {
    let invalid_def = RuleError::InvalidDefinition("test".to_string());
    assert!(invalid_def.to_string().contains("Invalid rule definition"));

    let not_found = RuleError::NotFound("rule".to_string());
    assert!(not_found.to_string().contains("Rule not found"));

    let invalid_regex = RuleError::InvalidRegex("pattern".to_string());
    assert!(invalid_regex.to_string().contains("Invalid regex pattern"));

    let invalid_query = RuleError::InvalidQuery("query".to_string());
    assert!(
        invalid_query
            .to_string()
            .contains("Invalid tree-sitter query")
    );
}

// Language integration tests

#[test]
fn test_language_roundtrip_serialization() {
    let languages = vec![
        Language::Rust,
        Language::TypeScript,
        Language::JavaScript,
        Language::Python,
        Language::Go,
    ];

    for lang in languages {
        let json = serde_json::to_string(&lang).unwrap();
        let deserialized: Language = serde_json::from_str(&json).unwrap();
        assert_eq!(lang, deserialized);
    }
}

#[test]
fn test_language_lowercase_serialization() {
    // Verify that all languages serialize to lowercase
    let rust_json = serde_json::to_string(&Language::Rust).unwrap();
    assert!(rust_json.contains("rust"));
    assert!(!rust_json.contains("Rust"));

    let typescript_json = serde_json::to_string(&Language::TypeScript).unwrap();
    assert!(typescript_json.contains("typescript"));
    assert!(!typescript_json.contains("TypeScript"));
}

#[test]
fn test_language_in_collections() {
    use std::collections::{HashMap, HashSet};

    let mut set = HashSet::new();
    set.insert(Language::Rust);
    set.insert(Language::Python);
    assert_eq!(set.len(), 2);

    let mut map = HashMap::new();
    map.insert(Language::Rust, "rs");
    map.insert(Language::Python, "py");
    assert_eq!(map.get(&Language::Rust), Some(&"rs"));
}

// Severity integration tests

#[test]
fn test_severity_roundtrip_serialization() {
    let severities = vec![Severity::Error, Severity::Warning, Severity::Info];

    for severity in severities {
        let json = serde_json::to_string(&severity).unwrap();
        let deserialized: Severity = serde_json::from_str(&json).unwrap();
        assert_eq!(severity, deserialized);
    }
}

#[test]
fn test_severity_lowercase_serialization() {
    let error_json = serde_json::to_string(&Severity::Error).unwrap();
    assert_eq!(error_json, "\"error\"");

    let warning_json = serde_json::to_string(&Severity::Warning).unwrap();
    assert_eq!(warning_json, "\"warning\"");

    let info_json = serde_json::to_string(&Severity::Info).unwrap();
    assert_eq!(info_json, "\"info\"");
}

// RuleId integration tests

#[test]
fn test_rule_id_roundtrip_serialization() {
    let rule_id = RuleId::new("test-rule-123").unwrap();
    let json = serde_json::to_string(&rule_id).unwrap();
    let deserialized: RuleId = serde_json::from_str(&json).unwrap();
    assert_eq!(rule_id, deserialized);
}

#[test]
fn test_rule_id_validation_comprehensive() {
    // Valid cases
    let valid_ids = vec![
        "simple",
        "with-dashes",
        "with_underscores",
        "Mixed-Case_123",
        "123numeric",
        "a",
    ];

    for id in valid_ids {
        assert!(RuleId::new(id).is_some(), "Expected '{}' to be valid", id);
    }

    // Invalid cases
    let invalid_ids = vec![
        "",
        "with spaces",
        "with@symbol",
        "with.dot",
        "with/slash",
        "with\\backslash",
        "with:colon",
    ];

    for id in invalid_ids {
        assert!(RuleId::new(id).is_none(), "Expected '{}' to be invalid", id);
    }
}

#[test]
fn test_rule_id_serde_deserialization_invalid() {
    // Verify that deserializing an invalid rule ID fails
    let result = serde_json::from_str::<RuleId>("\"invalid rule\"");
    assert!(result.is_err());
}

#[test]
fn test_rule_id_in_collections() {
    use std::collections::{HashMap, HashSet};

    let mut set = HashSet::new();
    set.insert(RuleId::new("rule1").unwrap());
    set.insert(RuleId::new("rule2").unwrap());
    set.insert(RuleId::new("rule1").unwrap()); // Duplicate
    assert_eq!(set.len(), 2);

    let mut map = HashMap::new();
    map.insert(RuleId::new("no-unwrap").unwrap(), "Rule description");
    assert_eq!(
        map.get(&RuleId::new("no-unwrap").unwrap()),
        Some(&"Rule description")
    );
}

// RegionPath integration tests

#[test]
fn test_region_path_roundtrip_serialization() {
    let path = RegionPath::new("src/parser/ast");
    let json = serde_json::to_string(&path).unwrap();
    let deserialized: RegionPath = serde_json::from_str(&json).unwrap();
    assert_eq!(path, deserialized);
}

#[test]
fn test_region_path_normalization_comprehensive() {
    // Test various normalization scenarios
    let test_cases = vec![
        ("", "."),
        (".", "."),
        ("/", "."),
        ("./", "."),
        ("src", "src"),
        ("src/", "src"),
        ("./src", "src"),
        ("src/parser", "src/parser"),
        ("src/parser/", "src/parser"),
        ("./src/parser", "src/parser"),
        ("src\\parser", "src/parser"),
        ("src\\parser\\ast", "src/parser/ast"),
        (".\\src", "src"),
        ("src\\parser/ast", "src/parser/ast"),
    ];

    for (input, expected) in test_cases {
        let path = RegionPath::new(input);
        assert_eq!(
            path.as_str(),
            expected,
            "Input '{}' should normalize to '{}'",
            input,
            expected
        );
    }
}

#[test]
fn test_region_path_serde_normalizes_on_deserialization() {
    // Verify that normalization happens during deserialization
    let json_with_backslash = "\"src\\\\parser\"";
    let path: RegionPath = serde_json::from_str(json_with_backslash).unwrap();
    assert_eq!(path.as_str(), "src/parser");

    let json_with_trailing = "\"src/parser/\"";
    let path: RegionPath = serde_json::from_str(json_with_trailing).unwrap();
    assert_eq!(path.as_str(), "src/parser");
}

#[test]
fn test_region_path_in_collections() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(RegionPath::new("src"));
    set.insert(RegionPath::new("tests"));
    set.insert(RegionPath::new("src")); // Duplicate
    assert_eq!(set.len(), 2);

    // Normalized paths should be equal
    let path1 = RegionPath::new("src/parser");
    let path2 = RegionPath::new("src/parser/");
    let path3 = RegionPath::new("./src/parser");
    assert_eq!(path1, path2);
    assert_eq!(path2, path3);
}

// GlobPattern integration tests

#[test]
fn test_glob_pattern_creation_and_display() {
    let patterns = vec!["**/*.rs", "src/**/*.toml", "*.md", "test_*"];

    for pattern_str in patterns {
        let pattern = GlobPattern::new(pattern_str);
        assert_eq!(pattern.as_str(), pattern_str);
        assert_eq!(pattern.to_string(), pattern_str);
    }
}

#[test]
fn test_glob_pattern_in_collections() {
    use std::collections::{HashMap, HashSet};

    let mut set = HashSet::new();
    set.insert(GlobPattern::new("*.rs"));
    set.insert(GlobPattern::new("*.toml"));
    set.insert(GlobPattern::new("*.rs")); // Duplicate
    assert_eq!(set.len(), 2);

    let mut map = HashMap::new();
    map.insert(GlobPattern::new("**/*.rs"), "Rust files");
    assert_eq!(map.get(&GlobPattern::new("**/*.rs")), Some(&"Rust files"));
}

// Cross-type integration tests

#[test]
fn test_types_work_together() {
    // Simulate a simple configuration-like structure
    let rule_id = RuleId::new("no-unwrap").unwrap();
    let region = RegionPath::new("src/parser");
    let pattern = GlobPattern::new("**/*.rs");
    let language = Language::Rust;
    let severity = Severity::Error;

    // Verify all types can be used together
    let _config = (rule_id, region, pattern, language, severity);

    // All types should be serializable
    assert!(serde_json::to_string(&Language::Rust).is_ok());
    assert!(serde_json::to_string(&Severity::Error).is_ok());
    assert!(serde_json::to_string(&RuleId::new("test").unwrap()).is_ok());
    assert!(serde_json::to_string(&RegionPath::new("src")).is_ok());
    assert!(serde_json::to_string(&GlobPattern::new("*.rs")).is_ok());
}

#[test]
fn test_all_types_are_cloneable_and_hashable() {
    // All foundation types should be Clone and Hash
    // Test that they can be cloned and inserted into HashSets
    use std::collections::HashSet;

    let mut lang_set = HashSet::new();
    lang_set.insert(Language::Rust); // Copy type

    let mut severity_set = HashSet::new();
    severity_set.insert(Severity::Error); // Copy type

    let mut rule_set = HashSet::new();
    rule_set.insert(RuleId::new("test").unwrap().clone());

    let mut path_set = HashSet::new();
    path_set.insert(RegionPath::new("src").clone());

    let mut pattern_set = HashSet::new();
    pattern_set.insert(GlobPattern::new("*.rs").clone());
}
