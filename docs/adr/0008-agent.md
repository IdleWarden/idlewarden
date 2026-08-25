# ADR-0008 — One ticked behaviour tree with pluggable deciders

**Status:** Accepted · **Date:** 2026-08-25

## Context

The wish-list was "rules, state machines, deterministic behaviour, ML, a vision
model, and an LLM for high-level decisions" — read naively, six agent
implementations.

## Decision

**One** execution model: a ticked behaviour tree. The "kinds of agent" become
node types implementing a single `Decider` trait — `RuleDecider`,
`ScriptDecider`, `ModelDecider`, `LlmDecider`.

**The LLM is never in the hot loop.** It proposes a *goal*, rarely (on an event,
at most once a minute), and the tree executes it. Results are cached.

## Why

* Six agent implementations means six loops, six sets of edge cases, six places
  where a pause is handled slightly differently. One tree with pluggable leaves
  gives the same expressive range for a sixth of the maintenance.
* Behaviour trees compose and are inspectable — you can render the tick and show
  the user *why* a decision happened, which is most of the debugging story.
* An LLM in the decision loop means per-tick latency, per-tick cost, and
  non-determinism in the one place determinism matters most. As a slow planner
  above a fast executor, it is genuinely useful.

## Consequences

* A new strategy is a new `Decider`, not a new agent.
* The tree must be serialisable so a profile can carry it and the UI can show
  it.
