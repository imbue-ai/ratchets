# Architecture of Ratchets

## TODO: A Design Agent must rewrite the following page while adhering in spirit to the following guidelines.

The philosophy of this software is to collaborate with the agents, other verification software and broad industry practice to enable long-horizon coding for coding agents. By providing quick verification, we enact the design goals of the human principals of the project to enable rapid iteration.

* Unix principles + Agentic design
** This tool will aim to follow the unix principles all the way, with the following additions:
** 1. Agent-first design, meaning all tools will support jsonl output, and structured json input if complex input is necessary.
** 2. TOML will be the language of configuration.
** 3. All output will consider the impact it might have on an LLM Agent reading the response, and be aimed to prompt the best possible response for the agent to achieve the goals of a long-lived codebase.

ratchet is a compiled binary program that runs fast, runs locally, that exits cleanly, and does not communicate on the internet when it runs.

ratchet's implementation is designed to rely on best-of-breed Rust and other AST libraries, so as not to implement existing complexity itself.

This tool is allowed to assume that at initial setup time, it will be configured by either an human or an authorized powerful agent who can make decisions about how it should be set up. Further configuration (e.g. adding support for a new language to an existing codebase, for instance) can be treated similar.
