# Supporting Sculptor: Rules Missing from This Library

**Tracking beads (one per porting strategy):**
- Group A — plain regex: [`code-61d`](../.beads/issues.jsonl)
- Group B — path-scoped regex: [`code-aoe`](../.beads/issues.jsonl)
- Group C — lookaround → tree-sitter AST: [`code-gkp`](../.beads/issues.jsonl)
- Group D — bespoke match-exhaustiveness: [`code-m2l`](../.beads/issues.jsonl)

All 42 issues in this document fit the existing `RegexRule`/`AstRule` infrastructure and require only TOML additions under `builtin-ratchets/` (with one possible exception called out in Group D).

## Background

This document is a punch list of lint rules that exist in sculptor's bespoke Python
ratchets implementation (`imbue_core/imbue_core/ratchets/` in the
`imbue-ai/sculptor` repository) but do **not** yet exist as builtin rules in this
library. The goal — the "north star" — is for sculptor to eventually delete its
Python implementation and adopt this Rust library, so every sculptor rule needs a
corresponding rule here.

### How this list was generated

1. Read `imbue_core/imbue_core/ratchets/ratchet_rules.py` in sculptor (47 rule
   definitions in a single `ratchet_test_builders` tuple).
2. Read every existing TOML in this repo's `builtin-ratchets/` directory (41
   rules across `common/`, `python/`, `rust/`, `typescript/`).
3. Mapped each sculptor rule to either (a) an existing builtin, (b) a
   plain-regex rule that can be added as a TOML, (c) an AST rule that must be
   added as a TOML because the Python original uses regex lookbehind/lookahead
   which Rust's `regex` crate does not support, or (d) the one bespoke rule
   that sculptor implements with hand-coded indentation parsing.

### What's already covered (do NOT re-port these)

| Sculptor rule | Existing builtin in this repo |
|---|---|
| `eval` | `python/ast/no-eval-usage.toml` |
| `relative_imports` | `python/regex/no-relative-imports.toml` |
| `NamedTuple` | `python/regex/no-namedtuple-usage.toml` (verify pattern: sculptor catches uppercase `NamedTuple`, existing builtin targets lowercase `namedtuple(`) |
| `import_underscore` | `python/ast/no-underscore-imports.toml` |
| `inline_functions` | `python/ast/no-inline-functions.toml` |
| `inline_imports` | `python/regex/no-inline-imports.toml` |
| `no_unnecessary_typing_imports` | `python/regex/no-typing-builtin-imports.toml` |

### What's left (42 rules)

Grouped by porting strategy:

- **Group A — Plain regex (21 rules)**: trivial port to a `.toml` under
  `builtin-ratchets/python/regex/`. The sculptor regex translates 1:1 (modulo
  TOML escaping).
- **Group B — Regex with path scoping (10 rules)**: same as A, but the rule
  applies only to certain paths. Use the `include`/`exclude` glob fields on
  the rule's `[match]` section.
- **Group C — Lookaround → tree-sitter (10 rules)**: sculptor's regex uses
  `(?<!...)` or `(?!...)` to express "this construct must / must not appear in
  a particular syntactic context." Rust regex doesn't support these. The
  rules become tree-sitter AST queries instead. Tree-sitter expresses the
  same constraints structurally and is generally more precise than the
  original regex.
- **Group D — Bespoke (1 rule)**: sculptor's
  `match_without_wildcard_or_assert_never` is hand-coded indentation parsing.
  With tree-sitter it collapses to a query on `match_statement` plus a
  `#not-match?` predicate; potentially zero code, potentially one small
  `PostFilter` variant. Verify during implementation.

### Schema

Each entry below uses the schema:

```
### Issue: <short description>
**beads:** <bead ID(s), once filed>
**description:** <one paragraph of context — what the rule catches and why>
**plan:** <rough guidance: file location, severity, source pattern, anything tricky>
```

