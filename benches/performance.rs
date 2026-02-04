//! Performance benchmarks for Ratchet
//!
//! These benchmarks measure the performance of key operations:
//! - File walking speed with various filters
//! - Regex rule execution on different file sizes
//! - AST rule execution with parser caching
//! - Full check workflow end-to-end
//!
//! ## Running Benchmarks
//!
//! To run all benchmarks:
//! ```bash
//! cargo bench
//! ```
//!
//! To run specific benchmarks:
//! ```bash
//! cargo bench file_walking
//! cargo bench regex_execution
//! cargo bench ast_execution
//! ```
//!
//! ## Performance Profiling
//!
//! For detailed profiling, you can use tools like:
//! - `cargo flamegraph --bench performance` (requires flamegraph package)
//! - `perf record -g cargo bench`
//!
//! ## Expected Performance Characteristics
//!
//! Based on the implementation:
//!
//! ### File Walking
//! - Should scale linearly with number of files
//! - Gitignore filtering is efficient (uses ignore crate)
//! - Glob filtering adds minimal overhead
//!
//! ### Regex Execution
//! - Regex compilation is cached (done once per rule)
//! - Execution time scales with file size and pattern complexity
//! - Parallel execution provides near-linear speedup
//!
//! ### AST Execution
//! - Parser initialization is cached per language
//! - AST parsing is the dominant cost
//! - Query execution is fast relative to parsing
//! - Caching is effective - subsequent uses avoid parser creation
//!
//! ### Parallel Execution
//! - Uses rayon for parallelism
//! - File I/O and parsing happen in parallel
//! - Should scale well up to number of CPU cores

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use ratchets::engine::file_walker::{FileEntry, FileWalker};
use ratchets::rules::{AstRule, ExecutionContext, ParserCache, RegexRule, Rule, RuleRegistry};
use ratchets::types::{GlobPattern, Language};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a temporary directory with test Rust files
fn create_test_files(count: usize, size: usize) -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    for i in 0..count {
        let content = format!(
            "// File {}\n{}\nfn main() {{\n    println!(\"Hello\");\n}}\n",
            i,
            "// TODO: implement this\n".repeat(size / 30) // Roughly 30 chars per line
        );
        fs::write(temp_dir.path().join(format!("file{}.rs", i)), content).unwrap();
    }

    temp_dir
}

/// Create a sample regex rule for TODO comments
fn create_todo_regex_rule() -> RegexRule {
    let toml = r#"
[rule]
id = "no-todo"
description = "Find TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#;
    RegexRule::from_toml(toml).unwrap()
}

/// Create a sample AST rule for unwrap calls (Rust)
#[cfg(feature = "lang-rust")]
fn create_unwrap_ast_rule() -> AstRule {
    let toml = r#"
[rule]
id = "no-unwrap"
description = "Find unwrap calls"
severity = "error"

[match]
query = """
(call_expression
  function: (field_expression
    field: (field_identifier) @method)
  (#eq? @method "unwrap")) @violation
"""
language = "rust"
"#;
    AstRule::from_toml(toml).unwrap()
}

// ============================================================================
// File Walking Benchmarks
// ============================================================================

