# Upgrading ratchets.toml from v1 to v2

The library now only parses `ratchets.toml` files with `[ratchets].version = "2"`. Any other version (including v1) is rejected at load time, and every CLI subcommand that reads the config prints this notice and exits with code 2. The canonical copy of this guide lives at https://github.com/imbue-ai/ratchets/blob/main/docs/upgrade-v1-to-v2.md.

## Schema diff

- `[ratchets].version` bumped from `"1"` to `"2"`.
- `[rules].rule-id = false` removed; use `disabled_ratchets = ["rule-id"]` instead.
- `[rules].rule-id = true` removed; enablement is now opt-in via `enabled_ratchets`.
- New `enabled_ratchets` array at the top of the file: list of rule IDs and `$set-name` references that enable the union.
- New `disabled_ratchets` array at the top of the file: same shape, subtracts from `enabled_ratchets` (disabled always wins).
- `$set-name` reference syntax names a ratchet-set (a curated bundle of rule IDs). The one set shipped with this binary is `$common-starter`; per-language starter sets will land in follow-up MRs. The `@` sigil is unchanged and still refers to entries in the existing `[patterns]` table.

## Concrete example

Before (v1):

```toml
[ratchets]
version = "1"
languages = ["python"]

[rules]
no-eval-usage = false
```

After (v2):

```toml
enabled_ratchets = ["$common-starter"]
disabled_ratchets = ["no-eval-usage"]

[ratchets]
version = "2"
languages = ["python"]
```

`$common-starter` enables the language-agnostic curated rules (today: `no-todo-comments`, `no-fixme-comments`). Any rule the user wants to silence — including rules pulled in via a set — goes into `disabled_ratchets`.

Read the canonical copy at https://github.com/imbue-ai/ratchets/blob/main/docs/upgrade-v1-to-v2.md
