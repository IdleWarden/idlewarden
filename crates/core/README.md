# idlewarden-core

Orchestration, the event bus, and the Governor.

## Two rules define this crate

**It knows nothing about any specific game.** Game knowledge lives in plugins,
always. Game-specific logic in here is not a shortcut, it is the bug the plugin
system exists to prevent.

**It contains no UI code.** Not one `tauri::` symbol
([ADR-0004](../../docs/adr/0004-ui-boundary.md)). The desktop app is an adapter
over `Command` and `Event`; that discipline is what makes a headless daemon a
refactor rather than a rewrite, and `tests/pipeline.rs` proves it by driving a
whole session with no UI at all.

## The session runner

`runner.rs` is the loop that plays: capture, perceive, decide, govern, act. It
is a **tick** rather than a thread, so a whole session can be driven
deterministically in a test with no sleeping and no wall clock. `service.rs`
owns the thread, and is deliberately thin because nothing worth testing lives in
it.

Two rules are enforced here rather than trusted:

**Every intent goes through the Governor.** There is no path from the behaviour
tree to the input backend that does not pass `review`.

**Nothing is reported as succeeded until it has been checked.** An action's
commands run, and the intent is then held in flight until the *next*
observation, which is the only thing that can say whether the world changed
(ADR-0003). Reporting success at the moment the click was sent would be a lie
the rest of the system would believe.

The pieces are traits the Core does not implement: `vision::Perceiver`,
`agent::Node`, `runner::Actuator`, `capture::CaptureBackend`,
`input::InputBackend`. The Core sequences them and knows nothing about any game.

## The Governor

The component the project is named after, and the reason it is named that.

Every `Intent` the agent produces passes through it before it can become a mouse
event: rate limit, confidence floor, staleness, intent allow-list, geometry,
session budget. Verdicts are `Allow`, `Reject` or `Halt`, and halts are checked
before rejections, so a failing session stops instead of spinning on refusals.

**An agent cannot police itself.** Putting limits outside the agent, on the
single path to the operating system, makes them structural rather than
advisory. This is also the answer to the user-safety question: software that
drives the mouse unattended for hours needs a component whose entire job is
deciding when to stop.

It is the most heavily tested code in the workspace. Keep it that way.

## The event bus

Everything that happens is published as an `Event`. That is what makes a session
replayable, and replay is what makes *"why did it click there at 3am"* an
answerable question rather than a shrug.
