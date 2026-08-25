# Plugin policy

This document governs what the **official registry** distributes. It does not,
and cannot, govern what people build privately — the engine is generic. What we
curate is what we ship.

## Accepted

* Single-player and offline games.
* Idle, incremental, clicker and management games — the genre where automating
  repetition is the intended experience.
* Games whose terms of service explicitly permit automation, macros or bots.
* Games with an official, documented automation or modding interface.

## Refused

* Any game with a competitive or ranked multiplayer mode, whether or not the
  plugin targets that mode. We do not adjudicate "but I only use it in
  single-player" — the answer is no.
* Anything that circumvents, disables, probes or works around an anti-cheat
  system or any other protection mechanism.
* Plugins relying on memory reading, DLL injection, code hooking or driver
  installation. This includes **bridge plugins** (ADR-0014), which depend on a
  mod running inside the game process. Such a plugin is perfectly legitimate to
  build and to install by hand; it is the *registry* that will not carry it,
  because we cannot review a binary we do not host and would not want to host.
  Requests to relax this will be declined.
* Games whose terms explicitly forbid automation, regardless of the mode.
* Anything designed to obtain a competitive advantage over other human players,
  or to acquire tradeable goods for sale.

## How this is enforced

Three layers, in order of strength:

1. **Review.** Every registry submission is a pull request. `multiplayer: true`
   in the manifest is an automatic refusal, and reviewers check the claim.
2. **Capabilities.** A plugin declares what it needs. `Unverified` plugins get
   nothing granted silently; network access is never granted silently below
   `Official`.
3. **Architecture.** There is no code path in IdleWarden that could load a
   native plugin, read another process's memory, inject into another process or
   install a driver. This is the strongest layer precisely because it is not a
   rule anyone can waive. A bridge does not weaken it: the Core connects to an
   endpoint the user created, it never creates one, and
   `Capability::Bridge` is the single capability no trust level grants
   silently.

## Grey areas

Bring them to a discussion thread before writing the plugin. A game with a
cooperative-only multiplayer mode, a single-player game with online
leaderboards, a game whose terms are silent rather than permissive — these are
real cases and they get decided individually, in public, and recorded here.

## If you disagree with a refusal

Open a discussion. Decisions get revisited when the facts change — a game adds
an official API, a publisher clarifies its terms. They do not get revisited by
reopening the same pull request.
