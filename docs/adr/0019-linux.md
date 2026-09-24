# ADR-0019: Linux actuates through uinput and captures through the portal, and gives up enumerating windows

**Status:** Accepted · **Date:** 2026-09-24

## Context

[ADR-0005](0005-capture.md) and [ADR-0007](0007-input.md) name Windows APIs in
their titles, but the traits under them say nothing about a platform:
`CaptureBackend` hands out frames, `InputBackend` executes a command, and the
Core never learns which one it got. Linux is work to add, not an architecture to
undo. That is issue #11.

What does not carry over is the assumption underneath the Detector: that a
program may ask the system which windows exist and where they are. On X11 it
can. On Wayland it cannot, by design, and that is not a gap waiting for an API.

## Decision

**Input goes through `uinput`.** The Core writes events to a virtual device it
creates itself, and the compositor treats them like any other keyboard or mouse.
It works the same under X11 and Wayland, which is the whole reason to prefer it
over `XTEST`, and `XTEST` would also be a second backend to keep alive for a
display server people are leaving.

The cost is a permission: `/dev/uinput` is root-only on a stock install, so the
user adds a udev rule, or their user to a group. That is a setup step we document
and the app checks for, not something to paper over by asking for root.

**Capture goes through `xdg-desktop-portal` and PipeWire.** The `ScreenCast`
portal is the only route on Wayland, it exists on X11 sessions too, and it hands
back a PipeWire node that carries frames without a round trip through the
compositor's clipboard of screenshots.

**The portal prompt is part of the product.** The first session asks the user to
pick the window, in the compositor's own dialog. A `restore_token` is kept so
later sessions reuse that grant, and the app says plainly when the token is gone
and the prompt is coming back. This is the same shape as the bridge's consent
([ADR-0014](0014-bridge.md)): a permission the user grants explicitly, which we
surface rather than hide.

**Detection changes meaning on Wayland.** There is no window list, so the
Detector cannot poll for a matching title or executable. The portal's picker
*is* the selection: the user points at the game once, and the plugin is matched
against what the portal reports about that stream rather than against an
enumeration. On X11 the existing enumeration still works, and we use it.

**macOS stays out of scope.** ScreenCaptureKit and `CGEvent` both need granted
permissions, shipping an updater there needs notarisation, and none of that is
work we can finish or test today. Saying so is better than a half-port.

## Consequences

* A Linux session starts with a dialog the user answers, once per grant. That is
  slower than Windows, where capture starts silently, and it is the price of a
  platform where a program cannot watch a window it was not handed.
* The plugin matcher's `executable` and `window_title` fields keep working on
  X11 and become advisory on Wayland: they name what the user should pick, and
  the host checks the portal's answer against them rather than searching.
* `uinput` events are global, exactly like `SendInput`: the kill switch stays the
  only hard stop, and the same focus rule applies, because a virtual device types
  into whatever has focus.
* Two capture paths on Linux would be worse than one. X11 keeps the portal path
  too; XComposite is not implemented unless a portal-less setup turns out to
  matter to someone.
* The work splits in three, and they land separately: the input backend, the
  portal and PipeWire capture, and the detection change. The first is useful
  alone, since it is what a bridged session needs on Linux, where the mod
  provides the observation and nothing needs to be captured at all.
