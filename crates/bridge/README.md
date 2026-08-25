# idlewarden-bridge

Client for a mod the user installed in the game process ([ADR-0014](../../docs/adr/0014-bridge.md)).

**Nothing in this crate injects anything.** The mod is a server, placed there by
the user through the game's own loader (BepInEx, MelonLoader, Reloaded-II). We
open a connection to it and fail cleanly when it is not there.

## Why a mod at all

Two things a bridge buys that capture cannot:

- **Certainty.** The game already holds `gold` in a variable. Reading it beats
  running OCR over a rendered number.
- **Parallelism.** `SendInput` reaches the foreground window and nothing else,
  so the Core can only actuate one game at a time. A pipe per instance has no
  such limit, which is the only supported route to running several games at once.

## Writing the mod side

The mod is the **server**. One JSON object per line, request in, response out,
in order. Transport is `\.\pipe\idlewarden.<name>` on Windows and
`$XDG_RUNTIME_DIR/idlewarden.<name>.sock` on Linux, where `<name>` is the value
of the plugin's `bridge:<name>` capability.

### Handshake

The Core opens with:

```json
{"request":"hello","api_version":"0.1.0"}
```

Answer with the plugin the mod belongs to and the contract you were built
against, as a semver requirement:

```json
{"response":"hello","plugin":"dev.example.cookie-clicker","api_version":"^0.1"}
```

A requirement the host does not satisfy is refused here, before any other
message.

### Observing

```json
{"request":"observe"}
{"response":"observed","signals":[{"id":"resource.gold","value":{"type":"int","value":42}}]}
```

There is no confidence field, deliberately. The Core stamps every bridged
signal `CERTAIN`; a mod that is not sure of a value must omit the signal rather
than report a weak one. The Core also stamps `frame_id` and `captured_at_ms`
itself, so a bridged observation is never stale.

Signal ids must match the plugin's declared schema, same as any other source.

### Acting

```json
{"request":"act","intent":{"name":"buy_upgrade","params":{"tier":{"type":"int","value":3}}}}
{"response":"acted","outcome":{"outcome":"succeeded"}}
```

The mod owns the translation from intent to game action, exactly as a plugin
owns the translation from intent to clicks (ADR-0003). The outcome is a
**post-condition**, not an acknowledgement: return `succeeded` only after
observing that the world changed.

```json
{"response":"acted","outcome":{"outcome":"failed","reason":"not affordable"}}
```

### Refusing

Any request may be answered with:

```json
{"response":"error","message":"the game is still loading"}
```

This surfaces as `BridgeError::Refused` with your message. Use it rather than
returning an empty observation.

## What the Core still does

Everything downstream is unchanged. The agent, the Governor, the kill switch and
the session state machine cannot tell a bridged observation from a perceived
one. The Governor's rate limit and intent allow-list matter more here, not less:
a bridged action is instant and free, so nothing else slows a runaway agent.
