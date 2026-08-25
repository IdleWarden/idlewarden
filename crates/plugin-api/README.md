# idlewarden-plugin-api

**The contract.** The only crate a plugin author or an out-of-process
integration ever depends on.

**Licensed Apache-2.0**, unlike the rest of the workspace (MPL-2.0), so a plugin
can be licensed however its author likes — see [ADR-0011](../../docs/adr/0011-licensing.md).

## What lives here

| Module | Carries |
|---|---|
| `manifest` | `PluginManifest`, `ApiVersion`, `GameMatcher`, `SignalDecl` |
| `observation` | `Observation`, `Signal`, `Confidence` |
| `action` | `Intent`, `InputCommand`, `ActionOutcome` |
| `capability` | `Capability`, `TrustLevel` |
| `value` | `Value` — dynamically typed signal values |

## The invariant

**Everything here is serialisable, and the contract is the data shape — not the
Rust types.** The types are a convenience for writing a host or an
out-of-process plugin in Rust; the actual interface is the JSON schema and the
message shapes ([ADR-0010](../../docs/adr/0010-versioned-contract.md)).

That is the only reason a stable plugin API is achievable at all: Rust has no
stable ABI, so a trait-based boundary could never be one.

## Changing this crate

Adding an optional field is a minor bump. Changing what a field *means* is a
major one, even when the type is unchanged. Nothing but review enforces that
distinction, so reviewers must.
