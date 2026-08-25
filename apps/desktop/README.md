# Desktop app (Tauri v2)

Not scaffolded yet — this arrives in **Phase 2**, and deliberately after
`apps/cli`. If the CLI can drive a full session, the Core is genuinely
UI-independent (ADR-0004); scaffolding the UI first would let that boundary rot
before it was ever tested.

When the time comes:

```bash
cd apps/desktop
pnpm create tauri-app@latest .   # vanilla TS or Svelte; no framework opinion yet
```

Then add `apps/desktop/src-tauri` to `members` in the workspace `Cargo.toml`.

## The one rule

**No `tauri::` symbol outside this directory, and no business logic inside it.**

The app is an adapter: it turns UI gestures into `idlewarden_core::Command` and
renders `idlewarden_core::Event`. If a screen needs something the event
vocabulary cannot express, extend the vocabulary in the Core — do not reach past
it.

## Screens, in build order

1. **Detect** — installed games found, whether a plugin exists for each.
2. **Session** — start/stop, dry-run toggle, live state, kill-switch status.
3. **Activity** — timeline of intents, Governor verdicts and outcomes.
4. **Logs** — filterable structured log stream.
5. **Region editor** — draw ROIs and anchors on a captured frame. This is what
   turns "no plugin exists" into "you just wrote one" (L0 in the README).
6. **Profiles** — per-game configuration and Governor limits.
