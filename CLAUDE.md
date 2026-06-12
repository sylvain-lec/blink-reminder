# CLAUDE.md

This file gives Claude Code (and other AI agents) context for this repository.

The full, canonical agent/contributor guide lives in **[AGENTS.md](AGENTS.md)** —
please read it. It covers the architecture, the build/test/lint commands, and the
non-obvious gotchas (eframe 0.34's `App::ui`, the runtime click-through toggle,
and the `muda` tray-event delivery quirk).

Quick start:

```sh
cargo run            # run locally
cargo test           # unit tests (scheduler + fade curve)
cargo clippy && cargo fmt
```

Always keep `cargo clippy` and `cargo fmt` clean before committing.
