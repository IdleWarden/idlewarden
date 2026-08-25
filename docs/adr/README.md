# Architecture decision records

Each file records one decision, the alternatives that were rejected, and why.
They are written to be argued with: if you think one is wrong, open a pull
request against the ADR, not around it.

| # | Decision | Status |
|---|----------|--------|
| [0001](0001-plugin-model.md) | The Core never loads third-party native code in-process | Accepted |
| [0002](0002-observation-model.md) | No `GameState` type — a schema-declared stream of confident observations | Accepted |
| [0003](0003-intent-vs-input.md) | `Intent` and `InputCommand` are strictly separated | Accepted |
| [0004](0004-ui-boundary.md) | Tauri v2 + web UI, zero `tauri::` in the Core | Accepted |
| [0005](0005-capture.md) | Windows Graphics Capture, window-scoped, 2–4 fps | Accepted |
| [0006](0006-vision.md) | Pure-Rust classical vision, no OpenCV; anchoring is the hard part | Accepted |
| [0007](0007-input.md) | `SendInput`, focus required, mandatory kill switch | Accepted |
| [0008](0008-agent.md) | One ticked behaviour tree with pluggable deciders | Accepted |
| [0009](0009-governor.md) | Limits live in the Core, never in the agent | Accepted |
| [0010](0010-versioned-contract.md) | The plugin contract is a data schema, not a Rust ABI | Accepted |
| [0011](0011-licensing.md) | MPL-2.0 core, Apache-2.0 plugin API | Accepted |
