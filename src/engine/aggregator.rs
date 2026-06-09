#![forbid(unsafe_code)]

//! Violation aggregation and budget comparison
//!
//! This module aggregates violations by (rule_id, region) and compares
//! actual counts against budgets from the CountsManager to determine
//! pass/fail status.

use crate::config::counts::CountsManager;
use crate::rules::Violation;
use crate::types::{RegionPath, RuleId};
use std::collections::HashMap;

/// Result of aggregating violations against budgets
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregationResult {
    /// Per-rule/region status
    pub statuses: Vec<RuleRegionStatus>,
    /// Overall pass/fail
    pub passed: bool,
    /// Total violations found
    pub total_violations: usize,
    /// Total violations over budget
    pub violations_over_budget: usize,
}

/// Status for a single (rule, region) pair
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRegionStatus {
    pub rule_id: RuleId,
    pub region: RegionPath,
    pub actual_count: u64,
    pub budget: u64,
    pub passed: bool,
    pub violations: Vec<Violation>,
}

/// Aggregates violations and compares against budgets
pub struct ViolationAggregator {
    counts: CountsManager,
}

impl ViolationAggregator {
    /// Creates a new ViolationAggregator with the given CountsManager
    pub fn new(counts: CountsManager) -> Self {
        ViolationAggregator { counts }
    }

    /// Aggregate violations and check against budgets
    ///
    /// Algorithm:
    /// 1. Group violations by (rule_id, region_path)
    /// 2. For each group, count violations
    /// 3. Look up budget from CountsManager using the first file path in the group
    /// 4. Compare count vs budget: if actual > budget, status is FAIL
    /// 5. Overall pass = all rule/regions pass
    pub fn aggregate(&self, violations: Vec<Violation>) -> AggregationResult {
        // Group violations by (rule_id, region)
        let mut groups: HashMap<(RuleId, RegionPath), Vec<Violation>> = HashMap::new();

        for violation in violations {
            let key = (violation.rule_id.clone(), violation.region.clone());
            groups.entry(key).or_default().push(violation);
        }

        // Calculate status for each group
        let mut statuses = Vec::new();
        let mut total_violations = 0;
        let mut violations_over_budget = 0;
        let mut all_passed = true;

        for ((rule_id, region), group_violations) in groups.into_iter() {
            let actual_count = group_violations.len() as u64;
            total_violations += actual_count as usize;

            // Look up budget using the file path from the first violation
            // The CountsManager uses the file path for inheritance lookup
            let budget = if let Some(first_violation) = group_violations.first() {
                self.counts.get_budget(&rule_id, &first_violation.file)
            } else {
                // This shouldn't happen since we only create groups with violations
                0
            };

            let passed = actual_count <= budget;

            if !passed {
                all_passed = false;
                violations_over_budget += (actual_count - budget) as usize;
            }

            statuses.push(RuleRegionStatus {
                rule_id,
                region,
                actual_count,
                budget,
                passed,
                violations: group_violations,
            });
        }

        // Sort statuses for deterministic output
        statuses.sort_by(|a, b| {
            a.rule_id
                .as_str()
                .cmp(b.rule_id.as_str())
                .then_with(|| a.region.as_str().cmp(b.region.as_str()))
        });

        AggregationResult {
            statuses,
            passed: all_passed,
            total_violations,
            violations_over_budget,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_violation(
        rule_id: &str,
        file_path: &str,
        region: &str,
        line: u32,
    ) -> Result<Violation, Box<dyn std::error::Error>> {
        Ok(Violation {
            rule_id: RuleId::new(rule_id).ok_or("invalid rule id")?,
            file: PathBuf::from(file_path),
            line,
            column: 1,
            end_line: line,
            end_column: 10,
            snippet: "test".to_string(),
            message: "Test violation".to_string(),
            region: RegionPath::new(region),
        })
    }

    #[test]
    fn test_aggregator_empty_violations() {
        let counts = CountsManager::new();
        let aggregator = ViolationAggregator::new(counts);

        let result = aggregator.aggregate(vec![]);

        assert!(result.passed);
        assert_eq!(result.total_violations, 0);
        assert_eq!(result.violations_over_budget, 0);
        assert_eq!(result.statuses.len(), 0);
    }

    #[test]
    fn test_aggregator_single_violation_within_budget() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            5,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![create_test_violation(
            "no-unwrap",
            "src/main.rs",
            "src",
            10,
        )?];

        let result = aggregator.aggregate(violations);

        assert!(result.passed);
        assert_eq!(result.total_violations, 1);
        assert_eq!(result.violations_over_budget, 0);
        assert_eq!(result.statuses.len(), 1);

        let status = &result.statuses[0];
        assert_eq!(status.rule_id.as_str(), "no-unwrap");
        assert_eq!(status.region.as_str(), "src");
        assert_eq!(status.actual_count, 1);
        assert_eq!(status.budget, 5);
        assert!(status.passed);
        Ok(())
    }

    #[test]
    fn test_aggregator_single_violation_over_budget() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            0,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![create_test_violation(
            "no-unwrap",
            "src/main.rs",
            "src",
            10,
        )?];

        let result = aggregator.aggregate(violations);

        assert!(!result.passed);
        assert_eq!(result.total_violations, 1);
        assert_eq!(result.violations_over_budget, 1);
        assert_eq!(result.statuses.len(), 1);

        let status = &result.statuses[0];
        assert!(!status.passed);
        Ok(())
    }

