# Ratchets Performance Characteristics

This document describes the performance characteristics of Ratchets and the optimizations implemented.

## Overview

Ratchets is designed to efficiently process large codebases with thousands of files. Key performance features include:

- **Parallel File Processing**: Uses `rayon` for parallel execution across multiple CPU cores
- **Parser Caching**: Tree-sitter parsers are cached to avoid repeated initialization
- **Regex Compilation Caching**: Regex patterns are compiled once per rule
- **Efficient File Walking**: Uses the `ignore` crate for fast gitignore-aware file traversal

## Benchmarks

Comprehensive benchmarks are available in `/code/benches/performance.rs`. Run them with:

```bash
cargo bench
```

### Key Benchmarks

1. **File Walking**: Measures directory traversal speed with various filter configurations
2. **Regex Rule Execution**: Tests pattern matching performance on different file sizes
3. **AST Rule Execution**: Evaluates tree-sitter parsing and query performance
4. **Parser Cache Effectiveness**: Verifies caching provides performance benefits
5. **Full Workflow**: End-to-end performance including file walking, parsing, and rule execution
6. **Parallel Scaling**: Demonstrates parallel execution benefits

## Scalability Tests

Scalability tests in `/code/tests/scalability_tests.rs` verify performance with large codebases:

- **1000+ Files**: Tests processing 1000 files with regex rules
- **Mixed File Sizes**: Tests with realistic mix of small, medium, and large files
- **Memory Efficiency**: Ensures constant memory usage regardless of file count
- **Parser Caching**: Verifies AST parsing benefits from cached parsers

Run scalability tests with:

```bash
cargo test --test scalability_tests -- --nocapture
```

## Performance Results

Based on test results (development machine):

### File Walking
- **1000 files**: ~7ms (file system and gitignore filtering)
- Scales linearly with file count
- Glob filtering adds minimal overhead

### Regex Rule Execution
- **1000 files with TODO pattern**: ~100ms total
- Execution is parallelized across all CPU cores
- Regex compilation is cached per rule

### AST Rule Execution
- **100 files**: ~100ms (includes parsing)
- Parser caching is effective - subsequent parses reuse the same parser
- Query execution is fast relative to parsing cost

### Total Test Suite
- **All 356 tests**: ~2.4 seconds
- Well under the 120-second requirement

## Optimization Focus Areas

### 1. File I/O Parallelism
✅ **Implemented**: Files are processed in parallel using rayon's thread pool
- Each file is read and processed independently
- No shared mutable state between threads
- Near-linear speedup with CPU core count

### 2. AST Parsing Caching
✅ **Implemented**: Parser instances are cached per language
- First parse creates and caches the parser
- Subsequent parses reuse the cached parser
- Thread-safe using `RwLock` for interior mutability

### 3. Regex Compilation Caching
✅ **Implemented**: Regex patterns are compiled once during rule creation
- Compilation happens at rule load time
- Each rule execution reuses the compiled pattern
- No runtime compilation overhead

## Profiling Tools

For detailed performance analysis, you can use:

### Flamegraph (CPU profiling)
```bash
cargo install flamegraph
cargo flamegraph --bench performance
```

This generates an interactive SVG showing where CPU time is spent.

### Perf (Linux)
```bash
perf record -g cargo bench
perf report
```

### Time Profiler (macOS)
Use Instruments.app with the Time Profiler template.

## Performance Characteristics by Component

### FileWalker
- **O(n)** where n is the number of files
- Gitignore filtering is efficient (uses `ignore` crate's optimized walker)
- Glob matching is compiled once and reused

### RegexRule
- **Compilation**: O(m) where m is pattern complexity (done once)
- **Execution**: O(n*k) where n is file size, k is pattern complexity
- **Memory**: O(1) per rule (compiled regex is shared)

### AstRule
- **Parsing**: O(n) where n is file size
- **Query**: O(t) where t is tree size
- **Caching**: O(1) lookup for cached parser

### ExecutionEngine
- **Parallelism**: Files processed in parallel (rayon)
- **Memory**: O(v) where v is violation count (results collected)
- **Scaling**: Near-linear with CPU cores for I/O bound workloads

## Known Performance Characteristics

### Bottlenecks
1. **AST Parsing**: Most expensive operation for AST rules
   - Unavoidable for syntactic analysis
   - Mitigated by parser caching

2. **File I/O**: Can be bottleneck for many small files
   - Mitigated by parallel processing
   - Benefits from SSD vs HDD

3. **Violation Collection**: Memory grows with violation count
   - Acceptable for typical use cases
   - Could be optimized with streaming output if needed

### No Obvious Bottlenecks Remain
- Parser caching prevents redundant initialization ✅
- Regex compilation is cached ✅
- File processing is parallelized ✅
- File walking respects gitignore efficiently ✅

## Recommendations for Large Codebases

1. **Use include/exclude patterns** to limit files processed
2. **Run on multi-core machines** to benefit from parallelism
3. **Use SSD storage** for faster file I/O
4. **Keep rule count reasonable** - each rule processes all applicable files
5. **Prefer regex rules** when possible - they're faster than AST rules

## Continuous Performance Monitoring

The benchmark and scalability tests should be run regularly to catch performance regressions:

```bash
# Run benchmarks and save baseline
cargo bench --bench performance -- --save-baseline main

# After changes, compare against baseline
cargo bench --bench performance -- --baseline main

# Run scalability tests
cargo test --test scalability_tests --release -- --nocapture
```
