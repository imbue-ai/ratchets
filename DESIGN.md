# Ratchets Design Specification

## Overview

Ratchets is a progressive lint enforcement tool that allows codebases to contain existing violations while preventing new ones. Unlike traditional linters that enforce binary pass/fail, Ratchets permits a budgeted number of violations per rule per region. These budgets can only decrease over time (the "ratchet" mechanism), ensuring technical debt monotonically decreases.

## Core Concepts

### Rules

A **rule** is a pattern that matches undesirable code constructs. Rules are identified by a unique `rule-id` (e.g., `no-unwrap`, `no-todo-comments`).

Rules come in two forms:

1. **Regex rules**: Match text patterns in source files
2. **AST rules**: Tree-sitter queries that match syntactic structures

### Regions

A **region** is a directory path explicitly configured in `ratchet-counts.toml` for a specific rule. Regions are scoped per-rule: the same directory may be a region for one rule but not another.

Key principles:

- Regions exist **only** when explicitly listed in configuration
- The root region `"."` is always implicitly available
- Files in unconfigured directories are counted toward their nearest configured ancestor region
- The same directory path may be a region for some rules but not others (per-rule scoping)

Region paths are always relative to the repository root and use forward slashes (e.g., `src/parser`, `tests`).

### Counts

A **count** (or **budget**) is the maximum number of tolerated violations for a specific rule in a specific region. Counts are stored in version control and represent a contract: the code must not exceed these limits.

Semantics:
- Each configured region has its own explicit budget
- Files in unconfigured directories count toward their nearest configured ancestor region's budget
- The root region `"."` defaults to count `0` (no violations permitted) if not explicitly set
- A count of `0` means the rule is strictly enforced (no violations allowed in that region)

### The Ratchet Mechanism

The tool enforces monotonic improvement:

1. **Check**: Violations exceeding the budget fail the build
2. **Tighten**: Budgets can be reduced to match current (lower) violation counts
3. **Bump**: Budgets can be increased only by explicit human action with justification in the commit message

Agents and automated processes may tighten but never bump.

### Region Creation Policy

**Regions are created only by humans, never by ratchet commands.**

- `ratchets init`: Creates default configuration with only the root region `"."`
- `ratchets check`: Read-only; never modifies configuration
- `ratchets bump`: Updates budgets for existing regions only; fails if region doesn't exist
- `ratchets tighten`: Updates budgets for existing regions only; never adds new regions

To create a new region, a human must manually edit `ratchet-counts.toml` and add the region path as a key under the relevant rule section. This ensures that region structure is an intentional architectural decision, not an artifact of tool behavior.

## File Structure

A ratchets-enabled repository contains:

```
project/
├── ratchets.toml           # Configuration: enabled rules, languages, options
├── ratchet-counts.toml    # Violation budgets per rule per region
├── ratchets/              # Custom rule definitions
│   ├── regex/             # Custom regex rules (*.toml)
│   └── ast/               # Custom AST rules (*.toml with tree-sitter queries)
└── src/                   # Source code to be checked
```

### ratchets.toml

The configuration file specifies which rules are enabled and global settings.

```toml
# Ratchets configuration

[ratchets]
version = "1"

# Languages to analyze (determines which parsers to load)
languages = ["rust", "typescript", "python"]

# File patterns to include (glob syntax)
include = ["src/**", "tests/**"]

# File patterns to exclude (glob syntax)
exclude = ["**/generated/**", "**/vendor/**"]

[rules]
# Enable built-in rules by ID
# Values: true (enable with defaults), false (disable), or table (enable with options)

no-unwrap = true
no-expect = true
no-todo-comments = { severity = "warning" }
no-fixme-comments = false

[rules.custom]
# Enable custom rules from ratchets/ directory
# Reference by filename (without .toml extension)

my-company-rule = true
legacy-api-usage = { regions = ["src/legacy/**"] }

[output]
# Default output format: "human" or "jsonl"
format = "human"

# Colorize human output (auto-detected if not specified)
color = "auto"
```

### ratchet-counts.toml

The counts file stores violation budgets. Structure is `[rule-id.region-path]`.

```toml
# Ratchets violation budgets
# These counts represent the maximum tolerated violations.
# Counts can only be reduced (tightened) or explicitly bumped with justification.

[no-unwrap]
# Root default: 0 (inherited by all regions unless overridden)
"." = 0
"src/legacy" = 15
"src/legacy/parser" = 7
"tests" = 50

[no-todo-comments]
"." = 0
"src" = 23

[my-company-rule]
"src/experimental" = 5
```

