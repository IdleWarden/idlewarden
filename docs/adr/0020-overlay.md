# ADR-0020: The overlay is a window above the game, never something drawn into it

**Status:** Accepted · **Date:** 2026-10-10

## Context

A session runs while the user is playing, and the controls live in a window they
are not looking at. The things they need mid-game are small: is it watching,
pause it, stop it right now, and why is nothing happening. The kill switch in
[ADR-0007](0007-input.md) is also supposed to work for someone who cannot reach
the mouse, and until this change it was a button in the main window.

Drawing into the game's own frame is the obvious way to build an in-game overlay
and the one this project cannot take: it means hooking the renderer and living
inside the game's process, which [ADR-0001](0001-plugin-model.md) and
[ADR-0014](0014-bridge.md) rule out.

## Decision

**The overlay is a second Tauri window.** It is transparent, always on top,
absent from the taskbar and not focusable, so a click on it does not take the
foreground from the game and `SendInput` keeps landing where it should. It is
placed over the client area of the window the Detector bound, in a corner and
with margins the user chooses, and it follows that window.

**It is shown only while the bound game is the foreground window.** Following
the user into another application would be a bug. The position is polled a few
times a second; nothing is subscribed to in the game.

**It has two forms.** A badge (the owl, its border carrying the session state)
and a panel with watch, stop, kill switch, dry run, each automation's readiness
and the plugin's live signal values. The panel reads the same session state as
the main window and never drains the event queue, so the two cannot starve each
other of events.

**Hotkeys are global and configurable.** They are registered through the
operating system's hotkey facility (`RegisterHotKey` on Windows), so they work
with the game focused: watch, panel, show or hide, and kill switch. Two actions
may not share a combination, the others may be left unbound, and **the kill
switch cannot be left unbound**. Settings persist in `overlay.json` in the data
directory.

## Why

* A separate window needs nothing from the game. It works for a screen-reading
  plugin and a bridged mod alike, and it is the same answer on every game.
* Not taking focus is the property that matters. An overlay that steals the
  foreground would make the Governor's focus requirement fail on the next
  command.
* A hotkey is the only control that exists when the game has the keyboard. The
  kill switch is the one that must.

## Consequences

* **Exclusive fullscreen hides it**, as it hides everything above the game.
  Borderless windowed is required, the same requirement as capture
  ([ADR-0005](0005-capture.md)).
* The kill switch now has the global hotkey ADR-0007 asked for, in addition to
  the button.
* A combination another program already owns cannot be registered. The app
  reports it and keeps the previous bindings rather than leaving none.
* Placement is in physical pixels from the overlay window's own scale factor. A
  game on a monitor with a different scale from the overlay's has not been
  tested.
