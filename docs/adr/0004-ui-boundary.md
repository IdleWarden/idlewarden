# ADR-0004 — Tauri v2 + web UI, zero `tauri::` in the Core

**Status:** Accepted · **Date:** 2026-08-25

## Decision

The desktop app is **Tauri v2** with a web front end. The Core is a library that
speaks in `Command` and `Event`; the Tauri app is nothing but an adapter over
that vocabulary. **No `tauri::` symbol appears outside `apps/desktop/`.**

At MVP both live in one process. That is fine.

## Why Tauri

The UI needs sortable tables, a filterable log stream, a session timeline, frame
previews and an interactive region editor. The web platform is simply better at
all five, and small bundles beat Electron.

`egui` / `iced` are viable and would delete the IPC layer entirely — a real
advantage. They lose on the region editor and the log viewer, which are the two
screens the user will spend the most time in.

## Why the boundary matters more than the toolkit

Keeping the Core UI-free is what makes three later moves cheap rather than
catastrophic: a headless daemon, a CLI that is not a toy, and replacing the UI
toolkit if Tauri turns out to be the wrong bet. The discipline costs almost
nothing today.

## Consequences

* Every UI capability must be expressible as `Command`/`Event`. This is a
  feature: it keeps the Core's surface honest and makes the session replayable.
* `apps/cli` exists partly as an enforcement mechanism — if the CLI can drive a
  session, the Core is genuinely UI-independent.