**Region membership example**: For rule `no-unwrap` with configured regions `"."`, `"src/legacy"`, and `"src/legacy/parser"`:
- `src/foo/bar.rs` → belongs to region `"."` (no configured region matches `src/foo`) → budget 0
- `src/legacy/foo.rs` → belongs to region `"src/legacy"` → budget 15
- `src/legacy/parser/x.rs` → belongs to region `"src/legacy/parser"` → budget 7
- `src/legacy/parser/nested/deep.rs` → belongs to region `"src/legacy/parser"` (most specific match) → budget 7

Note: `tests/test.rs` would also belong to region `"."` since `"tests"` is not configured for this rule.

### Custom Rule Definitions

#### Regex Rules (`ratchets/regex/*.toml`)

```toml
[rule]
id = "no-console-log"
description = "Disallow console.log statements"
severity = "error"

[match]
# Regex pattern (Rust regex syntax)
pattern = "console\\.log\\s*\\("

# File types this rule applies to (optional, defaults to all)
languages = ["javascript", "typescript"]

# Additional file glob filter (optional)
include = ["src/**"]
exclude = ["src/debug/**"]
```

#### AST Rules (`ratchets/ast/*.toml`)

```toml
[rule]
id = "no-unwrap-custom"
description = "Disallow .unwrap() calls in production code"
severity = "error"

[match]
# Tree-sitter query (S-expression syntax)
# Captures are used for reporting location
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""

# Language this query applies to
language = "rust"

# File patterns
include = ["src/**"]
exclude = ["tests/**", "benches/**"]
```

## Commands

### `ratchets init`

Initialize a repository for use with Ratchets.

```
ratchets init [--force]
```

Behavior:
- Creates `ratchets.toml` with sensible defaults
- Creates empty `ratchet-counts.toml`
- Creates `ratchets/regex/` and `ratchets/ast/` directories
- If files exist: skip without `--force`, overwrite with `--force`
- Idempotent: safe to run multiple times

### `ratchets check`

Verify that the codebase complies with all enabled rules within budgets.

```
ratchets check [--format human|jsonl] [PATH...]
```

Behavior:
- Parses configuration and counts
- Loads necessary parsers (lazy: only languages present in matched files)
- Runs all enabled rules in parallel
- Aggregates violations per rule per region
- Compares against budgets
- Reports violations and budget status

Exit codes:
- `0`: All rules within budget
- `1`: At least one rule exceeded budget
- `2`: Configuration or usage error (invalid config, missing files, bad arguments)

### `ratchets bump <rule-id> [--region <path>] [--count <n>]`

Increase the violation budget for a rule.

```
ratchets bump no-unwrap --region src/legacy --count 20
ratchets bump no-unwrap --region src/legacy  # Auto-detect current count
```

Behavior:
- If `--count` provided: set budget to that value
- If `--count` omitted: run check for that rule/region, use current violation count
- Updates `ratchet-counts.toml`
- Fails if new count is lower than current violations (use `tighten` instead)
- **Never creates new regions**: the specified region must already exist in configuration

**Important**: Bumping should be accompanied by justification in the git commit message. This is a social contract, not enforced by the tool.

### `ratchets tighten [<rule-id>] [--region <path>]`

Reduce budgets to match current violation counts.

```
ratchets tighten                    # Tighten all rules, all regions
ratchets tighten no-unwrap          # Tighten specific rule, all regions
ratchets tighten --region src/      # Tighten all rules in region
```

Behavior:
- Runs check to get current violation counts
- For each **configured** rule/region: if current < budget, reduce budget to current
- Fails if any current > budget (violations exist beyond budget)
- Updates `ratchet-counts.toml`
- **Never creates new regions**: only updates budgets for regions already in configuration

### `ratchets merge-driver`

Git merge driver for `ratchet-counts.toml` that resolves conflicts by taking the minimum count.

```
# In .gitattributes:
ratchet-counts.toml merge=ratchets

# In .git/config or ~/.gitconfig:
[merge "ratchets"]
    name = Ratchets counts merge driver (minimum wins)
    driver = ratchets merge-driver %O %A %B
```

Behavior:
- Parses base (`%O`), ours (`%A`), and theirs (`%B`)
- For each rule/region: result = min(ours, theirs)
- Writes merged result to `%A`
- Exit `0` on success, non-zero on parse failure

### `ratchets list`

List all enabled rules and their current status.

```
ratchets list [--format human|jsonl]
```

Output includes:
- Rule ID
- Source (built-in, custom regex, custom AST)
- Languages
- Current violation count
- Budget
- Status (ok, exceeded, warning)

## Output Formats

### Human Format (default)

Designed for terminal display with optional color.

```
✗ no-unwrap: 4 violations (budget: 3) in src/legacy/parser
  src/legacy/parser/lexer.rs:42:10  .unwrap()
  src/legacy/parser/lexer.rs:87:15  .unwrap()
  src/legacy/parser/ast.rs:23:8     .unwrap()
  src/legacy/parser/ast.rs:156:12   .unwrap()

✓ no-todo-comments: 21 violations (budget: 23) in src

Summary: 1 rule exceeded budget, 1 rule within budget
```

