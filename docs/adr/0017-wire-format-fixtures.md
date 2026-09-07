# ADR-0017: The UI's types stay hand-written, and a generated fixture keeps them honest

**Status:** Accepted · **Date:** 2026-09-07

## Context

`apps/desktop/src/app/session/session.model.ts` restates by hand every shape the
Core serialises. Nothing checked that the two stayed in step, so renaming a field
in `idlewarden_core` produced a runtime bug that both compilers were happy with.
That is issue #18.

The obvious fix is to generate the TypeScript from the Rust with `ts-rs` or
`tauri-specta`. Both work by putting derive macros on the Core's types, which
means the Core starts carrying annotations that exist purely for the desktop
app's benefit. [ADR-0004](0004-ui-boundary.md) is explicit that the Core contains
no UI code, and that this is what keeps a headless daemon a refactor rather than
a rewrite. A `#[derive(TS)]` on `Event` is UI code in the Core, just spelled
quietly.

## Decision

Keep `session.model.ts` hand-written. Add `crates/core/tests/wire_format.rs`,
which serialises one value of **every** variant the UI can observe and writes it
to `apps/desktop/src/app/session/wire-format.generated.ts` as typed constants:

```ts
export const EVENTS: readonly SessionEvent[] = [ … ];
```

The file is committed. The Rust test fails when the committed copy no longer
matches what the Core serialises; `pnpm typecheck` in `apps/desktop` fails when
the fixtures no longer satisfy the hand-written types. Both run in CI.

## Why this catches drift in both directions

A rename in the Core leaves the developer exactly two paths, and both are closed:

* Regenerate the fixture, and `tsc` rejects it against the stale model.
* Do not regenerate, and `cargo test -p idlewarden-core` rejects the stale file.

Verified rather than assumed: renaming `window_title` to `windowTitle` in the
fixture produces `TS2353: '"windowTitle"' does not exist in type
'{ event: "game_detected"; … }'` and a failing `the_generated_wire_format_matches_what_the_core_serialises`.

Two further tests assert that every `Event` tag and every `Value` tag appears in
the fixtures, because a variant nobody sampled is a variant nobody checks. Adding
a variant to the Core without sampling it fails those instead of passing silently.

## What it found immediately

The hand-written model was already wrong in two places, which is the argument for
the mechanism better than any reasoning about it:

* `SignalValue` was missing `enum`, the variant the example plugin's
  `ui.screen_id` actually emits.
* `Intent` was declared as `{ name: string }`, dropping `params` entirely, so
  every intent parameter reaching the UI was invisible to the compiler.

`ActionOutcome` was `unknown` and is now a real union.

## Consequences

* The Core keeps zero knowledge of TypeScript. The direction of the dependency is
  unchanged: the app adapts to the Core, never the reverse.
* Adding a variant means adding a sample. That is the cost, and it is the point:
  the sample is what the UI's handling of that variant is checked against.
* The generated file is in `.prettierignore`. It is written by a Rust test, and
  making its formatter agree with Prettier byte for byte would be busywork with a
  new way to fail.
* This does not check behaviour, only shape. A field that keeps its name and
  changes its meaning still passes, and no type system was going to catch that.
* If the desktop app ever needs the full type surface rather than the wire
  vocabulary, revisit generation. The tension with ADR-0004 would still be real,
  but the trade would be a different one.
