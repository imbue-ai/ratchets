#![forbid(unsafe_code)]

//! RuleStatus output formatters
//!
//! This module provides formatters for displaying rule status information
//! from the `ratchet list` command. It supports both human-readable and
//! JSONL output formats.

use serde::Serialize;

/// Status of a rule check (pass, fail, or over-budget)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    OverBudget,
}

impl CheckStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckStatus::Pass => "pass",
            CheckStatus::OverBudget => "over_budget",
        }
    }
}

/// Source of a rule (builtin or custom)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSource {
    Builtin,
    Custom,
}

impl RuleSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleSource::Builtin => "builtin",
            RuleSource::Custom => "custom",
        }
    }
}

/// Status information for a single rule
#[derive(Debug, Clone)]
pub struct RuleStatus {
    pub rule_id: String,
    pub description: String,
    pub source: RuleSource,
    pub languages: Vec<String>,
    pub severity: String,
    pub violations: u64,
    pub budget: u64,
    pub status: CheckStatus,
}

/// Human-readable formatter for rule status
pub struct RuleStatusHumanFormatter;

impl RuleStatusHumanFormatter {
    /// Create a new human formatter
    pub fn new() -> Self {
        RuleStatusHumanFormatter
    }

    /// Format a list of rule statuses for human consumption
    pub fn format(&self, statuses: &[RuleStatus]) -> String {
        let mut output = String::new();

        output.push_str(&format!("Rules ({} enabled):\n", statuses.len()));
        output.push('\n');

        for status in statuses {
            output.push_str(&format!(
                "{} ({})\n",
                status.rule_id,
                status.source.as_str()
            ));
            output.push_str(&format!("  Description: {}\n", status.description));
            output.push_str(&format!("  Languages: {}\n", status.languages.join(", ")));
            output.push_str(&format!("  Severity: {}\n", status.severity));
            output.push_str(&format!("  Violations: {}\n", status.violations));
            output.push_str(&format!("  Budget: {}\n", status.budget));

            let (icon, status_text) = match status.status {
                CheckStatus::Pass => ("✓", "within budget".to_string()),
                CheckStatus::OverBudget => {
                    let excess = status.violations.saturating_sub(status.budget);
                    ("✗", format!("exceeded by {}", excess))
                }
            };

            output.push_str(&format!("  Status: {} {}\n", icon, status_text));
            output.push('\n');
        }

        output
    }

    /// Write the formatted output to stdout
    pub fn write_to_stdout(&self, statuses: &[RuleStatus]) {
        print!("{}", self.format(statuses));
    }
}

impl Default for RuleStatusHumanFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// JSONL output structure for rule status
#[derive(Debug, Serialize)]
struct JsonlRuleStatus {
    rule_id: String,
    source: String,
    description: String,
    languages: Vec<String>,
    severity: String,
    violations: u64,
    budget: u64,
    status: String,
}

/// JSONL formatter for rule status
pub struct RuleStatusJsonlFormatter;

impl RuleStatusJsonlFormatter {
    /// Create a new JSONL formatter
    pub fn new() -> Self {
        RuleStatusJsonlFormatter
    }

    /// Format a list of rule statuses as JSONL
    ///
    /// Returns a string with one JSON object per line for each rule.
    pub fn format(&self, statuses: &[RuleStatus]) -> String {
        let mut output = String::new();

        for status in statuses {
            let jsonl_status = JsonlRuleStatus {
                rule_id: status.rule_id.clone(),
                source: status.source.as_str().to_string(),
                description: status.description.clone(),
                languages: status.languages.clone(),
                severity: status.severity.clone(),
                violations: status.violations,
                budget: status.budget,
                status: status.status.as_str().to_string(),
            };

            if let Ok(json) = serde_json::to_string(&jsonl_status) {
                output.push_str(&json);
                output.push('\n');
            }
        }

        output
    }

    /// Write the formatted output to stdout
    pub fn write_to_stdout(&self, statuses: &[RuleStatus]) {
        print!("{}", self.format(statuses));
    }
}

