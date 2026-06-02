# Ratchets: Progressive lint enforcement for human and AI developers

Ratchets is a progressive lint enforcement tool that allows codebases to contain existing violations while preventing new ones. Unlike traditional linters that enforce binary pass/fail, Ratchets permits a budgeted number of violations per rule per region. These budgets can only decrease over time (the "ratchet" mechanism), ensuring technical debt monotonically decreases.

## Key Features

- **Progressive enforcement**: Allow existing violations while preventing new ones
- **Region-based budgets**: Set different limits for different parts of your codebase
- **Regex and AST rules**: Match patterns via text or tree-sitter queries
- **Agent-friendly**: JSONL output, deterministic results, clear exit codes
- **Fast**: Parallel execution, lazy parser loading, Rust performance

## Installation

### Install from Github

Install from source using the installation script:

```bash
curl -sSf https://raw.githubusercontent.com/imbue-ai/ratchets/main/install.sh | sh
```

This will automatically build and install ratchets to `~/.cargo/bin/`. Requires Rust/Cargo to be installed.

## Quick Start

Initialize Ratchets in your repository:

```bash
ratchets init
```

This creates:
- `ratchets.toml` — Configuration file
- `ratchet-counts.toml` — Violation budgets
- `ratchets/` — Directory for custom rules

Optionally, drop a `.ratchetignore` file at any depth in the tree to exclude paths from ratchet enforcement. Syntax matches `.gitignore` (per-directory, nested files compose, negation supported with `!`); these files are checked in alongside source.

Run checks:

```bash
ratchets check
```

## Usage

### `ratchets check`

Verify that violations are within budget:

```bash
ratchets check                    # Check all files
ratchets check --format jsonl     # Machine-readable output
ratchets check src/               # Check specific path
ratchets check --since main       # Only files changed since the `main` ref
```

`--since <REF>` shells out to `git diff <REF> --name-only` and intersects the
result with the file walker's output. Include/exclude/gitignore filters still
apply, and files deleted relative to `<REF>` are skipped silently. The command
exits with code 2 if `<REF>` is unknown or the current directory is not inside
a git repository.

### `ratchets bump`

Increase the violation budget (requires justification in commit message):

```bash
ratchets bump no-unwrap --region src/legacy --count 20
ratchets bump no-unwrap --region src/legacy  # Auto-detect current count
```

### `ratchets tighten`

Reduce budgets to match current violation counts:

```bash
ratchets tighten                    # Tighten all rules
ratchets tighten no-unwrap          # Tighten specific rule
ratchets tighten --region src/      # Tighten specific region
```

### `ratchets list`

List all enabled rules and their status:

```bash
ratchets list
ratchets list --format jsonl
```

## Configuration

### ratchets.toml

Ratchets uses an explicit opt-in model: rules only fire when listed in
`enabled_ratchets` (directly or via a `$set-name` reference). Anything in
`disabled_ratchets` is subtracted from the resolved enabled set — disabled
always wins.

```toml
enabled_ratchets = ["$common-starter", "no-unwrap"]
disabled_ratchets = ["no-fixme-comments"]

[ratchets]
version = "2"
languages = ["rust", "typescript"]
include = ["src/**", "tests/**"]
exclude = ["**/generated/**"]

# Optional per-rule settings (severity, regions). Entries here do NOT
# enable rules; enablement is governed by enabled_ratchets above.
[rules]
no-todo-comments = { severity = "warning" }

[output]
format = "human"
```

#### Reference syntax

- `"rule-id"` — enables (or disables) a single rule by ID.
- `"$set-name"` — references a **ratchet-set**: a curated bundle of rule IDs.
  Sets can compose other sets via `$other-set` inside their own `rules`
  array; cycle-aware resolution catches accidental loops.
- The `@` sigil is unchanged — it still refers to entries in the existing
  `[patterns]` table for glob references.

#### Shipped ratchet-sets

This binary ships a single starter set:

- **`$common-starter`** — the language-agnostic curated default. Today's
  members: `no-todo-comments`, `no-fixme-comments`. Membership criterion
  ("stable, broadly applicable, no framework-specific opinions") is
  documented at the top of `builtin-ratchets/sets/common-starter.toml`.

Per-language starter sets (`$python-starter`, `$rust-starter`,
`$typescript-starter`) will land in follow-up MRs — their curation is
its own review topic.

#### User-defined ratchet-sets

Drop your own set files under `ratchets/sets/*.toml`. User-defined sets
override embedded sets with the same ID, mirroring how `ratchets/regex/`
and `ratchets/ast/` override embedded rules. A set file looks like:

```toml
[set]
id = "house-style"
description = "Rules that match our coding conventions"
rules = ["$common-starter", "no-unwrap"]
```

`enabled_ratchets = ["$house-style"]` then enables the union.

### ratchet-counts.toml

```toml
[no-unwrap]
"." = 0
"src/legacy" = 15
"tests" = 50

[no-todo-comments]
"src" = 23
```

Regions are explicitly configured directory paths. Files in unconfigured subdirectories count toward their nearest configured parent region. Regions are scoped per-rule.

Counts for rules no longer in the resolved enabled set are kept dormant
(no cleanup). `ratchets tighten` emits a stderr warning naming each
orphan so you can re-enable the rule later without losing the count.

## Git Integration

### Merge Driver

Ratchets provides a merge driver that resolves conflicts by taking the minimum count:

```bash
# .gitattributes
ratchet-counts.toml merge=ratchets

# .git/config
[merge "ratchets"]
    name = Ratchets counts merge driver (minimum wins)
    driver = ratchets merge-driver %O %A %B
```

### Pre-commit Hook

```bash
#!/bin/sh
ratchets check || exit 1
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All rules within budget |
| 1 | At least one rule exceeded budget |
| 2 | Configuration or usage error |
| 3 | Parse error in source file |

## Documentation

- [DESIGN.md](DESIGN.md) — Design specification and rationale
- [ARCHITECTURE.md](ARCHITECTURE.md) — Implementation architecture

## Further Reading

The ratchet concept was originally described by qntm: https://qntm.org/ratchet
