# JavaScript mods

A mod for an Electron or browser game ([ADR-0018](../../docs/adr/0018-bridge-websocket.md)).
The page cannot serve a pipe, so the roles swap: IdleWarden listens on
`127.0.0.1`, and the mod connects to `ws://127.0.0.1:47825/<endpoint>`. Above the
socket, the protocol is the one the C# side speaks, message for message.

```
mods/js/
├── idlewarden-bridge/bridge.js   ← the protocol, Apache-2.0, no dependencies
├── cookie-clicker/               ← a mod for Cookie Clicker's own loader
└── tests/                        ← the wire format, which is a cross-language contract
```

## Writing one

```js
IdleWardenBridge.serve({
  endpoint: "my-game",
  plugin: "dev.you.my-game",
  observe: () => [
    IdleWardenBridge.signal("resource.gold", IdleWardenBridge.Value.int(Game.gold)),
    IdleWardenBridge.signal("ui.screen_id", IdleWardenBridge.Value.enumeration(Game.screen)),
  ],
  act: (intent) =>
    intent.name === "buy_upgrade"
      ? buy(intent.params.tier.value)
      : IdleWardenBridge.Outcome.rejected("unknown intent"),
});
```

### The three things that are easy to get wrong

**The host is not always there.** It listens while a session is starting, not for
as long as the game runs. `serve` reconnects on its own every few seconds; do not
treat a closed socket as an error worth reporting to the player.

**An outcome is a post-condition, not an acknowledgement.** Return `succeeded`
after checking the world changed, never because you called a method
([ADR-0003](../../docs/adr/0003-intent-vs-input.md)).

**There is no confidence field, on purpose.** The Core stamps every bridged signal
as certain, because a mod reads what the game already knows. If you are not sure
of a value, omit the signal.

## Cookie Clicker

The game loads mods from `resources/app/mods/local/<id>/`, each with an `info.txt`
and a `main.js`. `build.mjs` concatenates the library and the mod into that
`main.js`, so there is one copy of the protocol in the repository and none in the
shipped file's history:

```bash
node mods/js/build.mjs out    # writes out/idlewarden-bridge/{main.js,info.txt}
node --test mods/js/tests/bridge.test.mjs
```

`info.txt` deliberately leaves `AllowSteamAchievs` unset, so Cookie Clicker blocks
Steam achievements while the mod is enabled. A session that plays for you should
not be unlocking them.

Each `mod/cookie-clicker@v...` release carries that folder as a zip with a build
provenance attestation, which is what a registry entry under `mods/` names.
