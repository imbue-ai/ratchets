# Ratchet Architecture

## Philosophy

Ratchet collaborates with coding agents, verification tools, and human developers to enable long-horizon coding. By providing fast, deterministic verification, it enables rapid iteration while maintaining code quality invariants.

Core tenets:
- **Agent-first**: Structured JSONL output, TOML configuration, actionable error messages
- **Unix principles**: Do one thing well, compose with other tools, no network calls
- **Best-of-breed dependencies**: Rely on established Rust crates rather than reimplementing

Ratchet is a compiled binary that runs locally, exits cleanly, and never communicates over the network.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                    CLI                                       │
│  (clap argument parsing, subcommand dispatch, output formatting)             │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Core Engine                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Config    │  │    Rules    │  │   Counts    │  │     Execution       │ │
│  │   Loader    │  │   Loader    │  │   Manager   │  │     Engine          │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Rule Executors                                    │
│  ┌──────────────────────────┐    ┌──────────────────────────────────────┐   │
│  │     Regex Executor       │    │         AST Executor                 │   │
│  │  (regex crate)           │    │  (tree-sitter + language grammars)  │   │
│  └──────────────────────────┘    └──────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            File System Layer                                 │
│  (ignore crate for gitignore-aware traversal, parallel file reading)        │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── main.rs                 # Entry point, CLI setup
├── cli/
│   ├── mod.rs              # CLI module root
│   ├── args.rs             # Argument definitions (clap)
│   ├── check.rs            # `ratchet check` command
│   ├── init.rs             # `ratchet init` command
│   ├── bump.rs             # `ratchet bump` command
│   ├── tighten.rs          # `ratchet tighten` command
│   ├── list.rs             # `ratchet list` command
│   └── merge_driver.rs     # `ratchet merge-driver` command
├── config/
│   ├── mod.rs              # Configuration module root
│   ├── ratchet_toml.rs     # ratchet.toml parsing and validation
│   └── counts.rs           # ratchet-counts.toml parsing and manipulation
├── rules/
│   ├── mod.rs              # Rules module root
│   ├── rule.rs             # Rule trait and common types
│   ├── registry.rs         # Rule registry (built-in + custom)
│   ├── regex_rule.rs       # Regex rule implementation
│   ├── ast_rule.rs         # AST (tree-sitter) rule implementation
│   └── builtin/            # Built-in rule definitions
│       ├── mod.rs
│       ├── rust.rs         # Rust-specific built-in rules
│       ├── typescript.rs   # TypeScript-specific built-in rules
│       └── python.rs       # Python-specific built-in rules
├── engine/
│   ├── mod.rs              # Execution engine module root
│   ├── executor.rs         # Parallel execution coordinator
│   ├── file_walker.rs      # File discovery and filtering
│   ├── violation.rs        # Violation data structures
│   └── aggregator.rs       # Violation aggregation by region
├── output/
│   ├── mod.rs              # Output module root
│   ├── human.rs            # Human-readable terminal output
│   └── jsonl.rs            # JSONL structured output
└── lib.rs                  # Library root (for integration testing)

builtin-ratchets/           # Built-in rules in same format as custom rules
├── regex/
│   ├── no-todo-comments.toml
│   ├── no-fixme-comments.toml
│   └── ...
└── ast/
    ├── rust/
    │   ├── no-unwrap.toml
    │   ├── no-expect.toml
    │   └── ...
    ├── typescript/
    │   └── ...
    └── python/
        └── ...
```

## Key Components

### Config Loader

Responsibilities:
- Parse `ratchet.toml` from current directory (or specified path)
- Validate configuration schema
- Resolve rule references (built-in names, custom paths)
- Merge CLI overrides with file configuration

Key types:
```rust
pub struct Config {
    pub version: String,
    pub languages: Vec<Language>,
    pub include: Vec<GlobPattern>,
    pub exclude: Vec<GlobPattern>,
    pub rules: RuleConfig,
    pub output: OutputConfig,
}
```

### Rules Loader

Responsibilities:
- Load built-in rules from compiled-in definitions
- Parse custom rules from `ratchets/regex/` and `ratchets/ast/`
- Validate rule definitions (regex syntax, tree-sitter query syntax)
- Build rule registry keyed by rule ID

Key types:
```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> &str;
    fn description(&self) -> &str;
    fn languages(&self) -> &[Language];
    fn execute(&self, ctx: &ExecutionContext) -> Vec<Violation>;
}

pub struct RegexRule { /* ... */ }
pub struct AstRule { /* ... */ }
```

### Counts Manager

Responsibilities:
- Parse `ratchet-counts.toml`
- Resolve region inheritance (child inherits from parent)
- Provide budget lookup: `get_budget(rule_id, file_path) -> u64`
- Mutate counts for `bump` and `tighten` commands
- Serialize counts back to TOML

Key types:
```rust
pub struct CountsManager {
    counts: HashMap<RuleId, RegionTree>,
}

pub struct RegionTree {
    root_count: u64,
    overrides: HashMap<PathBuf, u64>,
}
```

### Execution Engine

Responsibilities:
- Coordinate parallel file processing
- Dispatch rules to appropriate executors
- Collect and aggregate violations
- Support both file-local and cross-file rules

Execution model:
1. **File discovery**: Walk directory tree respecting include/exclude patterns
2. **Parse phase** (parallel): Parse all files into ASTs (for AST rules)
3. **Rule execution phase** (parallel):
   - File-local rules: Run in parallel per file
   - Cross-file rules: Run with access to full parsed file set
4. **Aggregation phase**: Group violations by rule and region

Key types:
```rust
pub struct ExecutionEngine {
    config: Config,
    rules: RuleRegistry,
    counts: CountsManager,
}