impl Default for RuleStatusJsonlFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_status(
        rule_id: &str,
        violations: u64,
        budget: u64,
        status: CheckStatus,
    ) -> RuleStatus {
        RuleStatus {
            rule_id: rule_id.to_string(),
            description: format!("{} description", rule_id),
            source: RuleSource::Builtin,
            languages: vec!["rust".to_string()],
            severity: "error".to_string(),
            violations,
            budget,
            status,
        }
    }

    #[test]
    fn test_check_status_as_str() {
        assert_eq!(CheckStatus::Pass.as_str(), "pass");
        assert_eq!(CheckStatus::OverBudget.as_str(), "over_budget");
    }

    #[test]
    fn test_rule_source_as_str() {
        assert_eq!(RuleSource::Builtin.as_str(), "builtin");
        assert_eq!(RuleSource::Custom.as_str(), "custom");
    }

    #[test]
    fn test_human_formatter_empty() {
        let formatter = RuleStatusHumanFormatter::new();
        let statuses = vec![];
        let output = formatter.format(&statuses);
        assert!(output.contains("Rules (0 enabled)"));
    }

    #[test]
    fn test_human_formatter_single_rule_pass() {
        let formatter = RuleStatusHumanFormatter::new();
        let statuses = vec![create_test_status("no-unwrap", 5, 10, CheckStatus::Pass)];
        let output = formatter.format(&statuses);

        assert!(output.contains("Rules (1 enabled)"));
        assert!(output.contains("no-unwrap (builtin)"));
        assert!(output.contains("Description: no-unwrap description"));
        assert!(output.contains("Languages: rust"));
        assert!(output.contains("Severity: error"));
        assert!(output.contains("Violations: 5"));
        assert!(output.contains("Budget: 10"));
        assert!(output.contains("✓ within budget"));
    }

    #[test]
    fn test_human_formatter_single_rule_over_budget() {
        let formatter = RuleStatusHumanFormatter::new();
        let statuses = vec![create_test_status(
            "no-unwrap",
            15,
            10,
            CheckStatus::OverBudget,
        )];
        let output = formatter.format(&statuses);

        assert!(output.contains("Rules (1 enabled)"));
        assert!(output.contains("no-unwrap (builtin)"));
        assert!(output.contains("Violations: 15"));
        assert!(output.contains("Budget: 10"));
        assert!(output.contains("✗ exceeded by 5"));
    }

    #[test]
    fn test_human_formatter_multiple_rules() {
        let formatter = RuleStatusHumanFormatter::new();
        let statuses = vec![
            create_test_status("no-unwrap", 5, 10, CheckStatus::Pass),
            create_test_status("no-todo", 8, 5, CheckStatus::OverBudget),
        ];
        let output = formatter.format(&statuses);

        assert!(output.contains("Rules (2 enabled)"));
        assert!(output.contains("no-unwrap (builtin)"));
        assert!(output.contains("no-todo (builtin)"));
        assert!(output.contains("✓ within budget"));
        assert!(output.contains("✗ exceeded by 3"));
    }

    #[test]
    fn test_jsonl_formatter_empty() {
        let formatter = RuleStatusJsonlFormatter::new();
        let statuses = vec![];
        let output = formatter.format(&statuses);
        assert_eq!(output, "");
    }

    #[test]
    fn test_jsonl_formatter_single_rule() {
        let formatter = RuleStatusJsonlFormatter::new();
        let statuses = vec![create_test_status("no-unwrap", 5, 10, CheckStatus::Pass)];
        let output = formatter.format(&statuses);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);

        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["rule_id"], "no-unwrap");
        assert_eq!(parsed["source"], "builtin");
        assert_eq!(parsed["description"], "no-unwrap description");
        assert_eq!(parsed["languages"][0], "rust");
        assert_eq!(parsed["severity"], "error");
        assert_eq!(parsed["violations"], 5);
        assert_eq!(parsed["budget"], 10);
        assert_eq!(parsed["status"], "pass");
    }

    #[test]
    fn test_jsonl_formatter_multiple_rules() {
        let formatter = RuleStatusJsonlFormatter::new();
        let statuses = vec![
            create_test_status("no-unwrap", 5, 10, CheckStatus::Pass),
            create_test_status("no-todo", 8, 5, CheckStatus::OverBudget),
        ];
        let output = formatter.format(&statuses);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);

        let parsed1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed1["rule_id"], "no-unwrap");
        assert_eq!(parsed1["status"], "pass");

        let parsed2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(parsed2["rule_id"], "no-todo");
        assert_eq!(parsed2["status"], "over_budget");
    }

    #[test]
    fn test_jsonl_valid_json() {
        let formatter = RuleStatusJsonlFormatter::new();
        let statuses = vec![create_test_status("no-unwrap", 5, 10, CheckStatus::Pass)];
        let output = formatter.format(&statuses);

        for line in output.lines() {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
            assert!(parsed.is_ok(), "Invalid JSON: {}", line);
        }
    }

    #[test]
    fn test_human_formatter_default() {
        let _formatter = RuleStatusHumanFormatter;
    }

    #[test]
    fn test_jsonl_formatter_default() {
        let _formatter = RuleStatusJsonlFormatter;
    }

    #[test]
    fn test_custom_source() {
        let mut status = create_test_status("custom-rule", 3, 5, CheckStatus::Pass);
        status.source = RuleSource::Custom;

        let formatter = RuleStatusHumanFormatter::new();
        let output = formatter.format(&[status.clone()]);
        assert!(output.contains("custom-rule (custom)"));

        let jsonl_formatter = RuleStatusJsonlFormatter::new();
        let jsonl_output = jsonl_formatter.format(&[status]);
        assert!(jsonl_output.contains("\"source\":\"custom\""));
    }

    #[test]
    fn test_multiple_languages() {
        let mut status = create_test_status("multi-lang", 2, 5, CheckStatus::Pass);
        status.languages = vec![
            "rust".to_string(),
            "python".to_string(),
            "typescript".to_string(),
        ];

        let formatter = RuleStatusHumanFormatter::new();
        let output = formatter.format(&[status.clone()]);
        assert!(output.contains("Languages: rust, python, typescript"));

        let jsonl_formatter = RuleStatusJsonlFormatter::new();
        let jsonl_output = jsonl_formatter.format(&[status]);
        let parsed: serde_json::Value =
            serde_json::from_str(jsonl_output.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["languages"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_zero_violations() {
        let formatter = RuleStatusHumanFormatter::new();
        let statuses = vec![create_test_status("no-unwrap", 0, 0, CheckStatus::Pass)];
        let output = formatter.format(&statuses);

        assert!(output.contains("Violations: 0"));
        assert!(output.contains("Budget: 0"));
        assert!(output.contains("✓ within budget"));
    }
}
