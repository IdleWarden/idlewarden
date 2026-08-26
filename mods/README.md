# Mods

A mod runs **inside the game process** and exposes a bridge endpoint that
IdleWarden connects to ([ADR-0014](../docs/adr/0014-bridge.md)). It is what
turns "read the number off the screen with OCR" into "read the field", and it is
the only supported way to drive several games at once, because `SendInput`
reaches the foreground window and nothing else.

**IdleWarden never injects anything.** The user installs the mod through the
game's own loader. This directory holds first-party mods and the library
third-party authors build on.

```
mods/
├── src/IdleWarden.Bridge/      ← the protocol, Apache-2.0, no dependencies
├── src/IdleWarden.Reference/   ← a BepInEx plugin that speaks it end to end
└── tests/                      ← the wire format, which is a cross-language contract
```

`IdleWarden.Bridge` is Apache-2.0 for the same reason `crates/plugin-api` is
([ADR-0011](../docs/adr/0011-licensing.md)): it is the contract, so anyone must
be able to build against it under any licence they like. The reference mod is
MPL-2.0 like the rest of the project.

## Writing one

Implement two methods and hand them to a server:

```csharp
internal sealed class MyBridge : IGameBridge
{
    public string PluginId => "dev.you.my-game";
    public string ApiVersion => "^0.1";

    public IReadOnlyList<Signal> Observe() => new List<Signal>
    {
        new Signal("resource.gold", Value.Int(GameState.Instance.Gold)),
        new Signal("ui.screen_id", Value.Enum(GameState.Instance.Screen.Name)),
    };

    public ActionOutcome Act(Intent intent) => intent.Name switch
    {
        "buy_upgrade" => Buy(intent.Parameter("tier").AsInt()),
        _ => ActionOutcome.Rejected("unknown intent"),
    };
}
```

```csharp
[BepInPlugin("com.you.my-game", "My game bridge", "1.0.0")]
public sealed class MyPlugin : BaseUnityPlugin
{
    private BridgeServer server;

    private void Awake()
    {
        server = new BridgeServer("my-game", new MyBridge(), Logger.LogInfo);
        server.Start();
    }

    private void Update() => server?.Pump();

    private void OnDestroy() => server?.Dispose();
}
```

### The three things that are easy to get wrong

**`Pump` is not optional.** Pipe IO runs on its own thread, and Unity objects
may only be touched from the main thread. `BridgeServer` queues every request
and `Pump` executes it during `Update`, which is why `Observe` and `Act` are safe
to write as if they were normal game code. Forget the `Pump` call and every
request times out.

**An outcome is a post-condition, not an acknowledgement.** Return
`ActionOutcome.Succeeded` after checking the world actually changed, never
because you called a method ([ADR-0003](../docs/adr/0003-intent-vs-input.md)).
An intent with no verifiable post-condition is a bug.

**There is no confidence field, on purpose.** The Core stamps every bridged
signal as certain, because a bridge reads what the game already knows. If you
are not sure of a value, omit the signal instead of reporting a weak one.

## Why the protocol carries its own JSON reader

A mod is loaded into a process whose assembly versions nobody controls.
System.Text.Json and Newtonsoft both drag dependencies that collide with what
Unity games already ship, and a collision breaks the game, not just the mod. The
protocol is small and fully specified, so `Json.cs` costs less than that risk.

## Building

```bash
dotnet test  tests/IdleWarden.Bridge.Tests
dotnet build src/IdleWarden.Reference --configuration Release
```

Drop the resulting `IdleWarden.Reference.dll` and `IdleWarden.Bridge.dll` into
`BepInEx/plugins/` in the game directory.

## Versions

Both projects are release units in the root `.ferrflow`, tagged `mod/bridge@v...`
and `mod/reference@v...`, on the same calver-short scheme as everything else
([ADR-0015](../docs/adr/0015-calver-everywhere.md)).

FerrFlow writes `<Version>` in each `.csproj`, and that single value drives
everything downstream: the assembly version, and `MyPluginInfo.PLUGIN_VERSION`,
which is what `[BepInPlugin]` reports to BepInEx.

That last part is the whole point. BepInEx reads the version from the attribute,
not from the assembly, so a literal there would silently disagree with the tag
the moment either moved. CI greps for a version literal inside `[BepInPlugin]`
and fails the build if one comes back.

## Known gaps

The transport is Windows named pipes only. The Rust client also speaks Unix
domain sockets, but .NET maps `NamedPipeServerStream` onto a socket path that
does not match, so a Linux mod needs an explicit `UnixDomainSocket` transport
here before that side works.

The reference mod exposes a synthetic counter, not a real game. It proves the
pipe end to end and shows the shape; it is not a plugin for anything.
