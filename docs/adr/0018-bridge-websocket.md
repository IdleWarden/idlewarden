# ADR-0018: A mod that cannot open a pipe connects to us instead, over a local WebSocket

**Status:** Accepted · **Date:** 2026-09-20

## Context

[ADR-0014](0014-bridge.md) makes the mod the server: it opens a named pipe, and
the Core connects to it. That works because a BepInEx or MelonLoader plugin is
.NET code running with the process' own privileges.

A whole family of idle games is out of reach that way. Cookie Clicker, Galaxy
Idle Clicker and most browser ports are Electron applications, and the mod API
they expose to authors is JavaScript running inside the page. Cookie Clicker
ships `Game.registerMod(id, hooks)` and loads mods from
`resources/app/mods/local/<id>/`, which is a real, documented loader, so the
premise of ADR-0014 holds: the user installs the mod themselves. But the page
runs with `contextIsolation: true`, and the preload exposes one fixed IPC channel
to the game's own main process. A mod there has no `net`, no `fs`, and no way to
serve a pipe.

What a page can always do is open a WebSocket to `127.0.0.1`.

## Decision

A bridge may be served over a **local WebSocket**, and the roles swap: IdleWarden
listens, the mod connects.

Everything above the transport is unchanged. The host still sends `hello` first
and still refuses an incompatible `api_version`; `observe` and `act` are the same
messages in the same order; signals are still stamped `CERTAIN` by the Core; and
post-conditions are still mandatory. `Transport` already hides all of this, so
the WebSocket is one more implementation of it.

* The listener binds **`127.0.0.1` only**, never `0.0.0.0`.
* It runs **only while a session whose plugin declares a granted bridge is
  starting**, and stops as soon as a mod connects or the wait expires.
* The endpoint name is the request path: `ws://127.0.0.1:<port>/<name>`. A
  connection asking for another path is refused, so one game's mod cannot answer
  for another's plugin.
* `Capability::Bridge { name }` gates it exactly as before, and is still never
  granted silently.

## The Origin header is the security boundary

A local WebSocket is reachable by any page the user has open, including a remote
website in their browser. Without a check, `https://evil.example` could connect
and pretend to be the mod, and everything the Core believes about the game would
come from it.

So the handshake **requires an `Origin` that belongs to a local document**: absent,
`null`, `file://…` or `app://…`. Anything with an `http://` or `https://` scheme
is refused, which is every remote page, because a browser sets `Origin` itself and
a page cannot forge it.

That is weaker than the pipe, where reaching the endpoint already meant running
code on the machine. It is the reason the listener's life is measured in seconds
rather than left open for the session, and the reason this ADR exists rather than
a line in ADR-0014.

## What this is not

It is **not a loader for arbitrary Electron applications**. The equivalent of
BepInEx for Electron would patch an app's `asar` or its preload script, which is a
different promise from the one ADR-0014 makes: there the user chose a loader and
installed a mod through it. Games that already expose a mod API need none of that,
and those are the ones this ADR serves. A game with no mod API is not covered, and
would have to argue its own case.

It also does **not** make the Core inject anything. We open a socket and wait; if
no mod was installed, nothing connects and the session pauses saying so.

## Consequences

* Two transports exist, so something has to choose. The pipe is tried first, and
  the WebSocket is the fallback with a visible wait, because a pipe that is not
  there fails immediately while a listener has to wait by its nature.
* A JavaScript implementation of the protocol becomes a shipped artefact, next to
  the C# one, with the same fixtures deciding what is correct
  ([ADR-0017](0017-wire-format-fixtures.md)).
* The install path gains a destination shaped like a game's own mod folder rather
  than a loader's, which the registry has to name.
* A session cannot start the instant a game launches: the mod connects when the
  page loads. That is a worse experience than the pipe and is accepted, because
  the alternative for these games is reading numbers off the screen.
