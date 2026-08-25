# ADR-0015 — calver-short everywhere, including the contract

**Status:** Accepted · **Date:** 2026-08-25 · **Supersedes:** [ADR-0013](0013-versioning.md)

## Decision

Every released unit uses **calver-short** (`YY.M.PATCH`, e.g. `26.8.1`). No
exceptions: crates, the desktop app, first-party plugins, the registry, the
cloud repository. `versioning` is set once on the workspace and no package
overrides it.

This replaces the split in ADR-0013, which kept semver for the crates and
`api_version` and used calver-short only for plugins.

## Why

One scheme is one thing to know. The split asked every contributor to hold two
models at once and to decide, per package, which one applied. In practice the
crates in this repository are compiled, released and installed together as one
product, so their independent semver numbers were never going to carry
independent meaning: bumping `capture` to `0.4.0` while `vision` sat at `0.2.1`
would have described the release process, not compatibility.

A calendar version describes what a release actually is here: the state of the
project at a point in time.

## What this costs, stated plainly

ADR-0013 was right that `api_version` is the one field where compatibility had
meaning, and this decision gives that up.

`ApiVersion::is_satisfied_by_host` still runs and still refuses plugins, but what
it now enforces is **temporal proximity, not compatibility**. A plugin declaring
`^26.8` is refused the moment the host reaches `27.x`, which happens in January
for no reason connected to the contract. Conversely a genuinely breaking change
to `Observation` or `Intent` in `26.9` will be accepted by a plugin built
against `26.8`, because caret ranges see a minor bump.

So the host can no longer say "I broke the contract" through a version number.
Until something replaces that signal, breaking the plugin contract is a thing we
have to catch in review rather than at load time. Options worth exploring, none
of them decided here:

* a separate `contract_revision` integer in the manifest, bumped by hand and
  independent of the release calendar,
* a capability-style declaration where a plugin names the fields it relies on.

## What still works unchanged

`calver-short` is `^(\d{2})\.(\d{1,2})\.(\d{1,2})$`: three numeric components, no
leading zeros. That is also valid semver and it sorts correctly
(`26.8.1 < 26.9.0 < 27.1.0`), so `semver::Version` in the host, the registry
schema patterns and update comparison need no special case.

The existing `@v0.1.0` tags are below `26.8.0`, so the first calendar release
moves forward without a collision.

## Consequences

* `API_VERSION` in `crates/plugin-api` is a hand-maintained constant that no
  longer matches the crate version, and now never will by accident. Either wire
  it into the release config or delete it in favour of reading the crate version.
* Every `api_version` requirement already written (`^0.1` in the registry
  templates, the reference mod, the bridge tests) is stale and will refuse
  everything once the first calendar release lands.
* A patch release in a new month is `26.9.0`, not `26.8.4`. That is the scheme
  working as intended, not a mistake.
