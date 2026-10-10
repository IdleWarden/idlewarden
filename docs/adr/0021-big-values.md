# ADR-0021: Numbers past an `f64` travel as a `big` value, written as text

**Status:** Accepted · **Date:** 2026-10-10

## Context

Incremental games routinely pass what an `f64` can hold (issue #83). Master
Healer Kale keeps a mantissa and a base-ten exponent in a `double_evrac`, and
Galaxy Idle Clicker ships `break_infinity.js`. Two things stop such a value
today, and they are in the transport, not in `Value::is_well_formed` (which no
caller uses):

* `serde_json` refuses `1e400`, the C# `JsonValue.Number` refuses infinity, and
  `JSON.stringify(Infinity)` writes `null`, which then fails to deserialise.
* `rules.json` thresholds are `f64`, so even a value that arrived could not be
  compared with one.

`KaleSignals.Wire` already saturates a larger magnitude to `double.MaxValue`.
Past that point `at_least` is always true and `increased` never is, with no
error anywhere.

## Decision

**`Value::Big`, serialised as `{"type":"big","value":"1.5e400"}`.** The payload
is text because one grammar then serves signal values and rule thresholds, the
UI's generic rendering already shows text, and the notation is what
`break_infinity` and Kale's own `Magnitude` print.

`<mantissa>[(e|E)<exponent>]`, a finite `f64` mantissa and an `i32` exponent. A
`Big` is normalised when it is built (`1 <= |mantissa| < 10`, one spelling of
zero), so `15e399` and `1.5e400` are the same value and equality needs nothing
special. NaN, infinities, and an exponent that overflows `i32` are refused at
deserialisation rather than stored.

**Comparing.** Two plain numbers compare as before. As soon as either side is a
`Big`, both are converted to `Big` and compared by sign, exponent, then
mantissa, so nothing is ever rounded through an `f64`. `Increased`,
`Decreased`, `AtLeast` and `AtMost` all go through this one function.

**The contract moves from `0.1.0` to `0.1.1`, not `0.2.0`.** Manifests say
`^0.1`, and a caret on a `0.x` version refuses `0.2.0`, which would orphan every
published plugin for an additive change. A plugin that emits `big` declares
`^0.1.1`, and an older host refuses it at the handshake instead of dropping its
signals.

## Alternatives rejected

* **The base-ten logarithm in a `Float`.** Finite and ordered, but every
  threshold an author writes becomes a logarithm, and it silently changes the
  meaning of signals that are not big: `at_least party.down 1` would compare a
  logarithm with a count.
* **Leave it and document the ceiling.** The ceiling is not a clean failure
  today, it is a wrong answer (see Context).
* **`{mantissa, exponent}` as an object.** Two ways to write a threshold, and
  the UI would need a case to render it.

## Consequences

* A new variant breaks exhaustive `match`es in Rust. ADR-0010 says the Rust
  types are not the contract, the JSON is, and no existing JSON changes.
* Precision is that of an `f64` mantissa, about 16 digits, the same trade
  `break_infinity` makes.
* Thresholds in `rules.json`, the C# and JavaScript bridges and the Kale mod
  follow in separate changes. Until the first of those lands nothing emits `big`.
* The vision perceiver does not read an exponent off the screen. A number too
  large for `f64` is reported as unreadable rather than wrong, which is the
  existing behaviour.
