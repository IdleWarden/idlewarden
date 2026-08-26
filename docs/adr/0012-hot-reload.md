# ADR-0012: Hot reload of plugin data, at safe swap points only

**Status:** Accepted · **Date:** 2026-08-25

## Context

Authoring a plugin means tuning coordinates, thresholds and template crops until
they match. If each tweak costs an app restart, the loop is unusable and nobody
writes plugins.

## Decision

**Plugin data hot-reloads. The Core does not. Capabilities never do.**

A file watcher (`notify`, debounced ~250 ms) covers each loaded plugin's
directory. On change: re-parse, validate against the schema, and **swap an
`Arc<PluginRuntime>` at the next safe point**. On a validation failure, keep the
running version and surface the error, a bad save must never take a running
session down.

| Change | Live | Why |
|---|---|---|
| `rules.json` | yes | pure data |
| `assets/*.png` | yes | pure data |
| `profiles/*.json` | yes | pure data |
| `presentation` | yes | runtime never reads it |
| `signals` / `intents` schema | yes, with a tree reset | the tree's cursor may reference a removed intent |
| **`capabilities`** | **no** | see below |
| **`api_version`** | **no** | compatibility is decided at load |
| Core / crates | no | that is what a rebuild is for |

### The game is never restarted

Not a feature, a consequence. IdleWarden is purely external: it captures a
window and synthesises input, and never touches the game's process (ADR-0001,
ADR-0005). There is no code inside the game that could need reloading.

### Safe swap points

A swap may only happen **between agent ticks, with no action in flight**.
Swapping mid-sequence would leave a half-executed macro against a rule set that
no longer describes it, a click landing wherever the old coordinates pointed.

The actuator therefore holds a swap gate: a pending reload waits for the current
`Intent` to reach a terminal `ActionOutcome`, then applies. If the in-flight
action exceeds its timeout, it is `Aborted` and the swap proceeds.

### Capabilities are excluded on purpose

The host already watches the plugin's directory. If capability grants were read
from a watched file, a plugin shipped with `capture` could write `net:evil.tld`
into its own manifest at runtime and gain it silently. That is privilege
escalation dressed as convenience. Capability changes require an explicit
reload, with the same consent prompt as a fresh install.

### What survives a swap

* **Governor counters persist.** Resetting the rate limit on reload would make
  "touch a file" an escape hatch from the limit, every constraint in ADR-0009
  would become advisory.
* **The behaviour tree resets.** Its cursor is meaningless against a new tree.
* **Observations are stateless** and need no migration.
* The session **stays in its current state**: a reload never silently resumes a
  paused or halted session.

## Consequences

* `rules.json` must stay purely declarative. The moment behaviour hides in
  compiled code, this all stops working, which is a second, independent reason
  for ADR-0001.
* Watching is on by default for plugins loaded from a development path and
  opt-in for installed ones: silently re-reading `%APPDATA%` is surprising, and
  a half-written file from an interrupted download should not be picked up.
* Every reload emits an `Event`, so the activity log answers "did my change
  actually apply".
