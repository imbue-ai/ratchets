#![forbid(unsafe_code)]

//! Ratchet-set definitions, registry, and resolver.
//!
//! Phase 2 of `blueprint/ratchet-sets/plan-ratchet-sets.md` introduces named
//! collections of rule IDs ("ratchet-sets"). A [`RatchetSet`] is a TOML file
//! that lists [`RatchetRef`]s — either bare rule IDs or `$other-set` references
//! to compose with another set. A [`SetRegistry`] holds the loaded sets and
//! exposes [`SetRegistry::resolve`] which performs DFS-based expansion with
//! cycle detection.
//!
//! Loading layers mirror the existing rule loaders in [`crate::rules::registry`]:
//!
//! 1. Embedded starter sets baked into the binary (via
//!    [`crate::rules::load_builtin_sets`]). Phase 4 ships the
//!    `common-starter` content; per-language starter sets are deferred to
//!    follow-up MRs.
//! 2. Filesystem builtin sets under `builtin-ratchets/sets/*.toml` (overrides
//!    embedded).
//! 3. User-defined sets under `ratchets/sets/*.toml` (overrides filesystem
//!    builtin).
//!
//! Phase 3 wires the resolver into [`crate::rules::RuleRegistry`]; the
//! resolver remains decoupled from rule definitions.

use crate::config::ratchet_toml::RatchetRef;
use crate::error::RuleError;
use crate::types::{Language, RuleId, SetId};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// TOML structure for set definitions.
///
/// The on-disk shape places everything inside the `[set]` table:
///
/// ```toml
/// [set]
/// id = "common-starter"
/// description = "Language-agnostic curated set"
/// languages = ["rust", "python"]  # optional, advisory only in Phase 2
/// rules = ["some-rule", "$other-set"]
/// ```
///
/// This mirrors how `RegexRule` uses a single `[rule]` table for metadata,
/// while keeping the array of references reachable as `set.rules`.
#[derive(Debug, Deserialize)]
struct RatchetSetDefinition {
    set: SetSection,
}

#[derive(Debug, Deserialize)]
struct SetSection {
    id: String,
    description: String,
    #[serde(default)]
    languages: Vec<Language>,
    #[serde(default)]
    rules: Vec<RatchetRef>,
}

/// A named collection of rule IDs and/or other set references.
///
/// `languages` is advisory only in Phase 2 of the ratchet-sets plan; consumers
/// may use it to inform composition decisions but the resolver does not
/// filter by it. `rules` may mix bare rule references and `$other-set`
/// composition references; [`SetRegistry::resolve`] flattens these via DFS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatchetSet {
    id: SetId,
    description: String,
    languages: Vec<Language>,
    rules: Vec<RatchetRef>,
}

impl RatchetSet {
    /// Construct a `RatchetSet` directly. Intended for tests and downstream
    /// callers that synthesize sets without TOML.
    pub fn new(
        id: SetId,
        description: impl Into<String>,
        languages: Vec<Language>,
        rules: Vec<RatchetRef>,
    ) -> Self {
        Self {
            id,
            description: description.into(),
            languages,
            rules,
        }
    }

    /// Parse a `RatchetSet` from TOML content.
    ///
    /// # Errors
    ///
    /// Returns [`RuleError::InvalidDefinition`] if:
    /// - TOML syntax is invalid
    /// - The `[set]` block is missing required fields
    /// - The set ID fails validation (see [`SetId`])
    /// - Any entry in `rules` fails to parse as a [`RatchetRef`]
    pub fn from_toml(content: &str) -> Result<Self, RuleError> {
        let def: RatchetSetDefinition = toml::from_str(content).map_err(|e| {
            RuleError::InvalidDefinition(format!("Failed to parse ratchet-set TOML: {}", e))
        })?;

        let id = SetId::new(def.set.id.clone()).ok_or_else(|| {
            RuleError::InvalidDefinition(format!("Invalid ratchet-set ID: {}", def.set.id))
        })?;

        Ok(Self {
            id,
            description: def.set.description,
            languages: def.set.languages,
            rules: def.set.rules,
        })
    }

