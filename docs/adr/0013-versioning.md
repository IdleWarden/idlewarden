# ADR-0013 — calver-short for plugins, semver for the contract

**Status:** Accepted · **Date:** 2026-08-25

## Decision

Releases are managed by **FerrFlow**, one `.ferrflow` per repository, with one
`package` entry per independently released unit.

| Unit | Strategy | Example |
|---|---|---|
| Rust crates (`plugin-api`, `core`, …) | **semver** | `0.3.1` |
| `api_version` in a manifest | **semver requirement** | `^0.1` |
| Plugins | **calver-short** (`YY.M.PATCH`) | `26.8.1` |

### Crates carry an explicit version

Every crate declares `version = "x.y.z"` in its own `Cargo.toml` rather than
`version.workspace = true`. Inheritance reads as tidier and makes the table
above impossible: one workspace version means one version for everything, so
"independently released unit" would be a claim the manifests contradict.
FerrFlow refuses the inherited form outright (E4103) for the same reason.

## Why plugins are not semver

Semver encodes *compatibility*. A plugin has no API and no consumers — it either
still matches the current build of a game or it does not, and that question is
answered by `game.tested_versions`, not by a version number. Calling a release
"2.0.0" because a publisher moved a button would be decoration pretending to be
a contract.

A plugin release tracks the *game's* calendar, so a calendar version is the
honest description of what it is.

## Why the contract stays semver

`api_version` is the one place where compatibility genuinely has meaning:
a host must be able to refuse a plugin built against an incompatible contract
(ADR-0010). Calver there would destroy the only information the field carries.

## The convenient accident

`calver-short` is `^(\d{2})\.(\d{1,2})\.(\d{1,2})$` — three numeric components,
no leading zeros. That is **also valid semver**, and it sorts correctly
(`26.8.1 < 26.9.0 < 27.1.0`). So the registry schema, `semver::Version` in the
host, and update comparison all work unchanged. No special case anywhere.

Worth stating explicitly because it is load-bearing: had we picked `YY.MM`
(two components, leading zeros) none of that would hold.

## Why one `.ferrflow` per repository, not per folder

FerrFlow's model is a **workspace with a `package` array**, each entry carrying
its own `path`, `versioning`, `changelog` and `tagTemplate`. Independent
per-plugin versions, changelogs and tags come from *entries*, not from scattered
config files.

So: the monorepo has **one** root `.ferrflow` listing every crate and every
first-party plugin, and a third-party plugin repository has its own `.ferrflow`
with a single package. Both give per-plugin release cadence; only the first
avoids N files drifting out of sync.
