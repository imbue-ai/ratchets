//! Git merge driver for ratchet-counts.toml.
//!
//! Merges ratchet-counts.toml files using a "minimum wins" strategy: because
//! ratchets only tighten, when two branches both reduce a count the smaller
//! value is kept so neither reduction is lost.

use crate::config::counts::CountsManager;
use crate::types::{RegionPath, RuleId};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Exit codes for merge driver
const EXIT_SUCCESS: i32 = 0;
const EXIT_ERROR: i32 = 1;

/// Run the merge driver for ratchet-counts.toml
///
/// This is called by git during a merge when configured as a merge driver.
/// Git passes three file paths:
/// - base: The common ancestor version (%O)
/// - ours: The current branch version (%A)
/// - theirs: The other branch version (%B)
///
/// The merge result is written to the "ours" file path.
///
/// # Arguments
///
/// * `base` - Path to the base/ancestor version
/// * `ours` - Path to our version (will be overwritten with merge result)
/// * `theirs` - Path to their version
///
/// # Returns
///
/// Exit code:
/// - 0: Success (merge completed)
/// - 1: Error (parse failure or I/O error)
pub fn run_merge_driver(base: &str, ours: &str, theirs: &str) -> i32 {
    match run_merge_driver_inner(base, ours, theirs) {
        Ok(()) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Merge driver error: {}", e);
            EXIT_ERROR
        }
    }
}

/// Internal implementation of merge driver
fn run_merge_driver_inner(base: &str, ours: &str, theirs: &str) -> Result<(), String> {
    // Parse all three versions
    let base_counts = parse_counts_file(base, "base")?;
    let ours_counts = parse_counts_file(ours, "ours")?;
    let theirs_counts = parse_counts_file(theirs, "theirs")?;

    // Perform the merge
    let merged = merge_counts(&base_counts, &ours_counts, &theirs_counts);

    // Write the merged result to the ours file
    write_counts_file(ours, &merged)?;

    Ok(())
}

/// Parse a counts file, treating missing or empty files as empty CountsManager
fn parse_counts_file(path: &str, label: &str) -> Result<CountsManager, String> {
    let path_obj = Path::new(path);

    // If file doesn't exist, treat as empty
    if !path_obj.exists() {
        return Ok(CountsManager::new());
    }

    // Read and parse the file
    match CountsManager::load(path_obj) {
        Ok(counts) => Ok(counts),
        Err(e) => Err(format!("Failed to parse {} file '{}': {}", label, path, e)),
    }
}

/// Write a CountsManager to a file
fn write_counts_file(path: &str, counts: &CountsManager) -> Result<(), String> {
    let toml_string = counts.to_toml_string();

    std::fs::write(path, toml_string)
        .map_err(|e| format!("Failed to write merged counts to '{}': {}", path, e))
}

/// Merge three versions of counts using "minimum wins" strategy
///
/// For each (rule_id, region) combination:
/// - If present in both ours and theirs: take minimum
/// - If present in only one: use that value
/// - If present in neither: skip (not in merged result)
///
/// The base version is currently not used in the merge logic, but is accepted
/// for potential future three-way merge strategies.
fn merge_counts(
    _base: &CountsManager,
    ours: &CountsManager,
    theirs: &CountsManager,
) -> CountsManager {
    let mut merged = CountsManager::new();

    let ours_counts = extract_all_counts(ours);
    let theirs_counts = extract_all_counts(theirs);

    let mut ours_map: HashMap<(String, String), u64> = HashMap::new();
    for (rule_id, region, count) in &ours_counts {
        ours_map.insert(
            (rule_id.as_str().to_string(), region.as_str().to_string()),
            *count,
        );
    }

    let mut theirs_map: HashMap<(String, String), u64> = HashMap::new();
    for (rule_id, region, count) in &theirs_counts {
        theirs_map.insert(
            (rule_id.as_str().to_string(), region.as_str().to_string()),
            *count,
        );
    }

    let mut all_keys: HashSet<(String, String)> = HashSet::new();
    all_keys.extend(ours_map.keys().cloned());
    all_keys.extend(theirs_map.keys().cloned());

    // Take the minimum of the two values, or the only value if present in one side.
    for (rule_id_str, region_str) in all_keys {
        let ours_count = ours_map.get(&(rule_id_str.clone(), region_str.clone()));
        let theirs_count = theirs_map.get(&(rule_id_str.clone(), region_str.clone()));

        let final_count = match (ours_count, theirs_count) {
            (Some(&o), Some(&t)) => std::cmp::min(o, t),
            (Some(&o), None) => o,
            (None, Some(&t)) => t,
            (None, None) => continue, // Should never happen
        };

        if let Some(rule_id) = RuleId::new(&rule_id_str) {
            let region = RegionPath::new(region_str);
            merged.set_count(&rule_id, &region, final_count);
        }
    }

    merged
}

