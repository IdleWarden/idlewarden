# idlewarden-cli

The headless driver. Runs a full session — capture, perception, agent,
Governor, input — with no UI.

```bash
cargo run -p idlewarden-cli
RUST_LOG=debug cargo run -p idlewarden-cli
```

## Why this exists before the desktop app

Two reasons, both deliberate:

1. **It proves the Core is genuinely UI-independent.** If the CLI can drive a
   session, no business logic has leaked into the UI layer
   ([ADR-0004](../../docs/adr/0004-ui-boundary.md)). Scaffolding the UI first
   would let that boundary rot before it was ever tested.
2. **A vision pipeline is far easier to debug from a terminal** than through a
   web view.

It currently runs against `NullBackend`, so it works on any OS.
