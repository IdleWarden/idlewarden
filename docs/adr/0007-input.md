# ADR-0007: `SendInput`, focus required, mandatory kill switch

**Status:** Accepted · **Date:** 2026-08-25

## Decision

* Mouse and keyboard via **`SendInput`**.
* **The game window must hold focus.** Background input is not promised.
* **A global kill switch is mandatory**, registered via `RegisterHotKey`, and
  checked before *every* command, not once per sequence.
* Timing is **jittered**, never metronomic.
* **Dry-run is the default.** Nothing touches the mouse until the user
  explicitly asks.
* Virtual gamepad (ViGEmBus) is post-MVP, behind the `InputBackend` trait.

## Why

* Many games consume DirectInput/RawInput and ignore synthesised messages, or
  require foreground focus. `PostMessage`-style background automation works for
  a small minority of titles; promising it means an endless stream of "it
  doesn't work for my game". Assume a dedicated foreground window and say so.
* **The kill switch is a safety feature, not a convenience.** Software that
  drives the mouse must be stoppable instantly by someone who cannot use the
  mouse to stop it. Checking it per command rather than per sequence is the
  difference between stopping now and stopping after the current macro.
* ViGEmBus requires installing a third-party kernel driver. That is a large
  trust ask for a genre that needs a mouse.

## Consequences

* Multi-boxing and true background automation are out of scope.
* `GuardedInput` wraps every backend; a backend used unguarded is a bug that
  code review should catch.