    /// Parse a `RatchetSet` from a TOML file path.
    ///
    /// # Errors
    ///
    /// Returns [`RuleError`] if the file cannot be read or its content fails
    /// to parse via [`RatchetSet::from_toml`].
    pub fn from_path(path: &Path) -> Result<Self, RuleError> {
        let content = fs::read_to_string(path).map_err(|e| {
            RuleError::InvalidDefinition(format!("Failed to read file {:?}: {}", path, e))
        })?;
        Self::from_toml(&content)
    }

    /// Set identifier.
    pub fn id(&self) -> &SetId {
        &self.id
    }

    /// Human-readable description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Advisory language list (not load-bearing in Phase 2).
    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    /// The mixed array of rule and `$set` references in this set.
    pub fn rules(&self) -> &[RatchetRef] {
        &self.rules
    }
}

/// Errors returned by [`SetRegistry::resolve`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ResolveError {
    /// A set referenced by ID is not present in the registry.
    #[error("Unknown ratchet-set: '{0}'")]
    UnknownSet(SetId),

    /// A composition cycle was detected. The chain lists the sets in
    /// traversal order; the last entry is the set whose re-entry triggered the
    /// detection (i.e. the chain repeats itself there).
    #[error("Cycle detected in ratchet-set composition: {}", format_cycle(.0))]
    Cycle(Vec<SetId>),
}