The plans are intentionally rough. An Opus-class coding agent should be
trusted to figure out the exact tree-sitter query shape, the right TOML
escaping, severity defaults, etc. Cross-reference sculptor's
`ratchet_rules.py` for the original regex and `match_examples` /
`non_match_examples` if a corner case is unclear.

---

## Group A — Plain regex (21 rules)

**Bead:** `code-61d`

### Issue: port `pytorch_lightning`
**beads:**
**description:** Block all imports of the `pytorch_lightning` package.
**plan:** `builtin-ratchets/python/regex/no-pytorch-lightning.toml`. Pattern: `import pytorch_lightning|from pytorch_lightning`. Severity: error.

### Issue: port `logger.warning`
**beads:**
**description:** Sculptor forbids `logger.warning` calls (prefer `logger.error` or `logger.info`).
**plan:** `builtin-ratchets/python/regex/no-logger-warning.toml`. Pattern: `logger\.warning`. Severity: warning.

### Issue: port `import_quarantine`
**beads:**
**description:** Block imports from `quarantine` packages.
**plan:** `builtin-ratchets/python/regex/no-quarantine-import.toml`. Pattern: `\s*(from|import)\s+.*quarantine`. Severity: error. Watch for false positives on identifiers like `_quarantine` (sculptor's non-match examples cover this).

### Issue: port `quarantine_paths`
**beads:**
**description:** Catch string references to `quarantine/` paths (e.g. script invocations of quarantined files).
**plan:** `builtin-ratchets/python/regex/no-quarantine-paths.toml`. Pattern: `quarantine/`. Severity: error.

### Issue: port `walrus_operator`
**beads:**
**description:** Walrus operator (`:=`) is banned codebase-wide — too easy to misread as `=`.
**plan:** `builtin-ratchets/python/regex/no-walrus-operator.toml`. Pattern: ` := `. Severity: warning.

### Issue: port `ssh_subprocess`
**beads:**
**description:** Block direct `subprocess.*` calls whose args mention `ssh`; sculptor wraps these in shared helpers.
**plan:** `builtin-ratchets/python/regex/no-ssh-subprocess.toml`. Pattern: `subprocess\.(Popen|run|call|check_call|check_output)\([^)]*?ssh[^)]*?\)`. Severity: error.

### Issue: port `use_pathlib_over_os.path.join`
**beads:**
**description:** Prefer `pathlib.Path` over `os.path.join`.
**plan:** `builtin-ratchets/python/regex/no-os-path-join.toml`. Pattern: `os\.path\.join`. Severity: warning.

### Issue: port `remove_all_todo_removes`
**beads:**
**description:** Catch `# TODO remove` end-of-line comments left in the tree.
**plan:** `builtin-ratchets/python/regex/no-todo-remove-comment.toml`. Pattern: `# TODO remove$`. Severity: warning.

### Issue: port `implicit_string_concat`
**beads:**
**description:** Catch implicit multiline string concatenation (`"foo" "bar"` across two adjacent string literals). The original is a `FullFileRatchetTest` (multiline regex over file content) but no lookaround — `RegexRule` already runs full-file, so this is straightforward.
**plan:** `builtin-ratchets/python/regex/no-implicit-string-concat.toml`. Pattern: `^[^"\n]*"[^"]+"\s*f?"[^"]+` with multiline flag. Severity: warning. Sculptor's rule has a rich set of `non_match_examples` (docstrings, simple assignments) — verify these don't false-positive after porting.

### Issue: port `disallow_builtin_hash_function`
**beads:**
**description:** Block direct calls to the builtin `hash()` — sculptor has a stable hash helper that should be used instead.
**plan:** `builtin-ratchets/python/regex/no-builtin-hash.toml`. Pattern: `[^a-zA-Z_]hash\(`. Severity: error. The leading negative character class avoids matching identifiers like `tensor_hash(`.

### Issue: port `make_composite_seed`
**beads:**
**description:** Block direct calls to `make_composite_seed()` (sculptor uses a `CompositeSeed` class instead).
**plan:** `builtin-ratchets/python/regex/no-make-composite-seed.toml`. Pattern: `[^a-zA-Z_]make_composite_seed\(`. Severity: error.

### Issue: port `default_rng`
**beads:**
**description:** Block `numpy.random.default_rng(...)` — sculptor has a wrapper.
**plan:** `builtin-ratchets/python/regex/no-numpy-default-rng.toml`. Pattern: `(np|numpy)\.random\.default_rng\(`. Severity: error.

### Issue: port `asyncio.run`
**beads:**
**description:** Block raw `asyncio.run(...)`; use the sculptor wrapped variant.
**plan:** `builtin-ratchets/python/regex/no-asyncio-run.toml`. Pattern: `asyncio\.run\(`. Severity: warning.

### Issue: port `logger.exception`
**beads:**
**description:** Block `logger.exception(...)` calls; sculptor uses `log_exception(e)` helper.
**plan:** `builtin-ratchets/python/regex/no-logger-exception.toml`. Pattern: `logger\.exception\(`. Severity: warning.

### Issue: port `pydantic_model_copy`
**beads:**
**description:** Block pydantic `.model_copy(update=...)` calls in favor of `model_update`/evolver helpers for extra validation.
**plan:** `builtin-ratchets/python/regex/no-pydantic-model-copy-update.toml`. Pattern: `\.model_copy\(.*update\)`. Severity: warning.

### Issue: port `.text.decode`
**beads:**
**description:** Block `<node>.text.decode(...)` on tree-sitter node text (indentation may not match source). Sculptor has `get_text`/`get_source_code_slice` helpers.
**plan:** `builtin-ratchets/python/regex/no-tree-sitter-text-decode.toml`. Pattern: `\.text\.decode`. Severity: warning. Watch for false positives on `stdout.decode(...)` — sculptor's non-match examples cover these.

### Issue: port `index_string_by_bytes`
**beads:**
**description:** Block byte-index slicing of source code with tree-sitter node `.start_byte`/`.end_byte` (broken for unicode).
**plan:** `builtin-ratchets/python/regex/no-byte-index-source.toml`. Pattern: `\[.*\.start_byte\s*:.*\.end_byte\]`. Severity: warning.

### Issue: port `mypy_ignore_errors`
**beads:**
**description:** Block file-level `# mypy: ignore-errors` directives.
**plan:** `builtin-ratchets/python/regex/no-mypy-ignore-errors.toml`. Pattern: `# mypy: ignore-errors`. Severity: warning.

### Issue: port `pyre_ignore`
**beads:**
**description:** Block any `# pyre-ignore` (even when numbered). Sculptor wants `# pyre-fixme` instead when uncertainty exists.
**plan:** `builtin-ratchets/python/regex/no-pyre-ignore.toml`. Pattern: `# pyre-ignore`. Severity: warning. Note: paired with `no-unnumbered-pyre-ignore` in Group C — both rules coexist with different counts.

### Issue: port `pyre_fixme`
**beads:**
**description:** Block any `# pyre-fixme` — fewer of these over time is the goal.
**plan:** `builtin-ratchets/python/regex/no-pyre-fixme.toml`. Pattern: `# pyre-fixme`. Severity: warning.

### Issue: port `type_ignore`
**beads:**
**description:** Block any `# type: ignore` (with or without label) — should be using pyre-ignore/pyre-fixme with codes instead.
**plan:** `builtin-ratchets/python/regex/no-type-ignore.toml`. Pattern: `# type: ignore`. Severity: warning.

---

## Group B — Regex with path scoping (10 rules)

**Bead:** `code-aoe`

The `[match]` section of each rule needs an `include` glob list. Sculptor uses
file-path regexes; the Rust equivalent uses globs. For most rules a direct
translation is trivial. `sculptor_subprocess_calls` is the exception (negative
lookahead in the path filter) — see its plan.

### Issue: port `sculptor_copytree_calls`
**beads:**
**description:** Block `copytree(...)` calls anywhere in sculptor's Python tree (use `copy_dir` instead, which handles socket files).
**plan:** `builtin-ratchets/python/regex/no-sculptor-copytree.toml`. Pattern: `copytree([(,])`. `include = ["sculptor/**/*.py"]`. Severity: error.

### Issue: port `sculptor_subprocess_calls`
**beads:**
**description:** Block `subprocess.run/check_output/check_call` in sculptor's production code (`sculptor/sculptor/**/*.py` excluding tests/testing/scripts and `*_test.py`). Sculptor's helper `run_blocking()` should be used.
**plan:** `builtin-ratchets/python/regex/no-sculptor-subprocess.toml`. Pattern: `subprocess\.(run|check_output|check_call)\(`. `include = ["sculptor/sculptor/**/*.py"]`. `exclude = ["**/test/**", "**/testing/**", "**/scripts/**", "**/*_test.py"]`. Severity: error. Sculptor expresses this with a single regex using negative lookahead; we split it into include + exclude globs.

### Issue: port `integration_test_page_reload`
**beads:**
**description:** Block `page.reload()` (and `*.reload()`) in sculptor integration tests — causes ERR_INSUFFICIENT_RESOURCES on CI.
**plan:** `builtin-ratchets/python/regex/no-integration-page-reload.toml`. Pattern: `\.reload\(\)`. `include = ["sculptor/tests/integration/**/*.py"]`. Severity: error.

### Issue: port `integration_test_non_testid_queries`
**beads:**
**description:** Force integration tests to find elements via `data-testid`, not text/role/CSS queries.
**plan:** `builtin-ratchets/python/regex/no-integration-non-testid-queries.toml`. Pattern: `\.get_by_text\(|\.get_by_role\(|\.query_selector\(`. `include = ["sculptor/tests/integration/**/*.py"]`. Severity: warning.

### Issue: port `integration_test_css_locators`
**beads:**
**description:** Block raw `.locator(...)` CSS selectors in integration tests.
**plan:** `builtin-ratchets/python/regex/no-integration-css-locators.toml`. Pattern: `\.locator\(`. `include = ["sculptor/tests/integration/**/*.py"]`. Severity: warning.

### Issue: port `integration_test_type_method`
**beads:**
**description:** Block Playwright `.type(...)` in integration tests — use `.fill(...)` instead.
**plan:** `builtin-ratchets/python/regex/no-integration-type-method.toml`. Pattern: `\.type\(`. `include = ["sculptor/tests/integration/**/*.py"]`. Severity: warning.

### Issue: port `integration_test_page_goto`
**beads:**
**description:** Block `.goto(...)` URL manipulation in integration tests — sculptor is a desktop app, navigate via UI.
**plan:** `builtin-ratchets/python/regex/no-integration-page-goto.toml`. Pattern: `\.goto\(`. `include = ["sculptor/tests/integration/**/*.py"]`. Severity: warning.

### Issue: port `integration_test_page_evaluate`
**beads:**
**description:** Block `.evaluate(...)` JS injection in integration tests (interact via UI).
**plan:** `builtin-ratchets/python/regex/no-integration-page-evaluate.toml`. Pattern: `\.evaluate\(`. `include = ["sculptor/tests/integration/**/*.py"]`. Severity: warning.

### Issue: port `integration_test_time_sleep`
**beads:**
**description:** Block `time.sleep(...)` in integration tests — use Playwright's auto-retrying `expect()`/wait helpers.
**plan:** `builtin-ratchets/python/regex/no-integration-time-sleep.toml`. Pattern: `time\.sleep\(`. `include = ["sculptor/tests/integration/**/*.py"]`. Severity: warning.

### Issue: port `raw_html_button_in_tsx`
**beads:**
**description:** Block raw `<button>` JSX in sculptor's TypeScript frontend — use Radix `Button`/`IconButton`.
**plan:** `builtin-ratchets/typescript/regex/no-raw-html-button.toml`. Pattern: `<button(\s|>|$)`. `include = ["sculptor/frontend/src/**/*.tsx"]`. `languages = ["typescript"]`. Severity: warning.

---

## Group C — Lookaround → tree-sitter (10 rules)

**Bead:** `code-gkp`

These rules need tree-sitter AST queries because the original regex uses
lookbehind/lookahead. The Python pattern is included for reference — translate
its intent to a tree-sitter query rather than the regex form. The
`builtin-ratchets/python/ast/no-eval-usage.toml` and
`no-underscore-imports.toml` already in this repo are good shape templates.

### Issue: port `attrs`
**beads:**
**description:** `@attr.s(...)` must use one of a small set of allowed argument combinations (alphabetical order, `auto_attribs=True` required, `frozen=True` and `kw_only=True` optional, plus `auto_exc=True` / `repr=False` variants). Sculptor regex: `@attr.s\((?!((auto_exc=True, )?auto_attribs=True(, frozen=True)?(, kw_only=True)?(, repr=False)?)\))`.
**plan:** `builtin-ratchets/python/ast/attrs-decorator.toml`. Match `decorator` nodes whose call is `attr.s(...)` and whose argument list doesn't fit the allowed shape. The `#not-match?` predicate on the captured argument-list text is a reasonable route. Severity: error.

### Issue: port `args_kwargs`
**beads:**
**description:** `def foo(*args, **kwargs)` is banned unless typed as `*args: P.args, **kwargs: P.kwargs` (paramspec usage). Sculptor regex: `(def \w+\(.*\*args(?!: P\.args))|(def \w+\(.*\*\*kwargs(?!: P\.kwargs))`.
**plan:** `builtin-ratchets/python/ast/no-untyped-args-kwargs.toml`. Match `function_definition` nodes whose parameters include `list_splat_pattern` / `dictionary_splat_pattern` whose type annotation isn't `P.args` / `P.kwargs`. Severity: warning.

### Issue: port `non_sys_exit`
**beads:**
**description:** `exit(N)` is banned (use `sys.exit(N)`). Sculptor regex: `(?<!sys\.)\bexit\(\d+\)`.
**plan:** `builtin-ratchets/python/ast/no-bare-exit.toml`. Match `call` nodes whose function is `(identifier) "exit"` (i.e. NOT `attribute object: "sys"`) with an integer argument. Severity: error.

### Issue: port `non_build_classmethods`
**beads:**
**description:** `@classmethod` decorated methods must start with one of: `from_`, `build`, `_build`, `load`, `get_config`, `__get_pydantic_core_schema__`. Sculptor regex: `(?<=(@classmethod\n    ))(async )?def (?!(from_|build|load|_build|get_config|__get_pydantic_core_schema__))`.
**plan:** `builtin-ratchets/python/ast/classmethod-builder-naming.toml`. Match `decorated_definition` with a `@classmethod` decorator and a `function_definition` whose name fails to match the allowed prefix regex (use `#not-match?` on the captured name). Severity: warning.

### Issue: port `non_private_staticmethods`
**beads:**
**description:** `@staticmethod` methods must be private (name starts with `_`). Sculptor regex: `(?<=(@staticmethod\n    ))def (?!(_))`.
**plan:** `builtin-ratchets/python/ast/staticmethod-private-only.toml`. Match `decorated_definition` with `@staticmethod` and `function_definition` whose name doesn't start with `_`. Severity: warning.

### Issue: port `mutable_attr_in_frozen_dataclass`
**beads:**
**description:** Classes with `@attr.s(auto_attribs=True, frozen=True, ...)` must not have fields typed as `Dict`, `List`, or `Set` (mutable). Sculptor uses a long full-file regex with lookbehinds to handle docstrings and comments between the decorator and the field.
**plan:** `builtin-ratchets/python/ast/no-mutable-attr-in-frozen-dataclass.toml`. Match `class_definition` with a `decorator` whose call args include both `auto_attribs=True` and `frozen=True`, then match `block` body containing a typed attribute whose type starts with `Dict`/`List`/`Set`. Severity: error. The structural form is much cleaner than the original regex.

### Issue: port `unnumbered_pyre_ignore`
**beads:**
**description:** `# pyre-ignore` without a code (`[123]`) is banned. Sculptor regex: `# pyre-ignore(?!-all-errors|\[[\d,\s]+\])`.
**plan:** `builtin-ratchets/python/ast/no-unnumbered-pyre-ignore.toml`. Match `comment` nodes whose text starts with `# pyre-ignore` but doesn't immediately have `-all-errors` or `[<digits>]`. Use `#match?` on the captured comment text for the positive form and `#not-match?` for the allowed forms. Severity: warning.

### Issue: port `unnumbered_pyre_fixme`
**beads:**
**description:** `# pyre-fixme` without a code is banned. Sculptor regex: `# pyre-fixme(?!\[[\d,\s]+\])`.
**plan:** `builtin-ratchets/python/ast/no-unnumbered-pyre-fixme.toml`. Same shape as `no-unnumbered-pyre-ignore`. Severity: warning.

### Issue: port `unlabeled_type_ignore`
**beads:**
**description:** `# type: ignore` without a label (`[return-value]`, etc.) is banned. Sculptor regex: `# type: ignore(?!\[.*\])`.
**plan:** `builtin-ratchets/python/ast/no-unlabeled-type-ignore.toml`. Match `comment` containing `# type: ignore` not immediately followed by `[...]`. Severity: warning.

### Issue: port `cast`
**beads:**
**description:** Block `cast(...)` calls (either `typing.cast(...)` or bare `cast(...)`); does NOT match `something.cast(...)`. Sculptor regex: `(?:typing\.|(?<![\w.]))cast\(`.
**plan:** `builtin-ratchets/python/ast/no-typing-cast.toml`. Two alternative query patterns: (a) `call` whose function is `identifier "cast"`, (b) `call` whose function is `attribute object: identifier "typing" attribute: identifier "cast"`. Severity: warning.

---

## Group D — Bespoke (1 rule)

**Bead:** `code-m2l`

### Issue: port `match_without_wildcard_or_assert_never`
**beads:**
**description:** Every Python `match` block must end with `case _ as <var>: assert_never(<var>)` for exhaustiveness checking. Sculptor implements this in `MatchCaseRatchetTest` with hand-coded indentation parsing to find the bounds of the match block, then a regex check on its text. With tree-sitter, `match_statement` is a native node — the rule collapses to "match statement whose text doesn't contain `case _ as X: assert_never(X)`."
**plan:** `builtin-ratchets/python/ast/match-must-assert-never.toml`. Match `match_statement` nodes and use `#not-match?` against the captured node text. Caveats:
- Rust's regex (used by tree-sitter's `#match?`) does not support backreferences, so the predicate can't enforce that the bound variable name matches — relax to `case\s+_\s+as\s+\w+\s*:\s*\n?\s*assert_never\(\w+\)`. False negatives only occur if someone writes mismatched variable names, which doesn't happen in practice.
- Confirm during implementation that `#not-match?` in tree-sitter 0.22 evaluates against multi-line node text. If it doesn't, the fallback is a small new `PostFilter` variant alongside `ClassNameNotException` — call it `MatchExhaustiveness` and do the text scan in Rust. ~20 lines.

Severity: error.

---

## Appendix: Cross-reference

The full original Python source for these rules is at:
`imbue_core/imbue_core/ratchets/ratchet_rules.py` in the `imbue-ai/sculptor`
repository (single tuple `ratchet_test_builders` containing 47 entries). Each
entry includes `match_examples` and `non_match_examples` that should be used to
validate ports — they catch the edge cases the regex was designed for.

Sculptor's current per-rule violation budgets are in
`imbue_core/imbue_core/ratchets/ratchet_values.json`. These should be carried
over into sculptor's `ratchet-counts.toml` under the root region `"."` when
sculptor adopts this library (out of scope for this document; tracked
separately).
