# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-06-22

### BREAKING

- The regex engine for ratchets' regex rules is now [resharp](https://crates.io/crates/resharp)
  (RE#) instead of the Rust `regex` crate. This affects every regex rule, both
  builtin and user-authored.
- Custom regex rules authored by users may behave differently or fail to compile
  under resharp. The relevant differences are:
  - **Leftmost-longest matching.** resharp selects the leftmost-longest match
    rather than leftmost-greedy. A pattern that previously stopped at the first
    acceptable match may now extend to the longest match starting at the same
    position.
  - **Lazy quantifiers are unsupported.** The lazy forms `*?`, `+?`, and `??`
    are not part of resharp's syntax and now produce parse errors. Rewrite lazy
    quantifiers using greedy quantifiers over a restricted character class
    (for example, `[^)]*?` becomes `[^)]*`) or use a lookahead to bound the
    match.
  - **Anchors and dot are configured to match the previous engine's defaults.**
    Multiline mode is off (so `^` and `$` match the start and end of the input,
    not of each line) and dot does not match a newline. Existing simple patterns
    are therefore unaffected by these settings.
  - **New capabilities are available.** resharp adds intersection `&`,
    complement `~(...)`, and lookaround (`(?=)`, `(?!)`, `(?<=)`, `(?<!)`),
    which were not expressible in the previous engine.

### Changed

- Rewrote two builtin patterns for resharp compatibility:
  - `no-ssh-subprocess`: the lazy class `[^)]*?` was replaced with the greedy
    `[^)]*`.
  - `no-click-echo`: the trailing `\b` after `.*` was replaced with an
    `echo(?=\W|\z)` lookahead.
- Converted three Python rules from tree-sitter AST rules to resharp regex rules
  using negative lookahead, and moved them from `builtin-ratchets/python/ast/`
  to `builtin-ratchets/python/regex/`:
  - `no-unnumbered-pyre-fixme`
  - `no-unnumbered-pyre-ignore`
  - `no-unlabeled-type-ignore`

  `classmethod-builder-naming` remains an AST rule.

### Migration notes

- Audit any custom regex rules under `ratchets/regex/` for lazy quantifiers
  (`*?`, `+?`, `??`); these are now parse errors and must be rewritten using
  greedy quantifiers over a restricted character class or a bounding lookahead.
- Review patterns that relied on leftmost-greedy semantics, since resharp's
  leftmost-longest matching can extend a match further than before.

### Note

- tree-sitter retains its own internal `regex` dependency to evaluate AST query
  text-predicates (`#match?` / `#not-match?`). This is unrelated to ratchets'
  regex rule engine, so the `regex` crate remains a transitive dependency of the
  project.