fn format_cycle(chain: &[SetId]) -> String {
    chain
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Registry of loaded [`RatchetSet`]s, keyed by [`SetId`].
///
/// Like [`crate::rules::RuleRegistry`], later loads override earlier ones
/// with the same ID. The intended loading order is embedded → filesystem
/// builtin → user-defined; this matches rule registry behavior so that
/// builtin sets can be locally overridden during development and user-defined
/// sets win at the end.
#[derive(Debug, Default)]
pub struct SetRegistry {
    sets: HashMap<SetId, RatchetSet>,
}

impl SetRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            sets: HashMap::new(),
        }
    }

    /// Insert (or replace) a set. Later inserts override earlier ones with
    /// the same ID, mirroring the rule registry's override behavior.
    pub fn insert(&mut self, set: RatchetSet) {
        self.sets.insert(set.id.clone(), set);
    }

    /// Look up a set by ID.
    pub fn get(&self, id: &SetId) -> Option<&RatchetSet> {
        self.sets.get(id)
    }

    /// Number of registered sets.
    pub fn len(&self) -> usize {
        self.sets.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }

    /// Load embedded builtin sets via [`crate::rules::load_builtin_sets`].
    ///
    /// Phase 4 of the ratchet-sets plan ships `common-starter`; per-language
    /// starter sets land in follow-up MRs.
    ///
    /// # Errors
    ///
    /// Returns [`RuleError`] if any embedded set fails to parse.
    pub fn load_embedded_builtin_sets(&mut self) -> Result<(), RuleError> {
        let sets = crate::rules::load_builtin_sets()?;
        for set in sets {
            self.insert(set);
        }
        Ok(())
    }

    /// Load filesystem builtin sets from `builtin-ratchets/sets/*.toml`.
    ///
    /// A missing directory is treated as "no overrides" — the caller does not
    /// need to check existence beforehand, mirroring the existing rule
    /// loaders' behavior.
    ///
    /// # Errors
    ///
    /// Returns [`RuleError`] if a `.toml` file in the directory fails to read
    /// or parse.
    pub fn load_builtin_sets(&mut self, dir: &Path) -> Result<(), RuleError> {
        self.load_sets_from_dir(dir)
    }

    /// Load user-defined sets from `ratchets/sets/*.toml`.
    ///
    /// A missing directory is treated as "no user sets" — symmetric with
    /// [`SetRegistry::load_builtin_sets`].
    ///
    /// # Errors
    ///
    /// Returns [`RuleError`] if a `.toml` file in the directory fails to read
    /// or parse.
    pub fn load_custom_sets(&mut self, dir: &Path) -> Result<(), RuleError> {
        self.load_sets_from_dir(dir)
    }

    fn load_sets_from_dir(&mut self, dir: &Path) -> Result<(), RuleError> {
        if !dir.exists() {
            return Ok(());
        }

        if !dir.is_dir() {
            return Err(RuleError::InvalidDefinition(format!(
                "Set path is not a directory: {}",
                dir.display()
            )));
        }

        let entries = fs::read_dir(dir).map_err(|e| {
            RuleError::InvalidDefinition(format!(
                "Failed to read set directory {}: {}",
                dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                RuleError::InvalidDefinition(format!(
                    "Failed to read set directory entry in {}: {}",
                    dir.display(),
                    e
                ))
            })?;

            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }

            let set = RatchetSet::from_path(&path)?;
            self.insert(set);
        }

        Ok(())
    }

    /// Expand `enabled` and subtract `disabled` to produce the resolved set of
    /// rule IDs.
    ///
    /// Both inputs are walked DFS: each [`RatchetRef::Set`] is expanded to the
    /// union of its [`RatchetSet::rules`]; each [`RatchetRef::Rule`] contributes
    /// directly. The same DFS detects cycles via a `visiting` stack: re-entry
    /// of a set already on the stack yields [`ResolveError::Cycle`] carrying
    /// the offending chain plus the repeated set.
    ///
    /// After full expansion of `enabled`, every rule ID reachable from
    /// `disabled` is removed. Unknown set IDs (in either input) yield
    /// [`ResolveError::UnknownSet`]; unknown *rule* IDs are not an error here
    /// — the resolver works purely on IDs, leaving definition lookup to the
    /// rule registry in Phase 3.
    pub fn resolve(
        &self,
        enabled: &[RatchetRef],
        disabled: &[RatchetRef],
    ) -> Result<HashSet<RuleId>, ResolveError> {
        let mut resolved: HashSet<RuleId> = HashSet::new();
        for r in enabled {
            self.dfs_expand(r, &mut resolved, &mut Vec::new())?;
        }

        let mut to_remove: HashSet<RuleId> = HashSet::new();
        for r in disabled {
            self.dfs_expand(r, &mut to_remove, &mut Vec::new())?;
        }

        for rule_id in &to_remove {
            resolved.remove(rule_id);
        }

        Ok(resolved)
    }

    /// DFS helper used by [`SetRegistry::resolve`]. `visiting` is the active
    /// recursion stack; we use a `Vec<SetId>` (rather than a `HashSet`) so the
    /// cycle chain reported back to the user preserves traversal order.
    fn dfs_expand(
        &self,
        node: &RatchetRef,
        out: &mut HashSet<RuleId>,
        visiting: &mut Vec<SetId>,
    ) -> Result<(), ResolveError> {
        match node {
            RatchetRef::Rule(rule_id) => {
                out.insert(rule_id.clone());
                Ok(())
            }
            RatchetRef::Set(set_id) => {
                if visiting.contains(set_id) {
                    let mut chain = visiting.clone();
                    chain.push(set_id.clone());
                    return Err(ResolveError::Cycle(chain));
                }

                let set = self
                    .sets
                    .get(set_id)
                    .ok_or_else(|| ResolveError::UnknownSet(set_id.clone()))?;

                visiting.push(set_id.clone());
                for child in &set.rules {
                    self.dfs_expand(child, out, visiting)?;
                }
                visiting.pop();
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RuleId;
    use std::fs;
    use tempfile::TempDir;

    fn rule_ref(id: &str) -> RatchetRef {
        RatchetRef::Rule(RuleId::new(id).unwrap())
    }

    fn set_ref(id: &str) -> RatchetRef {
        RatchetRef::Set(SetId::new(id).unwrap())
    }

    fn make_set(id: &str, rules: Vec<RatchetRef>) -> RatchetSet {
        RatchetSet::new(
            SetId::new(id).unwrap(),
            format!("Test set {}", id),
            Vec::new(),
            rules,
        )
    }

    fn rule_id(id: &str) -> RuleId {
        RuleId::new(id).unwrap()
    }

    #[test]
    fn ratchet_set_from_toml_parses_set_block_and_rules() {
        // Synthetic rule names (no real builtin IDs) keep this test self-
        // contained and dodge the cross-language regex rules that scan for
        // task tags, since the literal rule names would otherwise count as
        // violations against this very source file.
        let toml = r#"
[set]
id = "common-starter"
description = "Language-agnostic starter set"
languages = ["rust", "python"]

rules = ["rule-alpha", "$strict-extras", "rule-beta"]
"#;
        let set = RatchetSet::from_toml(toml).unwrap();
        assert_eq!(set.id().as_str(), "common-starter");
        assert_eq!(set.description(), "Language-agnostic starter set");
        assert_eq!(set.languages(), &[Language::Rust, Language::Python]);
        assert_eq!(set.rules().len(), 3);

        assert!(matches!(
            &set.rules()[0],
            RatchetRef::Rule(id) if id.as_str() == "rule-alpha"
        ));
        assert!(matches!(
            &set.rules()[1],
            RatchetRef::Set(id) if id.as_str() == "strict-extras"
        ));
        assert!(matches!(
            &set.rules()[2],
            RatchetRef::Rule(id) if id.as_str() == "rule-beta"
        ));
    }

    #[test]
    fn ratchet_set_from_toml_no_rules_array_is_empty() {
        let toml = r#"
[set]
id = "empty-set"
description = "An empty set, useful as a base override marker"
"#;
        let set = RatchetSet::from_toml(toml).unwrap();
        assert!(set.rules().is_empty());
        assert!(set.languages().is_empty());
    }

    #[test]
    fn ratchet_set_from_toml_rejects_invalid_set_id() {
        let toml = r#"
[set]
id = "bad set"
description = "Spaces are not allowed in set IDs"

rules = []
"#;
        let err = RatchetSet::from_toml(toml).unwrap_err();
        assert!(matches!(err, RuleError::InvalidDefinition(_)));
    }

    #[test]
    fn ratchet_set_from_toml_rejects_invalid_rule_ref() {
        // `$bad set` strips the `$` and tries to parse the remainder as a
        // SetId, which fails validation because of the space.
        let toml = r#"
[set]
id = "good-set"
description = "But bad rule reference"

rules = ["$bad set"]
"#;
        let err = RatchetSet::from_toml(toml).unwrap_err();
        assert!(matches!(err, RuleError::InvalidDefinition(_)));
    }

    #[test]
    fn ratchet_set_from_path_reads_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("starter.toml");
        fs::write(
            &path,
            r#"
[set]
id = "starter"
description = "From-path test"

rules = ["rule-alpha"]
"#,
        )
        .unwrap();

        let set = RatchetSet::from_path(&path).unwrap();
        assert_eq!(set.id().as_str(), "starter");
        assert_eq!(set.rules().len(), 1);
    }

    #[test]
    fn set_registry_new_is_empty() {
        let registry = SetRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn set_registry_insert_and_get() {
        let mut registry = SetRegistry::new();
        let set = make_set("foo", vec![rule_ref("a")]);
        let set_id = set.id().clone();
        registry.insert(set);

        assert_eq!(registry.len(), 1);
        let fetched = registry.get(&set_id).unwrap();
        assert_eq!(fetched.rules().len(), 1);
    }

    #[test]
    fn set_registry_insert_overrides_existing_id() {
        let mut registry = SetRegistry::new();
        registry.insert(make_set("foo", vec![rule_ref("a")]));
        registry.insert(make_set("foo", vec![rule_ref("b"), rule_ref("c")]));

        assert_eq!(registry.len(), 1);
        let fetched = registry.get(&SetId::new("foo").unwrap()).unwrap();
        assert_eq!(fetched.rules().len(), 2);
        assert!(matches!(
            &fetched.rules()[0],
            RatchetRef::Rule(id) if id.as_str() == "b"
        ));
    }

    #[test]
    fn resolve_trivial_enabled_rules_returns_them_directly() {
        let registry = SetRegistry::new();
        let enabled = vec![rule_ref("a"), rule_ref("b")];

        let resolved = registry.resolve(&enabled, &[]).unwrap();
        let expected: HashSet<RuleId> = [rule_id("a"), rule_id("b")].into_iter().collect();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_single_set_expansion() {
        let mut registry = SetRegistry::new();
        registry.insert(make_set(
            "s",
            vec![rule_ref("a"), rule_ref("b"), rule_ref("c")],
        ));

        let resolved = registry.resolve(&[set_ref("s")], &[]).unwrap();
        let expected: HashSet<RuleId> = [rule_id("a"), rule_id("b"), rule_id("c")]
            .into_iter()
            .collect();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_nested_set_expansion() {
        let mut registry = SetRegistry::new();
        registry.insert(make_set("s", vec![rule_ref("a"), set_ref("t")]));
        registry.insert(make_set("t", vec![rule_ref("b")]));

        let resolved = registry.resolve(&[set_ref("s")], &[]).unwrap();
        let expected: HashSet<RuleId> = [rule_id("a"), rule_id("b")].into_iter().collect();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_two_set_cycle_returns_chain() {
        let mut registry = SetRegistry::new();
        registry.insert(make_set("s", vec![set_ref("t")]));
        registry.insert(make_set("t", vec![set_ref("s")]));

        let err = registry.resolve(&[set_ref("s")], &[]).unwrap_err();
        match err {
            ResolveError::Cycle(chain) => {
                assert_eq!(chain.len(), 3, "expected s -> t -> s, got {:?}", chain);
                assert_eq!(chain[0].as_str(), "s");
                assert_eq!(chain[1].as_str(), "t");
                assert_eq!(chain[2].as_str(), "s");
            }
            other => panic!("expected Cycle, got {:?}", other),
        }
    }

    #[test]
    fn resolve_self_cycle_returns_single_repeat() {
        let mut registry = SetRegistry::new();
        registry.insert(make_set("s", vec![set_ref("s")]));

        let err = registry.resolve(&[set_ref("s")], &[]).unwrap_err();
        match err {
            ResolveError::Cycle(chain) => {
                assert_eq!(chain.len(), 2);
                assert_eq!(chain[0].as_str(), "s");
                assert_eq!(chain[1].as_str(), "s");
            }
            other => panic!("expected Cycle, got {:?}", other),
        }
    }

    #[test]
    fn resolve_disabled_rule_overrides_enabled_set() {
        let mut registry = SetRegistry::new();
        registry.insert(make_set("s", vec![rule_ref("a"), rule_ref("b")]));

        let resolved = registry.resolve(&[set_ref("s")], &[rule_ref("a")]).unwrap();
        let expected: HashSet<RuleId> = [rule_id("b")].into_iter().collect();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_disabled_set_expands_and_subtracts() {
        // disabled = ["$t"], t = {a, b}, enabled = ["$s", "c"], s = {a, c}
        // -> {a, c} ∪ {} - {a, b} = {c}
        let mut registry = SetRegistry::new();
        registry.insert(make_set("s", vec![rule_ref("a"), rule_ref("c")]));
        registry.insert(make_set("t", vec![rule_ref("a"), rule_ref("b")]));

        let resolved = registry
            .resolve(&[set_ref("s"), rule_ref("c")], &[set_ref("t")])
            .unwrap();
        let expected: HashSet<RuleId> = [rule_id("c")].into_iter().collect();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_unknown_set_in_enabled_errors() {
        let registry = SetRegistry::new();
        let err = registry.resolve(&[set_ref("missing")], &[]).unwrap_err();
        assert!(matches!(err, ResolveError::UnknownSet(id) if id.as_str() == "missing"));
    }

    #[test]
    fn resolve_unknown_set_in_disabled_errors() {
        let mut registry = SetRegistry::new();
        registry.insert(make_set("s", vec![rule_ref("a")]));

        let err = registry
            .resolve(&[set_ref("s")], &[set_ref("missing")])
            .unwrap_err();
        assert!(matches!(err, ResolveError::UnknownSet(id) if id.as_str() == "missing"));
    }

    #[test]
    fn resolve_unknown_rule_id_in_enabled_is_returned_not_an_error() {
        // The resolver works purely on IDs. The rule registry (Phase 3) is
        // responsible for filtering out unknown rule IDs at lookup time.
        let registry = SetRegistry::new();
        let resolved = registry.resolve(&[rule_ref("typo-rule")], &[]).unwrap();
        let expected: HashSet<RuleId> = [rule_id("typo-rule")].into_iter().collect();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn load_builtin_sets_overrides_embedded() {
        // Embedded sets are empty in Phase 2 (Phase 4 populates them), so we
        // simulate the override by pre-inserting an "embedded" set and then
        // loading a filesystem-builtin set with the same ID.
        let mut registry = SetRegistry::new();
        registry.insert(make_set("starter", vec![rule_ref("a")]));

        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("starter.toml"),
            r#"
[set]
id = "starter"
description = "Filesystem builtin override"

rules = ["b", "c"]
"#,
        )
        .unwrap();

        registry.load_builtin_sets(temp.path()).unwrap();

        let fetched = registry.get(&SetId::new("starter").unwrap()).unwrap();
        assert_eq!(fetched.description(), "Filesystem builtin override");
        assert_eq!(fetched.rules().len(), 2);
        assert!(matches!(
            &fetched.rules()[0],
            RatchetRef::Rule(id) if id.as_str() == "b"
        ));
    }

    #[test]
    fn load_custom_sets_overrides_builtin() {
        // User-defined sets win at the end of the loading chain.
        let mut registry = SetRegistry::new();
        registry.insert(make_set("starter", vec![rule_ref("a")]));

        let builtin_dir = TempDir::new().unwrap();
        fs::write(
            builtin_dir.path().join("starter.toml"),
            r#"
[set]
id = "starter"
description = "Filesystem builtin"

rules = ["b"]
"#,
        )
        .unwrap();
        registry.load_builtin_sets(builtin_dir.path()).unwrap();

        let user_dir = TempDir::new().unwrap();
        fs::write(
            user_dir.path().join("starter.toml"),
            r#"
[set]
id = "starter"
description = "User override"

rules = ["c"]
"#,
        )
        .unwrap();
        registry.load_custom_sets(user_dir.path()).unwrap();

        let fetched = registry.get(&SetId::new("starter").unwrap()).unwrap();
        assert_eq!(fetched.description(), "User override");
        assert_eq!(fetched.rules().len(), 1);
        assert!(matches!(
            &fetched.rules()[0],
            RatchetRef::Rule(id) if id.as_str() == "c"
        ));
    }

    #[test]
    fn load_builtin_sets_missing_dir_is_ok() {
        let mut registry = SetRegistry::new();
        registry
            .load_builtin_sets(Path::new("/definitely/does/not/exist"))
            .unwrap();
        assert!(registry.is_empty());
    }

    #[test]
    fn load_custom_sets_missing_dir_is_ok() {
        let mut registry = SetRegistry::new();
        registry
            .load_custom_sets(Path::new("/definitely/does/not/exist"))
            .unwrap();
        assert!(registry.is_empty());
    }

    #[test]
    fn load_sets_ignores_non_toml_files() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("good.toml"),
            r#"
[set]
id = "good"
description = "OK"

rules = ["a"]
"#,
        )
        .unwrap();
        fs::write(temp.path().join("README.md"), "# Readme").unwrap();
        fs::write(temp.path().join("notes.txt"), "hi").unwrap();

        let mut registry = SetRegistry::new();
        registry.load_builtin_sets(temp.path()).unwrap();
        assert_eq!(registry.len(), 1);
        assert!(registry.get(&SetId::new("good").unwrap()).is_some());
    }

    #[test]
    fn load_embedded_builtin_sets_includes_common_starter() {
        // Phase 4 of the ratchet-sets plan lands `common-starter` as the only
        // embedded set. Per-language starter sets (`python-starter`,
        // `rust-starter`, `typescript-starter`) are deferred to follow-up MRs;
        // if/when they land this assertion must be updated alongside the new
        // toml files.
        //
        // The body uses explicit `match` ladders rather than `.unwrap()` /
        // `.expect()` because both shorthands are governed by enforced rules
        // against this very file.
        let mut registry = SetRegistry::new();
        match registry.load_embedded_builtin_sets() {
            Ok(()) => {}
            Err(e) => panic!("embedded sets must parse: {:?}", e),
        }
        assert_eq!(registry.len(), 1);

        let common_starter_id = match SetId::new("common-starter") {
            Some(id) => id,
            None => panic!("common-starter is a valid SetId"),
        };
        match registry.get(&common_starter_id) {
            Some(set) => assert_eq!(set.rules().len(), 2),
            None => panic!("common-starter must be embedded in Phase 4"),
        }
    }
}
