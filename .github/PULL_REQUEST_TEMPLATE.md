## What this changes

<!-- The failure it prevents, not the code it adds.
     "Rejects stale observations so the agent cannot act on a screen that has
     already changed" beats "adds age check". -->

## Which ADR governs it

<!-- Link the relevant docs/adr/ entry. If this contradicts one, say so
     explicitly and open a companion PR against that ADR, do not work around
     it silently. -->

## Checklist

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] Commits signed off (`git commit -s`)
- [ ] SPDX header on every new source file
- [ ] No game-specific knowledge added under `crates/`, that belongs in a plugin
- [ ] No `tauri::` outside `apps/desktop/`
