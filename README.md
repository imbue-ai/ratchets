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
```

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

```toml
[ratchets]
version = "1"
languages = ["rust", "typescript"]
include = ["src/**", "tests/**"]
exclude = ["**/generated/**"]

[rules]
no-unwrap = true
no-todo-comments = { severity = "warning" }

[output]
format = "human"
```

### ratchet-counts.toml

```toml
[no-unwrap]
"." = 0
"src/legacy" = 15
"tests" = 50

[no-todo-comments]
"src" = 23
```

Regions are directory subtrees. Child regions inherit parent budgets unless overridden.

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
