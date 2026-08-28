<img src="docs/logo.png" alt="" width="176" align="right">

# IdleWarden

**An extensible automation platform for single-player idle, incremental and
management games on Windows.**

IdleWarden watches a game window, builds an abstract picture of what is on
screen, decides what to do, and does it: mouse, keyboard, nothing exotic. New
games are added as plugins; the engine itself never learns about any particular
game.

> **Status: pre-alpha.** Nothing here is usable yet. The architecture is
> settled (see [`docs/adr/`](docs/adr/)); the implementation is not.

---

## The red line

This project is built for games where automation is *legitimate*: single-player,
offline, idle and incremental titles, the genre where automating the grind is
arguably the point, and games whose terms explicitly allow it.

IdleWarden is **not** designed around defeating anti-cheat systems, and the
official plugin registry refuses plugins targeting competitive or multiplayer
titles. That boundary is editorial, not technical: the engine is generic and
cannot police what people do with it, but what we distribute, we curate. See
[`PLUGIN_POLICY.md`](PLUGIN_POLICY.md).

There is deliberately **no** memory reading, DLL injection or process hooking
anywhere in this codebase, and adding some would be a breaking architectural
change. On idle games those techniques buy nothing that screen capture does not
already give you, while contaminating the entire security model.

---

## How it works

```
   Game window
        │  Windows Graphics Capture, 2-4 fps, window-relative
        ▼
    Capture ──────────► Frame (Arc, never cloned)
        │
        ▼
     Vision ──────────► anchored ROIs · template match · OCR · colour probe
        │
        ▼
   Observation ───────► typed signals, each with a confidence and an age
        │
        ▼
      Agent ──────────► one ticked behaviour tree, pluggable deciders
        │
        ▼
     Intent  ─────────► "buy_upgrade", in the plugin's own vocabulary
        │
        ▼
  ┌─ Governor ─┐ ◄──── rate limits · confidence floor · intent allow-list
  │   allow?   │       geometry bounds · session budget
  └─────┬──────┘
        ▼
     Input ───────────► window-relative commands · jitter · kill switch
        │
        ▼
   Game window
```

Two properties are load-bearing:

**Perception is probabilistic, and the uncertainty travels.** Every signal
carries a confidence and every observation carries an age. The agent sees both.
This is what stops it from clicking on ghosts.

**The agent does not police itself.** Every intent it produces passes through
the Governor before it can become a mouse event. Rate limits, confidence floors,
geometry bounds and session budgets live in the Core, not in the agent, which
is where the project's name comes from.

---

## Levels of integration

| Level | What it is | Plugin needed |
|-------|-----------|---------------|
| **L0** | Window detection + regions you draw yourself in the UI + simple rules | none, the UI writes an L1 plugin for you |
| **L1** | Declarative plugin: manifest, template assets, rules. No code. | yes |
| **L2** | L1 + sandboxed script (Rhai) for conditional logic, optional ONNX perception | yes |
| **L3** | Official integration: a supported mod over IPC, a documented API, a save file read read-only, the Core connects, it never injects ([ADR-0014](docs/adr/0014-bridge.md)) | yes |

L0 is not a second code path. "Working without a plugin" means the app builds a
declarative plugin from what you draw on screen, then runs it like any other.
One pipeline, always.

---

## Repository layout

```
idlewarden/
├── crates/
│   ├── plugin-api/    ← the contract. Apache-2.0, not MPL. Depend on this.
│   ├── capture/       ← Windows Graphics Capture of the game window
│   ├── vision/        ← anchored ROI matching, OCR, colour probes
│   ├── input/         ← SendInput, humanised timing, kill switch
│   ├── bridge/        ← client for a user-installed game mod (ADR-0014)
│   ├── plugin-host/   ← loads plugins. Never loads native third-party code.
│   ├── agent/         ← behaviour tree + deciders
│   └── core/          ← orchestration, event bus, Governor. No UI, no game.
├── apps/
│   └── desktop/       ← Tauri v2 + Angular shell. An adapter, nothing more.
├── plugins/           ← first-party plugins, one folder per game
├── docs/adr/          ← why everything is the way it is
└── .ferrflow          ← release config: crates semver, plugins calver-short
```

The plugin registry lives in a separate repository:
[`idlewarden/registry`](https://github.com/idlewarden/registry).

---

## Build

```bash
rustup toolchain install stable
cargo check --workspace
cargo test  --workspace

cd apps/desktop && pnpm install && pnpm tauri dev
```

The desktop app is the only front end. The pipeline underneath it is also
exercised headless by `crates/core/tests/pipeline.rs`, which runs on any OS
against the stub capture backend. The real capture and input backends do not
exist yet, so a session stays in `searching`.

---

## Writing a plugin

A plugin is a directory with a `plugin.json` and its assets:

```
plugins/example-game/
├── plugin.json        ← manifest: id, game matcher, signal schema, capabilities
├── rules.json         ← what to extract, and what to do about it
└── assets/
    └── collect_button.png
```

No compilation, no linking, no `cdylib`. The contract is a **data schema**, not
a Rust ABI, which is the only reason a stable plugin API is achievable at all.
See [`docs/adr/0001-plugin-model.md`](docs/adr/0001-plugin-model.md) and
[`docs/adr/0010-versioned-contract.md`](docs/adr/0010-versioned-contract.md).

Start from the [template](https://github.com/IdleWarden/registry/tree/main/template).

### It reloads live

Edit `rules.json` or a template crop and save: the running agent picks it up at
the next safe swap point. **No app restart, and never a game restart**: the
latter is not a feature but a consequence of being purely external.

Swaps only happen between agent ticks with no action in flight, so a save can
never land in the middle of a macro. Capabilities and `api_version` are
deliberately excluded: a plugin able to grant itself new powers by writing to a
file the host already watches would be privilege escalation dressed as
convenience. See [`docs/adr/0012-hot-reload.md`](docs/adr/0012-hot-reload.md).

### Versioning

Plugins use **calver-short** (`YY.M.PATCH`, e.g. `26.8.1`); the crates and
`api_version` stay **semver**. A plugin version tracks the *game's* patches,
which carry no compatibility meaning, semver there would be decoration
pretending to be a contract. Releases are managed by FerrFlow from conventional
commits. See [`docs/adr/0013-versioning.md`](docs/adr/0013-versioning.md).

---

## Licensing

| Part | Licence | Why |
|------|---------|-----|
| `crates/plugin-api` | **Apache-2.0** | Anything that must be adopted widely should be maximally permissive, with a patent grant. Your plugin can be licensed however you like. |
| Everything else | **MPL-2.0** | File-level copyleft: modify our files, publish those modifications. Combine with proprietary code freely otherwise. |

MPL-2.0 is GPL-compatible by default, so this choice does not close the
copyleft door, it just declines to force it open. See
[`docs/adr/0011-licensing.md`](docs/adr/0011-licensing.md).

## Contributing

Sign-offs, not CLAs, see [`CONTRIBUTING.md`](CONTRIBUTING.md). Security issues
go to [`SECURITY.md`](SECURITY.md), not the issue tracker.
