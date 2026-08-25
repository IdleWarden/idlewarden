# ADR-0003 — `Intent` and `InputCommand` are strictly separated

**Status:** Accepted · **Date:** 2026-08-25

## Decision

Two layers that never mix:

* **`Intent`** — what the agent decides, in the plugin's vocabulary:
  `buy_upgrade { tier: 3 }`.
* **`InputCommand`** — what the Core executes: move, click, key, scroll, wait —
  in **window-relative coordinates normalised to `0.0..=1.0`**, never screen
  pixels.

The plugin owns the translation. The Core executes sequences with
**preconditions** (focus, expected resolution, known screen), humanised timing,
and — critically — a **post-condition check**. Every action returns an
`ActionOutcome`: `Succeeded`, `Failed`, `Rejected`, `Aborted` or `TimedOut`.

## Why

* **Screen-absolute coordinates break constantly** — the window moves, the user
  changes monitor, DPI scaling shifts. Window-relative coordinates survive all
  three, and bounds-checking against `0.0..=1.0` gives the Governor a trivial,
  reliable geometry guard.
* **Without post-conditions there is no robustness.** An action that fires a
  click and assumes it worked cannot detect that a modal stole the input. The
  agent needs to know whether the world actually changed, and only a
  post-condition tells it.
* **`execute(&mut self, action)` returning `()` is the original sin** of the
  interface this replaces: no failure channel, no timeout, no cancellation.

## Consequences

* Plugins carry the translation burden. This is correct: it is game knowledge.
* Every intent needs a defined post-condition. An intent without one is a bug,
  and reviewers should treat it as such.
