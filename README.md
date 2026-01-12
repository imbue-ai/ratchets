# Ratchet: Enforce progressive lint checks for your codebase

`ratchet` provides human and AI developers the ability to define, prevent and reduce the occurrences of specific
undesirable patterns in code that is added to the codebase. Ratchet rules, hereafter just called `ratchets`, are Regex or AST expressions that may be run on a codebase to detect instances of these undesirable patterns.

Although similar in goals to lint rules, ratchet rules differ in that they allow a regional count of maximum tolerated
exceptions to the rule. This count is stored in files that are checked in to the repository, alongside the code itself.

The `ratchet` tool verifies that all the code is compliant with the rules. If the number of violations exceeds the permitted count for that rule, `ratchet` will fail with an informational message. Humans or explicitly authorized agentic developers are able to increment or `bump` ratchet counts to allow for special cases where a ratchet rule must be broken.

`ratchet` is expected to be used as a pre-commit hook and as a continuous integration check  to automatically verify that agents adhere to style guides. Any commits or pull requests that fail the ratchet check, or bump ratchet counts without justification can be rejected.

## Usage

### TODO: This section is speculative. Once the design has been fleshed out, this section must be re-written to adhere to the design we have agreed on.

`ratchet init`

Initializes the repository for use with `ratchet`, by creating the `ratchet.toml` file, `ratchet-counts.toml` file,  `ratchets/` folder.

`ratchet bump rule-id [new-count]`

Will bump the count for a given rule id. If the count is provided, we will use that, else we will re-run the ratchet check for the given region and use that.

`ratchet tighten [rule-id]`
Re-runs all ratchets, and updates the new counts in the ratchet-counts file. If the counts have increased instead, this will fail.

If a rule-id is provided, this will only run that specific rule id.

## Design

Please see DESIGN.md for the design.

## Further Reading

The ratchet tool was initially described here: https://qntm.org/ratchet
