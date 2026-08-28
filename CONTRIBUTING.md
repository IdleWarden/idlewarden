# Contributing to IdleWarden

## Before you write code

Read [`docs/adr/`](docs/adr/). Ten decisions shape everything here, and a pull
request that contradicts one without arguing against the ADR will be closed with
a link to it. If you think an ADR is wrong, that is a legitimate pull request,
against the ADR.

Two of them are effectively constitutional:

* **ADR-0001**: the Core never loads third-party native code in-process.
* **ADR-0009**: limits live in the Core, never in the agent.

## Sign your commits off (DCO)

We use the [Developer Certificate of Origin](https://developercertificate.org/),
not a CLA. A CLA scares off contributors and only pays off if we intend to
relicense, which we do not.

```bash
git commit -s -m "vision: register anchors before matching"
```

That adds `Signed-off-by: Your Name <you@example.com>`. CI rejects commits
without it. You keep your copyright.

## Licensing of contributions

Contributions to `crates/plugin-api/` are Apache-2.0. Everything else is
MPL-2.0. Every source file starts with its SPDX identifier:

```rust
// SPDX-License-Identifier: MPL-2.0
```

## Before you open a pull request

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## What makes a good pull request here

* **Small.** One decision per PR.
* **Explains the failure it prevents**, not the code it adds. "Rejects stale
  observations so the agent cannot act on a screen that has already changed"
  beats "adds age check".
* **Tested where it can fail.** The Governor, the manifest validator and the
  coordinate maths are the parts where a bug does damage silently.

## What will not be merged

* Anything reading another process's memory, injecting code, or installing a
  driver, see [`PLUGIN_POLICY.md`](PLUGIN_POLICY.md).
* Game-specific knowledge inside `crates/`. It belongs in a plugin. If a plugin
  cannot express it, that is a gap in `plugin-api`, open an issue about the
  gap.
* `tauri::` anywhere outside `apps/desktop/`.

## Releases

FerrFlow tags, bumps and publishes on every push to `main`, all in one job.
Configured publishers run inside release mode, so there is no separate publish
step and no way to split them: a job that tags is a job that publishes.
Publishing is idempotent, a version already on the registry is skipped rather
than treated as an error.

It needs a `CARGO_REGISTRY_TOKEN` repository secret holding a crates.io token
scoped to `publish-update`. **A fork without that secret gets a red release**,
because the publishers fail after the tag and the commit have already been
pushed. That ordering is worth knowing when a release run goes red: the tag
usually landed, and only the upload did not.

`apps/desktop` and the mods are not published: the desktop crate is
`publish = false`, and the mods are .NET, which cargo has nothing to say about.

The order of the `package` array in `.ferrflow` is the publish order, and it has
to stay a topological one: a crate cannot reach crates.io before something it
depends on, or its verification build fails with `no matching package`.
