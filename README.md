# Ratchet: Progressive lint enforcement for human and AI developers

Ratchet is a progressive lint enforcement tool that allows codebases to contain existing violations while preventing new ones. Unlike traditional linters that enforce binary pass/fail, Ratchet permits a budgeted number of violations per rule per region. These budgets can only decrease over time (the "ratchet" mechanism), ensuring technical debt monotonically decreases.

## Key Features

- **Progressive enforcement**: Allow existing violations while preventing new ones
- **Region-based budgets**: Set different limits for different parts of your codebase
- **Regex and AST rules**: Match patterns via text or tree-sitter queries
- **Agent-friendly**: JSONL output, deterministic results, clear exit codes
- **Fast**: Parallel execution, lazy parser loading, Rust performance

## Installation

```bash
cargo install ratchet
```

## Quick Start

Initialize Ratchet in your repository:

```bash
ratchet init
```

This creates:
- `ratchet.toml` — Configuration file
- `ratchet-counts.toml` — Violation budgets
- `ratchets/` — Directory for custom rules

Run checks:

```bash
ratchet check
```

## Usage

### `ratchet check`

Verify that violations are within budget:

```bash
ratchet check                    # Check all files
ratchet check --format jsonl     # Machine-readable output
ratchet check src/               # Check specific path
```

### `ratchet bump`

Increase the violation budget (requires justification in commit message):

```bash
ratchet bump no-unwrap --region src/legacy --count 20
ratchet bump no-unwrap --region src/legacy  # Auto-detect current count
```

### `ratchet tighten`

Reduce budgets to match current violation counts:

```bash
ratchet tighten                    # Tighten all rules
ratchet tighten no-unwrap          # Tighten specific rule
ratchet tighten --region src/      # Tighten specific region
```

### `ratchet list`

List all enabled rules and their status:

```bash
ratchet list
ratchet list --format jsonl
```

## Configuration

### ratchet.toml

```toml
[ratchet]
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

Ratchet provides a merge driver that resolves conflicts by taking the minimum count:

```bash
# .gitattributes
ratchet-counts.toml merge=ratchet

# .git/config
[merge "ratchet"]
    name = Ratchet counts merge driver (minimum wins)
    driver = ratchet merge-driver %O %A %B
```

### Pre-commit Hook

```bash
#!/bin/sh
ratchet check || exit 1
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