/// Benchmark file walking performance
///
/// This measures the speed of discovering and filtering files.
/// Tests both unfiltered and filtered walking.
fn bench_file_walking(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_walking");

    // Test with different numbers of files
    for file_count in [10, 50, 100].iter() {
        let temp_dir = create_test_files(*file_count, 500);

        group.throughput(Throughput::Elements(*file_count as u64));

        // Benchmark: Walk all files (no filter)
        group.bench_with_input(
            BenchmarkId::new("unfiltered", file_count),
            file_count,
            |b, _| {
                b.iter(|| {
                    let walker = FileWalker::new(temp_dir.path(), &[], &[]).unwrap();
                    let files: Vec<_> = walker.walk().collect();
                    black_box(files)
                });
            },
        );

        // Benchmark: Walk with include filter
        group.bench_with_input(
            BenchmarkId::new("with_include_filter", file_count),
            file_count,
            |b, _| {
                b.iter(|| {
                    let include = vec![GlobPattern::new("*.rs")];
                    let walker = FileWalker::new(temp_dir.path(), &include, &[]).unwrap();
                    let files: Vec<_> = walker.walk().collect();
                    black_box(files)
                });
            },
        );

        // Benchmark: Walk with exclude filter
        group.bench_with_input(
            BenchmarkId::new("with_exclude_filter", file_count),
            file_count,
            |b, _| {
                b.iter(|| {
                    let exclude = vec![GlobPattern::new("*0.rs")]; // Exclude files ending in 0
                    let walker = FileWalker::new(temp_dir.path(), &[], &exclude).unwrap();
                    let files: Vec<_> = walker.walk().collect();
                    black_box(files)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Regex Rule Benchmarks
// ============================================================================

/// Benchmark regex rule execution
///
/// This measures the performance of pattern matching using regex rules.
/// Tests with different file sizes to show scaling characteristics.
fn bench_regex_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_execution");

    let rule = create_todo_regex_rule();

    // Test with different file sizes (in bytes)
    for size in [500, 5_000, 50_000].iter() {
        let content = format!(
            "{}\nfn main() {{\n    println!(\"Hello\");\n}}\n",
            "// TODO: implement\n".repeat(size / 30)
        );

        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &content, |b, content| {
            b.iter(|| {
                let ctx = ExecutionContext {
                    file_path: Path::new("test.rs"),
                    content,
                    ast: None,
                };
                let violations = rule.execute(&ctx);
                black_box(violations)
            });
        });
    }

    group.finish();
}

// ============================================================================
// AST Rule Benchmarks
// ============================================================================

/// Benchmark AST rule execution with parser caching
///
/// This demonstrates the effectiveness of parser caching.
/// The first parse is expensive, but subsequent parses reuse the cached parser.
#[cfg(feature = "lang-rust")]
fn bench_ast_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("ast_execution");

    let rule = create_unwrap_ast_rule();

    // Test with different file sizes
    for size in [500, 5_000, 50_000].iter() {
        let content = format!(
            "fn main() {{\n{}\n}}\n",
            "    Some(42).unwrap();\n".repeat(size / 30)
        );

        group.throughput(Throughput::Bytes(*size as u64));

        // Benchmark: AST parsing + query execution (includes parser cache lookup)
        group.bench_with_input(
            BenchmarkId::new("with_parsing", size),
            &content,
            |b, content| {
                b.iter(|| {
                    let ctx = ExecutionContext {
                        file_path: Path::new("test.rs"),
                        content,
                        ast: None,
                    };
                    let violations = rule.execute(&ctx);
                    black_box(violations)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark parser cache effectiveness
///
/// This specifically tests that parser caching is working correctly
/// and providing performance benefits.
#[cfg(feature = "lang-rust")]
fn bench_parser_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_cache");

    let content = "fn main() { Some(42).unwrap(); }";

    // Benchmark: First parser creation (cache miss)
    group.bench_function("first_access", |b| {
        b.iter(|| {
            // Create new cache for each iteration to measure cold start
            let cache = ParserCache::new();
            let parser = cache.get_parser(Language::Rust);
            black_box(parser)
        });
    });

    // Benchmark: Subsequent parser access (cache hit)
    group.bench_function("cached_access", |b| {
        // Create cache once outside the benchmark
        let cache = ParserCache::new();
        // Prime the cache
        let _ = cache.get_parser(Language::Rust);

        b.iter(|| {
            let parser = cache.get_parser(Language::Rust);
            black_box(parser)
        });
    });

    // Benchmark: Full parse operation with cached parser
    group.bench_function("cached_parse", |b| {
        let cache = ParserCache::new();
        let _ = cache.get_parser(Language::Rust);

        b.iter(|| {
            let mut parser = cache.get_parser(Language::Rust).unwrap();
            let tree = parser.parse(content, None);
            black_box(tree)
        });
    });

    group.finish();
}

// ============================================================================
// End-to-End Workflow Benchmarks
// ============================================================================

/// Benchmark the complete check workflow
///
/// This measures the end-to-end performance including:
/// - File walking
/// - Rule execution (both regex and AST)
/// - Parallel processing
/// - Result aggregation
fn bench_full_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_workflow");
    group.sample_size(10); // Reduce sample size for expensive benchmarks

    // Test with different numbers of files
    for file_count in [10, 50].iter() {
        let temp_dir = create_test_files(*file_count, 1000);

        group.throughput(Throughput::Elements(*file_count as u64));

        // Add regex rules
        let rule_dir = temp_dir.path().join("rules");
        fs::create_dir(&rule_dir).unwrap();
        fs::write(
            rule_dir.join("todo.toml"),
            r#"
[rule]
id = "no-todo"
description = "No TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#,
        )
        .unwrap();

        // Benchmark: Execute all rules against all files
        group.bench_with_input(
            BenchmarkId::new("regex_rules", file_count),
            file_count,
            |b, _| {
                b.iter(|| {
                    // Create registry for each iteration (cheap compared to file operations)
                    let mut registry = RuleRegistry::new();
                    registry.load_custom_regex_rules(&rule_dir, None).unwrap();

                    let walker = FileWalker::new(temp_dir.path(), &[], &[]).unwrap();
                    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

                    let engine = ratchets::engine::executor::ExecutionEngine::new(registry);
                    let result = engine.execute(files);
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark parallel execution scaling
///
/// This demonstrates that parallel execution provides performance benefits.
/// Uses rayon's thread pool for parallel file processing.
fn bench_parallel_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_scaling");
    group.sample_size(10);

    // Create enough files to benefit from parallelism
    let temp_dir = create_test_files(100, 1000);

    let walker = FileWalker::new(temp_dir.path(), &[], &[]).unwrap();
    let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();

    let rule_dir = temp_dir.path().join("rules");
    fs::create_dir(&rule_dir).unwrap();
    fs::write(
        rule_dir.join("todo.toml"),
        r#"
[rule]
id = "no-todo"
description = "No TODO comments"
severity = "warning"

[match]
pattern = "TODO"
"#,
    )
    .unwrap();

    group.bench_function("100_files_parallel", |b| {
        b.iter(|| {
            // Create registry for each iteration
            let mut registry = RuleRegistry::new();
            registry.load_custom_regex_rules(&rule_dir, None).unwrap();

            let engine = ratchets::engine::executor::ExecutionEngine::new(registry);
            let result = engine.execute(files.clone());
            black_box(result)
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark Registration
// ============================================================================

criterion_group!(file_benches, bench_file_walking,);

criterion_group!(rule_benches, bench_regex_execution,);

#[cfg(feature = "lang-rust")]
criterion_group!(ast_benches, bench_ast_execution, bench_parser_cache,);

criterion_group!(
    workflow_benches,
    bench_full_workflow,
    bench_parallel_scaling,
);

#[cfg(feature = "lang-rust")]
criterion_main!(file_benches, rule_benches, ast_benches, workflow_benches);

#[cfg(not(feature = "lang-rust"))]
criterion_main!(file_benches, rule_benches, workflow_benches);
