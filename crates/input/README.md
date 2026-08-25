# idlewarden-input

Actuation. Governed by [ADR-0007](../../docs/adr/0007-input.md).

## Non-negotiables

* **The kill switch is checked before every command**, not once per sequence.
  That is the difference between stopping *now* and stopping after the current
  macro finishes. Software that drives the mouse must be stoppable by someone
  who cannot use the mouse to stop it.
* **Dry-run is the default.** Nothing touches the mouse until the user
  explicitly asks.
* **Coordinates are window-relative** and bounds-checked against `0.0..=1.0`, so
  "never click outside the game" is a cheap, reliable invariant rather than a
  hope.

## Using it correctly

Always wrap a backend in `GuardedInput`. A raw `InputBackend` bypasses the kill
switch and the geometry check — an unguarded backend in a pull request is a bug
review should catch.

## Scope

Focus is required; background input (`PostMessage`) works for too few games to
promise. Virtual gamepad support (ViGEmBus) is post-MVP and stays behind the
`InputBackend` trait, because it means asking the user to install a kernel
driver.
