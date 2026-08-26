# idlewarden-agent

Decision-making. Governed by [ADR-0008](../../docs/adr/0008-agent.md).

## One execution model

A ticked behaviour tree. That is all. "Rules", "state machine", "ML model" and
"LLM" are not four agents, they are four implementations of `Decider` plugged
in as leaves.

Six agent implementations would mean six loops, six sets of edge cases, and six
places where pausing behaves slightly differently.

## The LLM is never in the hot loop

It proposes a **goal**, rarely, on an event, at most once a minute, and the
tree executes it. In the per-tick path it would add latency, cost and
non-determinism exactly where determinism matters most.

## What this crate must never do

Enforce limits. Rate limiting, confidence floors and intent allow-lists live in
`core`'s Governor, because **an agent cannot police itself**
([ADR-0009](../../docs/adr/0009-governor.md)). Any limit implemented in here is
a limit a bad rule or a mis-tuned model can route around.
