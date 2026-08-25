# ADR-0001 — The Core never loads third-party native code in-process

**Status:** Accepted · **Date:** 2026-08-25

## Context

IdleWarden's value is its plugin ecosystem. The obvious implementation is a
`cdylib` per plugin, loaded with `libloading`.

## Decision

**The Core never loads third-party native code into its own process.** There are
three plugin tiers, none of which is a native library:

1. **Declarative** (default, ~90% of games) — a directory with `plugin.json`,
   template assets and rules. No code at all.
2. **Scripted** — Rhai: a pure-Rust interpreter with a natural sandbox and no
   FFI, for conditional logic.
3. **Out-of-process** — a child process speaking JSON-RPC over stdio, for
   official integrations. A crash kills that process, not the Core.

## Why not `cdylib`

* **Rust has no stable ABI.** Every plugin would have to be rebuilt against
  every Core release with the exact same compiler. That is not an ecosystem,
  that is a monorepo with extra steps.
* **A plugin panic or segfault takes the Core down with it.** This is the whole
  of the answer to "how do we survive faulty plugins".
* **A downloaded `cdylib` is arbitrary code execution** with the user's full
  privileges. No capability system can constrain it, because it can simply
  ignore the capability system.

## Why not WASM (yet)

`wasmtime` plus the component model is the natural evolution of tier 3 once
third-party signed plugins matter. Today it costs real complexity — toolchain,
host bindings, debugging — and buys nothing tier 1 does not already provide for
a genre whose plugins are mostly coordinates and thresholds. Revisit when the
registry has meaningful third-party volume.

## Consequences

* A plugin cannot do anything the host has not exposed. That is the point.
* `plugin-host` has no `NativeLibrary` variant, and adding one is a breaking
  architectural change requiring a superseding ADR.
* Very unusual games may be genuinely inexpressible. Acceptable: the answer is
  an out-of-process integration, not an in-process escape hatch.