### JSONL Format

Machine-readable output for agent consumption. One JSON object per line.

#### Violation Record

```json
{"type":"violation","rule":"no-unwrap","file":"src/legacy/parser/lexer.rs","line":42,"column":10,"end_line":42,"end_column":18,"snippet":".unwrap()","message":"Disallow .unwrap() calls","region":"src/legacy/parser"}
```

#### Summary Record

```json
{"type":"summary","rule":"no-unwrap","region":"src/legacy/parser","violations":4,"budget":3,"status":"exceeded"}
```

#### Final Status Record

```json
{"type":"status","passed":false,"rules_checked":2,"rules_exceeded":1,"total_violations":25}
```

### Output Schema (for evolvability)

All JSONL records include:
- `type`: Discriminator for record type
- `version`: Schema version (omitted = v1, future versions will include explicitly)

Reserved fields for future use:
- `severity`: warning, error, info
- `fix`: Suggested automatic fix
- `related`: Related violations or locations
- `metadata`: Rule-specific additional data

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success: all rules within budget |
| 1 | Failure: at least one rule exceeded budget |
| 2 | Error: configuration, usage, or I/O error |
| 3 | Error: parse failure (invalid source file for AST rule) |

Codes 4-127 are reserved for future use.

## Rule Registry

### Single Point of Interface

The **RuleRegistry** is the canonical interface for loading and managing rules in Ratchets. All rule loading in normal operation MUST go through the `RuleRegistry::build_from_config()` method. This single point of interface ensures:

- **Consistency**: All components load rules the same way
- **No duplicates**: Rules with the same ID are properly deduplicated through override behavior
- **Proper filtering**: Configuration-based and language-based filters are applied uniformly

### Loading Order

`RuleRegistry::build_from_config()` loads rules in a specific order that supports overrides:

1. **Embedded rules**: Built-in rules compiled into the binary (from `include_str!` macros)
2. **Filesystem builtin rules**: Rules from `builtin-ratchets/` directory (for development/overrides)
3. **Custom rules**: User-defined rules from `ratchets/` directory
4. **Config filter**: Remove disabled rules based on `ratchets.toml` settings
5. **Language filter**: Remove rules for unconfigured languages

Later rules override earlier rules with the same ID. This allows filesystem rules to override embedded rules, and custom rules to override builtin rules.

### Rule Structure

Rules are organized by type:

- **Regex rules**: Language-agnostic pattern matching in `common/regex/` and per-language `regex/` directories
- **AST rules**: Tree-sitter queries in per-language `ast/` directories

Builtin rules use a language-first directory structure:
```
builtin-ratchets/
├── common/regex/           # Language-agnostic regex rules
├── rust/ast/               # Rust AST rules
├── python/ast/             # Python AST rules
└── typescript/ast/         # TypeScript AST rules
```

Custom rules use a type-first structure:
```
ratchets/
├── regex/                  # Custom regex rules (all languages)
└── ast/                    # Custom AST rules (all languages)
```

### Usage in Commands

All commands that need rules use `RuleRegistry::build_from_config()`:

- `ratchets check`: Loads all enabled rules filtered by config
- `ratchets bump`: Validates rule exists before bumping budget
- `ratchets tighten`: Loads all enabled rules to count violations
- `ratchets list`: Lists all enabled rules with their status

This centralization eliminates rule loading duplication and ensures all commands see the same set of rules.

## Design Principles

### Performance

1. **Parallel execution**: Parse files and run rules using all available cores
2. **Lazy parser loading**: Only load tree-sitter grammars for languages actually present
3. **Early termination**: Option to stop on first budget exceeded (for fast CI feedback)
4. **Incremental potential**: Design allows future support for only checking changed files

### Agent-First Design

1. **Structured output**: JSONL format with stable schema for programmatic parsing
2. **Actionable messages**: Include file, line, column, snippet for precise location
3. **Clear status**: Unambiguous pass/fail with exit codes
4. **Deterministic**: Same input produces same output (sorted, no timing-dependent order)

### Unix Philosophy

1. **Do one thing well**: Check code against rules with budgets
2. **Composable**: Exit codes and structured output integrate with other tools
3. **No network**: Runs entirely locally, no telemetry or remote calls
4. **Text configuration**: TOML files are human-readable and diff-friendly

### Safety

1. **Read-only by default**: `check` never modifies files
2. **Explicit mutations**: Only `init`, `bump`, `tighten` modify config files
3. **No code execution**: Rules are declarative (regex, tree-sitter queries), not arbitrary code
4. **Auditable**: All budget changes are visible in version control diffs