/// Extract all (rule_id, region, count) tuples from a CountsManager.
///
/// CountsManager has no public iterator, so we round-trip through TOML to read
/// back its rule/region/count entries.
fn extract_all_counts(counts: &CountsManager) -> Vec<(RuleId, RegionPath, u64)> {
    let mut result = Vec::new();

    let toml_str = counts.to_toml_string();

    if let Ok(parsed) = toml::from_str::<toml::Value>(&toml_str)
        && let toml::Value::Table(table) = parsed
    {
        for (rule_id_str, value) in table {
            if let Some(rule_id) = RuleId::new(&rule_id_str)
                && let toml::Value::Table(regions) = value
            {
                for (region_str, count_value) in regions {
                    let region = RegionPath::new(region_str);
                    if let Some(count) = count_value.as_integer()
                        && count >= 0
                    {
                        result.push((rule_id.clone(), region, count as u64));
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test file with given TOML content
    fn create_test_file(
        dir: &TempDir,
        name: &str,
        content: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let path = dir.path().join(name);
        fs::write(&path, content)?;
        Ok(path.to_string_lossy().to_string())
    }

    #[test]
    fn test_merge_driver_basic() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let base = create_test_file(
            &temp_dir,
            "base.toml",
            r#"
[no-unwrap]
"." = 20
"#,
        )?;

        let ours = create_test_file(
            &temp_dir,
            "ours.toml",
            r#"
[no-unwrap]
"." = 15
"#,
        )?;

        let theirs = create_test_file(
            &temp_dir,
            "theirs.toml",
            r#"
[no-unwrap]
"." = 18
"#,
        )?;

        let result = run_merge_driver(&base, &ours, &theirs);
        assert_eq!(result, EXIT_SUCCESS);

        let merged_content = fs::read_to_string(&ours)?;
        let merged_counts = CountsManager::parse(&merged_content)?;

        let rule_id = RuleId::new("no-unwrap").ok_or("invalid rule id")?;
        assert_eq!(merged_counts.get_budget(&rule_id, Path::new(".")), 15);
        Ok(())
    }

    #[test]
    fn test_merge_driver_new_rule_in_ours() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let base = create_test_file(&temp_dir, "base.toml", "")?;

        let ours = create_test_file(
            &temp_dir,
            "ours.toml",
            r#"
[no-unwrap]
"." = 10
"#,
        )?;

        let theirs = create_test_file(&temp_dir, "theirs.toml", "")?;

        let result = run_merge_driver(&base, &ours, &theirs);
        assert_eq!(result, EXIT_SUCCESS);

        let merged_content = fs::read_to_string(&ours)?;
        let merged_counts = CountsManager::parse(&merged_content)?;

        let rule_id = RuleId::new("no-unwrap").ok_or("invalid rule id")?;
        assert_eq!(merged_counts.get_budget(&rule_id, Path::new(".")), 10);
        Ok(())
    }

    #[test]
    fn test_merge_driver_new_rule_in_theirs() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let base = create_test_file(&temp_dir, "base.toml", "")?;

        let ours = create_test_file(&temp_dir, "ours.toml", "")?;

        let theirs = create_test_file(
            &temp_dir,
            "theirs.toml",
            r#"
[no-todo]
"." = 5
"#,
        )?;

        let result = run_merge_driver(&base, &ours, &theirs);
        assert_eq!(result, EXIT_SUCCESS);

        let merged_content = fs::read_to_string(&ours)?;
        let merged_counts = CountsManager::parse(&merged_content)?;

        let rule_id = RuleId::new("no-todo").ok_or("invalid rule id")?;
        assert_eq!(merged_counts.get_budget(&rule_id, Path::new(".")), 5);
        Ok(())
    }

    #[test]
    fn test_merge_driver_multiple_rules() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let base = create_test_file(
            &temp_dir,
            "base.toml",
            r#"
[no-unwrap]
"." = 20
[no-todo]
"." = 30
"#,
        )?;

        let ours = create_test_file(
            &temp_dir,
            "ours.toml",
            r#"
[no-unwrap]
"." = 15
[no-todo]
"." = 30
"#,
        )?;

        let theirs = create_test_file(
            &temp_dir,
            "theirs.toml",
            r#"
[no-unwrap]
"." = 18
[no-todo]
"." = 25
"#,
        )?;

        let result = run_merge_driver(&base, &ours, &theirs);
        assert_eq!(result, EXIT_SUCCESS);

        let merged_content = fs::read_to_string(&ours)?;
        let merged_counts = CountsManager::parse(&merged_content)?;

        let no_unwrap = RuleId::new("no-unwrap").ok_or("invalid rule id")?;
        let no_todo = RuleId::new("no-todo").ok_or("invalid rule id")?;

        assert_eq!(merged_counts.get_budget(&no_unwrap, Path::new(".")), 15);
        assert_eq!(merged_counts.get_budget(&no_todo, Path::new(".")), 25);
        Ok(())
    }

    #[test]
    fn test_merge_driver_multiple_regions() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let base = create_test_file(
            &temp_dir,
            "base.toml",
            r#"
[no-unwrap]
"." = 20
"src" = 15
"#,
        )?;

        let ours = create_test_file(
            &temp_dir,
            "ours.toml",
            r#"
[no-unwrap]
"." = 18
"src" = 10
"#,
        )?;

        let theirs = create_test_file(
            &temp_dir,
            "theirs.toml",
            r#"
[no-unwrap]
"." = 19
"src" = 12
"#,
        )?;

        let result = run_merge_driver(&base, &ours, &theirs);
        assert_eq!(result, EXIT_SUCCESS);

        let merged_content = fs::read_to_string(&ours)?;
        let merged_counts = CountsManager::parse(&merged_content)?;

        let rule_id = RuleId::new("no-unwrap").ok_or("invalid rule id")?;
        assert_eq!(merged_counts.get_budget(&rule_id, Path::new(".")), 18);
        assert_eq!(
            merged_counts.get_budget(&rule_id, Path::new("src/file.rs")),
            10
        );
        Ok(())
    }

    #[test]
    fn test_merge_driver_missing_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let base = temp_dir.path().join("nonexistent_base.toml");
        let ours = create_test_file(
            &temp_dir,
            "ours.toml",
            r#"
[no-unwrap]
"." = 10
"#,
        )?;
        let theirs = temp_dir.path().join("nonexistent_theirs.toml");

        let base = base.to_str().ok_or("base path is not valid UTF-8")?;
        let theirs = theirs.to_str().ok_or("theirs path is not valid UTF-8")?;
        let result = run_merge_driver(base, &ours, theirs);
        assert_eq!(result, EXIT_SUCCESS);

        let merged_content = fs::read_to_string(&ours)?;
        let merged_counts = CountsManager::parse(&merged_content)?;

        let rule_id = RuleId::new("no-unwrap").ok_or("invalid rule id")?;
        assert_eq!(merged_counts.get_budget(&rule_id, Path::new(".")), 10);
        Ok(())
    }

    #[test]
    fn test_merge_driver_invalid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let base = create_test_file(&temp_dir, "base.toml", "")?;
        let ours = create_test_file(&temp_dir, "ours.toml", "invalid [[ toml")?;
        let theirs = create_test_file(&temp_dir, "theirs.toml", "")?;

        let result = run_merge_driver(&base, &ours, &theirs);
        assert_eq!(result, EXIT_ERROR);
        Ok(())
    }

    #[test]
    fn test_extract_all_counts() -> Result<(), Box<dyn std::error::Error>> {
        let mut counts = CountsManager::new();
        let rule1 = RuleId::new("rule1").ok_or("invalid rule id")?;
        let rule2 = RuleId::new("rule2").ok_or("invalid rule id")?;

        counts.set_count(&rule1, &RegionPath::new("."), 10);
        counts.set_count(&rule1, &RegionPath::new("src"), 5);
        counts.set_count(&rule2, &RegionPath::new("."), 20);

        let extracted = extract_all_counts(&counts);

        assert_eq!(extracted.len(), 3);

        let has_rule1_root = extracted
            .iter()
            .any(|(r, p, c)| r.as_str() == "rule1" && p.as_str() == "." && *c == 10);
        let has_rule1_src = extracted
            .iter()
            .any(|(r, p, c)| r.as_str() == "rule1" && p.as_str() == "src" && *c == 5);
        let has_rule2_root = extracted
            .iter()
            .any(|(r, p, c)| r.as_str() == "rule2" && p.as_str() == "." && *c == 20);

        assert!(has_rule1_root);
        assert!(has_rule1_src);
        assert!(has_rule2_root);
        Ok(())
    }

    #[test]
    fn test_merge_counts_minimum_wins() -> Result<(), Box<dyn std::error::Error>> {
        let mut base = CountsManager::new();
        let mut ours = CountsManager::new();
        let mut theirs = CountsManager::new();

        let rule_id = RuleId::new("no-unwrap").ok_or("invalid rule id")?;
        let region = RegionPath::new(".");

        base.set_count(&rule_id, &region, 20);
        ours.set_count(&rule_id, &region, 15);
        theirs.set_count(&rule_id, &region, 18);

        let merged = merge_counts(&base, &ours, &theirs);

        assert_eq!(merged.get_budget(&rule_id, Path::new(".")), 15);
        Ok(())
    }

    #[test]
    fn test_merge_counts_only_in_ours() -> Result<(), Box<dyn std::error::Error>> {
        let base = CountsManager::new();
        let mut ours = CountsManager::new();
        let theirs = CountsManager::new();

        let rule_id = RuleId::new("no-unwrap").ok_or("invalid rule id")?;
        let region = RegionPath::new(".");

        ours.set_count(&rule_id, &region, 10);

        let merged = merge_counts(&base, &ours, &theirs);

        assert_eq!(merged.get_budget(&rule_id, Path::new(".")), 10);
        Ok(())
    }

    #[test]
    fn test_merge_counts_only_in_theirs() -> Result<(), Box<dyn std::error::Error>> {
        let base = CountsManager::new();
        let ours = CountsManager::new();
        let mut theirs = CountsManager::new();

        let rule_id = RuleId::new("no-unwrap").ok_or("invalid rule id")?;
        let region = RegionPath::new(".");

        theirs.set_count(&rule_id, &region, 10);

        let merged = merge_counts(&base, &ours, &theirs);

        assert_eq!(merged.get_budget(&rule_id, Path::new(".")), 10);
        Ok(())
    }

    #[test]
    fn test_merge_counts_complex() -> Result<(), Box<dyn std::error::Error>> {
        let mut base = CountsManager::new();
        let mut ours = CountsManager::new();
        let mut theirs = CountsManager::new();

        let rule1 = RuleId::new("rule1").ok_or("invalid rule id")?;
        let rule2 = RuleId::new("rule2").ok_or("invalid rule id")?;
        let rule3 = RuleId::new("rule3").ok_or("invalid rule id")?;

        let root = RegionPath::new(".");
        let src = RegionPath::new("src");

        // Rule1: Both sides reduce from 20
        base.set_count(&rule1, &root, 20);
        ours.set_count(&rule1, &root, 15);
        theirs.set_count(&rule1, &root, 18);

        // Rule2: Only ours has it
        ours.set_count(&rule2, &root, 10);

        // Rule3: Only theirs has it
        theirs.set_count(&rule3, &src, 5);

        let merged = merge_counts(&base, &ours, &theirs);

        assert_eq!(merged.get_budget(&rule1, Path::new(".")), 15);
        assert_eq!(merged.get_budget(&rule2, Path::new(".")), 10);
        assert_eq!(merged.get_budget(&rule3, Path::new("src/file.rs")), 5);
        Ok(())
    }
}
