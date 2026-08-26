# ADR-0009: Limits live in the Core, never in the agent

**Status:** Accepted · **Date:** 2026-08-25

## Decision

A **Governor** in the Core intercepts every `Intent` before it can become input.
It enforces:

* **Rate limit**: actions per minute.
* **Confidence floor**: perception below threshold halts the session.
* **Staleness**: an observation older than the budget is not grounds for
  acting.
* **Intent allow-list**: per profile.
* **Geometry**: nothing outside the game window, enforced by the
  `0.0..=1.0` invariant from ADR-0003.
* **Session budget**: wall-clock ceiling.

Verdicts are `Allow`, `Reject { reason }` or `Halt { reason }`. Halts are
checked before rejections, so a failing session stops instead of spinning.

## Why

**An agent cannot police itself.** Any limit implemented inside the decision
logic is a limit the decision logic can route around, through a bug, a bad
rule, a mis-tuned model, or an LLM that decided the rule did not apply. Putting
limits outside the agent, on the single path to the operating system, makes them
structural rather than advisory.

This is also the answer to the user-safety question. Software that drives the
mouse for hours unattended needs a component whose entire job is deciding when
to stop. That component is what the project is named after.

## Consequences

* Every intent passes one choke point. That choke point is heavily tested.
* The Governor needs the observation, not just the intent: its most valuable
  checks are about *perception quality*, not action content.
