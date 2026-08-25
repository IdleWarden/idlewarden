# ADR-0014 — L3 bridges connect to a mod the user installed; the Core never injects

**Status:** Accepted · **Date:** 2026-08-25

## Context

Two limits meet here.

`SendInput` is session-global and always reaches the foreground window, so the
Core can only actuate one game at a time (ADR-0007). And screen capture plus
classical vision is the slowest, least certain way to read a value the game
already holds in a variable.

Both dissolve if something inside the game process talks to us. The question is
who puts it there.

## Decision

A plugin may declare a **bridge**: a named IPC endpoint exposed by a mod
**the user installed themselves**, through the game's own mod loader (BepInEx,
MelonLoader, Reloaded-II) or by dropping a file into the game directory.

* **IdleWarden contains no injection code.** No `CreateRemoteThread`, no
  `WriteProcessMemory`, no `SetWindowsHookEx`, no `LoadLibrary` into a foreign
  process. We connect to an endpoint; we never create one.
* Transport is a **named pipe** on Windows and a **Unix domain socket** on
  Linux, carrying newline-delimited JSON. The mod is the server, the Core is
  the client.
* The connection opens with a **handshake**. A mod that does not satisfy the
  host's `api_version` is refused at connect time, not at first use (ADR-0010).
* A bridge produces `Observation`s whose every signal is `Confidence::CERTAIN`.
  The Core sets this; the mod cannot claim a confidence of its own.
* A bridge executes `Intent`s and returns an `ActionOutcome`, exactly like the
  input path (ADR-0003). Post-conditions are still mandatory.
* Everything downstream is unchanged: the agent, the Governor, the kill switch
  and the session state machine do not know a bridge is in use.

### Gating

* `Capability::Bridge { name }` must be declared in the manifest.
* It is **never granted silently**, at any trust level, including `Official`.
  Every other capability can be granted by trust; this one always asks.
* Capabilities never hot-reload (ADR-0012), so a bridge cannot be switched on
  mid-session.
* The registry never carries a mod binary, and does not distribute third-party
  bridge plugins, exactly as it already refuses anything needing injection or
  memory reading (`PLUGIN_POLICY.md`). A third-party bridge plugin is therefore
  `Unverified`: installed by hand, no auto-update, loud warning.

### First-party mods are in scope

Writing mods is not the same act as injecting them, and this ADR does not
forbid the first. The project may develop and ship its own mods under `mods/`,
built in CI like any other artefact, and a plugin bound to one may be
`Official`. What does not change: the mod is still installed by the user through
the game's loader, and `Capability::Bridge` still asks every time. Trust decides
whether an update is automatic; it never decides whether a bridge opens.

## Why not inject ourselves

**The antivirus is the real adversary, not an anti-cheat.** Idle games rarely
have anti-cheat. But a remote-thread injector is a behavioural malware
signature: Defender and SmartScreen quarantine it on the user's machine
regardless of what the injected payload does. Shipping one means permanent
whitelisting work with AV vendors and installations lost anyway.

**A hook is specific to a build, not to a game.** Hooking Unity 2021 and Unity 6
are different jobs. Owning the injector means owning that treadmill for every
supported title; letting the modding community own it means we inherit their
work instead.

**It keeps a sentence true.** "A plugin can only look at a window and move the
mouse" is verifiable today, and the whole trust hierarchy rests on it. Injection
performed by the Core makes it false for every user, including those who never
enable a bridge. Injection performed by a mod loader the user installed keeps
the claim accurate about *IdleWarden*, and moves the trust decision to where it
belongs: an explicit, informed act by the person who owns the machine.

**It is already the promised design.** `README.md` lists L3 as "a supported mod,
a documented API, a save file read read-only, in a separate process". This ADR
implements L3; it does not invent a level.

## What the user still carries

A mod runs inside the game process with the user's privileges, and nothing in
IdleWarden can sandbox it. That risk is identical to installing any other mod,
and it is real. It must be stated on the screen that enables the bridge, not
buried in documentation.

We never host these binaries. A registry entry may link to a mod's page; the
registry never carries the file.

## Consequences

* Multi-instance becomes possible without focus: one pipe per game instance, no
  contention, no window juggling. This is the only supported route to it.
* Bridge sessions bypass capture and vision entirely, so a confidence collapse
  can no longer pause the agent. The Governor's rate limit, intent allow-list
  and session budget become the only brakes, and matter more, not less: a bridge
  action is instant and free.
* Two observation sources now exist. The Core must never mix them in one
  session: an `Observation` is either wholly perceived or wholly bridged.
* Plugin authors who want a bridge must ship and support a mod. That cost is
  deliberate. It is what keeps the declarative plugin the default.