    #[test]
    fn test_aggregator_multiple_violations_same_rule_region()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            2,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![
            create_test_violation("no-unwrap", "src/main.rs", "src", 10)?,
            create_test_violation("no-unwrap", "src/lib.rs", "src", 20)?,
        ];

        let result = aggregator.aggregate(violations);

        assert!(result.passed);
        assert_eq!(result.total_violations, 2);
        assert_eq!(result.violations_over_budget, 0);
        assert_eq!(result.statuses.len(), 1);

        let status = &result.statuses[0];
        assert_eq!(status.actual_count, 2);
        assert_eq!(status.budget, 2);
        assert!(status.passed);
        assert_eq!(status.violations.len(), 2);
        Ok(())
    }

    #[test]
    fn test_aggregator_multiple_violations_same_rule_region_over_budget()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            1,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![
            create_test_violation("no-unwrap", "src/main.rs", "src", 10)?,
            create_test_violation("no-unwrap", "src/lib.rs", "src", 20)?,
            create_test_violation("no-unwrap", "src/util.rs", "src", 30)?,
        ];

        let result = aggregator.aggregate(violations);

        assert!(!result.passed);
        assert_eq!(result.total_violations, 3);
        assert_eq!(result.violations_over_budget, 2); // 3 actual - 1 budget = 2 over
        assert_eq!(result.statuses.len(), 1);

        let status = &result.statuses[0];
        assert_eq!(status.actual_count, 3);
        assert_eq!(status.budget, 1);
        assert!(!status.passed);
        Ok(())
    }

    #[test]
    fn test_aggregator_multiple_rules_same_region() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            5,
        );
        counts.set_count(
            &RuleId::new("no-todo").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            3,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![
            create_test_violation("no-unwrap", "src/main.rs", "src", 10)?,
            create_test_violation("no-todo", "src/main.rs", "src", 20)?,
        ];

        let result = aggregator.aggregate(violations);

        assert!(result.passed);
        assert_eq!(result.total_violations, 2);
        assert_eq!(result.violations_over_budget, 0);
        assert_eq!(result.statuses.len(), 2);

        // Statuses should be sorted by rule_id
        assert_eq!(result.statuses[0].rule_id.as_str(), "no-todo");
        assert_eq!(result.statuses[1].rule_id.as_str(), "no-unwrap");
        Ok(())
    }

    #[test]
    fn test_aggregator_multiple_regions_same_rule() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            5,
        );
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("tests"),
            10,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![
            create_test_violation("no-unwrap", "src/main.rs", "src", 10)?,
            create_test_violation("no-unwrap", "tests/test.rs", "tests", 20)?,
        ];

        let result = aggregator.aggregate(violations);

        assert!(result.passed);
        assert_eq!(result.total_violations, 2);
        assert_eq!(result.violations_over_budget, 0);
        assert_eq!(result.statuses.len(), 2);

        // Statuses should be sorted by region within same rule
        assert_eq!(result.statuses[0].region.as_str(), "src");
        assert_eq!(result.statuses[1].region.as_str(), "tests");
        Ok(())
    }

    #[test]
    fn test_aggregator_zero_budget_strict_enforcement() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            0,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![create_test_violation(
            "no-unwrap",
            "src/main.rs",
            "src",
            10,
        )?];

        let result = aggregator.aggregate(violations);

        assert!(!result.passed);
        assert_eq!(result.total_violations, 1);
        assert_eq!(result.violations_over_budget, 1);
        Ok(())
    }

    #[test]
    fn test_aggregator_missing_rule_defaults_to_zero() -> Result<(), Box<dyn std::error::Error>> {
        let counts = CountsManager::new(); // Empty counts, no rules configured
        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![create_test_violation(
            "no-unwrap",
            "src/main.rs",
            "src",
            10,
        )?];

        let result = aggregator.aggregate(violations);

        // Missing rule should default to 0 budget (strict enforcement)
        assert!(!result.passed);
        assert_eq!(result.total_violations, 1);
        assert_eq!(result.violations_over_budget, 1);

        let status = &result.statuses[0];
        assert_eq!(status.budget, 0);
        assert!(!status.passed);
        Ok(())
    }

    #[test]
    fn test_aggregator_complex_scenario() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("."),
            0,
        );
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src/legacy"),
            15,
        );
        counts.set_count(
            &RuleId::new("no-todo").ok_or("invalid rule id")?,
            &RegionPath::new("."),
            0,
        );
        counts.set_count(
            &RuleId::new("no-todo").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            10,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![
            // 2 violations in src/legacy for no-unwrap (budget: 15) - PASS
            create_test_violation("no-unwrap", "src/legacy/old.rs", "src/legacy", 10)?,
            create_test_violation("no-unwrap", "src/legacy/old.rs", "src/legacy", 20)?,
            // 1 violation in src for no-unwrap (inherits root budget: 0) - FAIL
            create_test_violation("no-unwrap", "src/main.rs", "src", 30)?,
            // 3 violations in src for no-todo (budget: 10) - PASS
            create_test_violation("no-todo", "src/main.rs", "src", 40)?,
            create_test_violation("no-todo", "src/lib.rs", "src", 50)?,
            create_test_violation("no-todo", "src/util.rs", "src", 60)?,
        ];

        let result = aggregator.aggregate(violations);

        // Should fail because one group (no-unwrap in src) is over budget
        assert!(!result.passed);
        assert_eq!(result.total_violations, 6);
        assert_eq!(result.violations_over_budget, 1); // Only the src/no-unwrap violation

        assert_eq!(result.statuses.len(), 3);

        // Find specific statuses (sorted by rule_id, then region)
        let no_todo_src = result
            .statuses
            .iter()
            .find(|s| s.rule_id.as_str() == "no-todo" && s.region.as_str() == "src")
            .ok_or("missing no-todo src status")?;
        assert_eq!(no_todo_src.actual_count, 3);
        assert_eq!(no_todo_src.budget, 10);
        assert!(no_todo_src.passed);

        let no_unwrap_legacy = result
            .statuses
            .iter()
            .find(|s| s.rule_id.as_str() == "no-unwrap" && s.region.as_str() == "src/legacy")
            .ok_or("missing no-unwrap src/legacy status")?;
        assert_eq!(no_unwrap_legacy.actual_count, 2);
        assert_eq!(no_unwrap_legacy.budget, 15);
        assert!(no_unwrap_legacy.passed);

        let no_unwrap_src = result
            .statuses
            .iter()
            .find(|s| s.rule_id.as_str() == "no-unwrap" && s.region.as_str() == "src")
            .ok_or("missing no-unwrap src status")?;
        assert_eq!(no_unwrap_src.actual_count, 1);
        assert_eq!(no_unwrap_src.budget, 0);
        assert!(!no_unwrap_src.passed);
        Ok(())
    }

    #[test]
    fn test_aggregator_exact_budget_match() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            3,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![
            create_test_violation("no-unwrap", "src/a.rs", "src", 10)?,
            create_test_violation("no-unwrap", "src/b.rs", "src", 20)?,
            create_test_violation("no-unwrap", "src/c.rs", "src", 30)?,
        ];

        let result = aggregator.aggregate(violations);

        // Exact match should pass (3 <= 3)
        assert!(result.passed);
        assert_eq!(result.total_violations, 3);
        assert_eq!(result.violations_over_budget, 0);

        let status = &result.statuses[0];
        assert_eq!(status.actual_count, 3);
        assert_eq!(status.budget, 3);
        assert!(status.passed);
        Ok(())
    }

    #[test]
    fn test_aggregator_sorted_output() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("rule-a").ok_or("invalid rule id")?,
            &RegionPath::new("z-region"),
            10,
        );
        counts.set_count(
            &RuleId::new("rule-b").ok_or("invalid rule id")?,
            &RegionPath::new("a-region"),
            10,
        );
        counts.set_count(
            &RuleId::new("rule-a").ok_or("invalid rule id")?,
            &RegionPath::new("a-region"),
            10,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![
            create_test_violation("rule-b", "a-region/file.rs", "a-region", 10)?,
            create_test_violation("rule-a", "z-region/file.rs", "z-region", 20)?,
            create_test_violation("rule-a", "a-region/file.rs", "a-region", 30)?,
        ];

        let result = aggregator.aggregate(violations);

        // Verify sorting: first by rule_id, then by region
        assert_eq!(result.statuses.len(), 3);
        assert_eq!(result.statuses[0].rule_id.as_str(), "rule-a");
        assert_eq!(result.statuses[0].region.as_str(), "a-region");
        assert_eq!(result.statuses[1].rule_id.as_str(), "rule-a");
        assert_eq!(result.statuses[1].region.as_str(), "z-region");
        assert_eq!(result.statuses[2].rule_id.as_str(), "rule-b");
        assert_eq!(result.statuses[2].region.as_str(), "a-region");
        Ok(())
    }

    #[test]
    fn test_aggregator_preserves_violation_details() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        counts.set_count(
            &RuleId::new("no-unwrap").ok_or("invalid rule id")?,
            &RegionPath::new("src"),
            5,
        );

        let aggregator = ViolationAggregator::new(counts);

        let violations = vec![
            create_test_violation("no-unwrap", "src/main.rs", "src", 10)?,
            create_test_violation("no-unwrap", "src/lib.rs", "src", 20)?,
        ];

        let result = aggregator.aggregate(violations);

        let status = &result.statuses[0];
        assert_eq!(status.violations.len(), 2);
        assert_eq!(status.violations[0].file, PathBuf::from("src/main.rs"));
        assert_eq!(status.violations[0].line, 10);
        assert_eq!(status.violations[1].file, PathBuf::from("src/lib.rs"));
        assert_eq!(status.violations[1].line, 20);
        Ok(())
    }

    #[test]
    fn test_aggregation_result_derives() {
        let result = AggregationResult {
            statuses: vec![],
            passed: true,
            total_violations: 0,
            violations_over_budget: 0,
        };

        // Test clone
        let cloned = result.clone();
        assert_eq!(result, cloned);

        // Test debug
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("AggregationResult"));
    }

    #[test]
    fn test_rule_region_status_derives() -> Result<(), Box<dyn std::error::Error>> {
        let status = RuleRegionStatus {
            rule_id: RuleId::new("test").ok_or("invalid rule id")?,
            region: RegionPath::new("src"),
            actual_count: 5,
            budget: 10,
            passed: true,
            violations: vec![],
        };

        // Test clone
        let cloned = status.clone();
        assert_eq!(status, cloned);

        // Test debug
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("RuleRegionStatus"));
        Ok(())
    }
}
