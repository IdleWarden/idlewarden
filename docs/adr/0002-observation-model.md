# ADR-0002 — No `GameState` type; a schema-declared stream of confident observations

**Status:** Accepted · **Date:** 2026-08-25

## Context

The instinctive design is a shared `GameState { player_position, health,
enemies, resources, ... }` struct that plugins fill in.

## Decision

There is **no** `GameState` type in the Core. Instead:

* A plugin **declares a schema of signals** in its manifest (id, value type,
  unit, description).
* At runtime it emits `Observation { frame_id, captured_at_ms, signals }`, where
  each `Signal` carries a dynamically-typed `Value` **and a `Confidence`**.
* The Core validates against the declared schema, transports, and stores. The UI
  renders any schema generically.
* A small set of **well-known signals** (`game.focused`, `game.window_rect`,
  `ui.screen_id`, `resource.<name>`) lets generic agents work across games.

## Why

* **A concrete shared struct couples the Core to every game at once.** Adding a
  game means editing the Core, which is exactly what the plugin system exists to
  avoid.
* **The obvious fields are the wrong fields.** `player_position` and `enemies`
  are FPS/RPG vocabulary. In an idle game none of them exist, and the fields
  that matter (`resource.gold`, `upgrade.affordable`) cannot be enumerated in
  advance.
* **Vision is probabilistic and the uncertainty must travel.** A struct of plain
  values silently asserts certainty the perception layer does not have. Carrying
  `Confidence` to the agent — and letting the Governor halt on a confidence
  collapse — is what stops the agent acting on a misread screen.
* **Observations are stamped and age.** Acting on a two-second-old screen is a
  bug; making age a first-class field makes it a checkable one.

## Consequences

* Type safety at the plugin boundary is *runtime* schema validation, not the
  Rust type system. Accepted deliberately: the alternative is no boundary.
* Signal ids are a namespace and need conventions. Well-known ids are
  documented; everything else is plugin-scoped.