pub struct ExecutionContext<'a> {
    pub file_path: &'a Path,
    pub content: &'a str,
    pub ast: Option<&'a tree_sitter::Tree>,
    pub all_files: &'a FileSet,  // For cross-file rules
}
```

### Regex Executor

Responsibilities:
- Compile regex patterns (cached)
- Execute regex against file content
- Extract match locations (line, column, snippet)

Implementation notes:
- Use `regex` crate with `RegexSet` for multi-pattern matching
- Precompute line offsets for efficient line/column conversion
- Parallel execution across files using `rayon`

### AST Executor

Responsibilities:
- Load tree-sitter grammars on demand (lazy loading)
- Parse source files into ASTs
- Execute tree-sitter queries
- Extract match locations from query captures

Implementation notes:
- Use `tree-sitter` crate with language-specific grammar crates
- Cache parsed ASTs for reuse across multiple rules
- Grammar crates compiled in as optional features for each language

Supported languages (v1):
- Rust (`tree-sitter-rust`)
- TypeScript/JavaScript (`tree-sitter-typescript`, `tree-sitter-javascript`)
- Python (`tree-sitter-python`)
- Go (`tree-sitter-go`)

Additional languages added via feature flags.

### Output Formatters

#### Human Formatter

- Colorized output (when TTY detected or `--color=always`)
- Grouped by rule, then by region
- Shows violation locations with snippets
- Summary line at end

#### JSONL Formatter

- One JSON object per line
- Three record types: `violation`, `summary`, `status`
- Deterministic ordering (sorted by rule, file, line)
- Stable schema for agent consumption

## Data Flow

### `ratchet check` Flow

```
1. Parse CLI arguments
2. Load ratchet.toml → Config
3. Load ratchet-counts.toml → CountsManager
4. Load rules (built-in + custom) → RuleRegistry
5. Discover files (respecting include/exclude)
6. For each enabled rule:
   a. Filter files by rule's language/pattern constraints
   b. Execute rule against matching files (parallel)
   c. Collect violations
7. Aggregate violations by rule and region
8. For each rule/region:
   a. Look up budget from CountsManager
   b. Compare violation count to budget
   c. Record pass/fail status
9. Format output (human or JSONL)
10. Exit with appropriate code
```

### `ratchet tighten` Flow

```
1. Run check flow (steps 1-8)
2. For each rule/region where violations < budget:
   a. Update CountsManager with new (lower) count
3. Serialize CountsManager to ratchet-counts.toml
4. Report changes made
```

### `ratchet merge-driver` Flow

```
1. Parse base, ours, theirs TOML files
2. Build unified set of all rule/region keys
3. For each key:
   a. Get count from ours (or inherit/default)
   b. Get count from theirs (or inherit/default)
   c. Result = min(ours, theirs)
4. Write merged result to ours file
5. Exit 0 on success
```

## Parallelism Model

Ratchet uses `rayon` for parallel execution:

1. **File-level parallelism**: Files are processed in parallel
2. **Rule-level parallelism**: Independent rules run concurrently
3. **Parse caching**: ASTs are parsed once and shared across rules

Thread pool sizing:
- Default: Use all available cores
- Override: `RAYON_NUM_THREADS` environment variable

Synchronization points:
- After file discovery (need full file list for cross-file rules)
- After rule execution (need all violations for aggregation)

## Error Handling

Ratchet uses typed errors with `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum RatchetError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Rule error: {0}")]
    Rule(#[from] RuleError),

    #[error("Parse error in {file}: {message}")]
    Parse { file: PathBuf, message: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

Error handling philosophy:
- Configuration errors: Exit 2, report clearly
- Parse errors (invalid source): Exit 3, continue checking other files
- Rule execution errors: Log warning, treat as 0 violations for that file
- I/O errors: Exit 2, report file path

## Dependency Choices

| Purpose | Crate | Rationale |
|---------|-------|-----------|
| CLI parsing | `clap` | Industry standard, derive macros |
| TOML parsing | `toml` | Standard Rust TOML crate |
| Regex | `regex` | Fast, well-maintained |
| AST parsing | `tree-sitter` | Multi-language, fast, mature |
| Parallelism | `rayon` | Ergonomic data parallelism |
| File walking | `ignore` | Gitignore-aware, fast |
| Glob patterns | `globset` | Part of ignore crate ecosystem |
| Serialization | `serde` | Standard for Rust serialization |
| Error handling | `thiserror` | Ergonomic error types |
| Color output | `termcolor` | Cross-platform terminal colors |

## Testing Strategy

See `TESTING.md` for full details. Key points:

- Unit tests: Per-module, test individual components
- Integration tests: End-to-end CLI tests in `tests/`
- Fixture-based: Test rules against known-good/known-bad code samples
- Property tests: Use `proptest` for regex and query parsing edge cases

## Future Considerations

Reserved for future versions (not in v1 scope):

1. **Incremental checking**: Only check files changed since last run
2. **Watch mode**: Re-run on file changes
3. **Auto-fix**: Some rules may support automatic fixes
4. **LSP integration**: Language server for editor integration
5. **Remote rule sharing**: Fetch rule definitions from URLs
6. **Custom rule plugins**: WASM or Lua for complex rules beyond tree-sitter queries
