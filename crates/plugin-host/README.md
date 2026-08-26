# idlewarden-plugin-host

Plugin loading and hot reload. Governed by
[ADR-0001](../../docs/adr/0001-plugin-model.md) and
[ADR-0012](../../docs/adr/0012-hot-reload.md).

## The root rule

**The Core never loads third-party native code into its own process.**

There is no `NativeLibrary` variant in `PluginKind` and adding one is a breaking
architectural change requiring a superseding ADR. CI enforces this: a job fails
the build if `libloading` or `dlopen2` appears in any `Cargo.toml`.

Three tiers instead:

| Tier | Form | Crash blast radius |
|---|---|---|
| Declarative | manifest + assets + rules, no code | none |
| Script | Rhai, pure Rust, no FFI | trapped |
| Out-of-process | child process over IPC | its own process |

## Hot reload (`reload.rs`)

Plugin *data* swaps live; the Core does not; **capabilities never do**. The host
already watches the plugin's directory, so honouring capability grants read from
it would let a plugin escalate its own privileges by writing to its own file.

`decide_swap` is a pure function of (what changed, is an action in flight),
easy to get subtly wrong, trivial to test.

Note what `apply` does *not* take: the Governor. Its counters survive a reload
on purpose. Otherwise "touch a file" would be an escape hatch from the rate
limit and every limit in ADR-0009 would become advisory.
