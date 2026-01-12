# Design for the Ratchet tool.

TODO: A design agent must rewrite this entire file.

## Major design goals

TODO: Please preserve these goals, but rewrite them to be more factored, clear, precise and self-consistent.

* Fast
** Optimized, Compiled tool in Rust
** use all cores if possible
** use a standard rust crate for fast evaluation of the tight loops on a codebase
** Don't search files that aren't important/source

* Safe and customizable defaults
** All pre-provided ratchets add value by improving agent generation performance on long-horizon coding tasks.
** All ratchets can be disabled for all or parts of the code programmatically
** AST ratchets in the ratchet codebase are implemented to be fast.

* Extensible for adoption in a variety of use cases
** Easy to add new custom regex expressions.
** Easy to add new custom AST expressions that execute fast!

* No unnecessary overhead
** If you have no AST expressions, we don't load a AST parser.
** We don't load a Python AST-parser if you're only validating a JS codebase.

* Unix principles + Agentic design
** This tool will aim to follow the unix principles all the way, with the following additions:
** 1. Agent-first design, meaning all tools will support jsonl output, and structured json input if complex input is necessary.
** 2. TOML will be the language of configuration.
** 3. All output will consider the impact it might have on an LLM Agent reading the response, and be aimed to prompt the best possible response for the agent to achieve the goals of a long-lived codebase.


# TODO: OPEN DESIGN QUESTION:
* Should we aim to reuse the same structure for AST expressions -within- the ratchet codebase as for custom AST expressions? I think so!



## TODO: This section will be re-written in precise detail by a design agent.

* The ratchet tool is a Rust executable, which will be installable from source by using `cargo install`.

* The ratchet tool ships with a set of optional ratchets for different languages by default. We will continually be adding more languages and ratchets.

* Configuration of which ratchets are enabled will be done via the ratchet.toml file in the current folder.

* Ratchet counts may be stored in multiple different folders in a source code project.

* Custom ratchets may be defined as regexs and ast expressions in the ratchets/ folders relative to the current folder.

* When you run ratchet in a given folder, it looks for configuration and counts in the current folder, parses the ratchets in the ratchets/ folder, and then evaluates them upon the current folder as the root for the source code to check. This is the simplest and intended way to run ratchets. Later, for convenience, will will support the ability to invoke ratchets with a different conf folder. We will separately also support the ability to invoke ratchets with a different target folder.

* We don't currently know how custom AST expressions will be implemented. This is a major design question. We want to keep ratchets fast to evaluate but also expressive. Possible answers might be lua, Python,

* Ratchet must be fast. It will explicitly run using all cores, in a multithreaded way, to check all the code in parallel.
