# ADR-0016: Two rulesets on `main`, and the release bot bypasses only one

**Status:** Accepted · **Date:** 2026-09-04

## Context

FerrFlow pushes its release commit straight to `main`; the commits are authored
by the `ferrflow[bot]` GitHub App. A single ruleset that requires a pull request
rejects that push, and the failure surfaces as a FerrFlow error rather than as
the policy conflict it is.

## Decision

Keep releases committing directly. Protect `main` with **two** rulesets rather
than one, because bypass in GitHub is granted per ruleset:

* **`main-integrity`**, no bypass actors: block force pushes, block deletion,
  require the status checks `check`, `desktop`, `licence-headers`,
  `no-native-plugins`, `signed-off` and `build`.
* **`main-review`**, bypass actor the FerrFlow GitHub App: require a pull
  request.

The bot therefore cannot force-push, cannot delete the branch, and cannot land
anything that failed CI. The only rule it skips is the one it structurally
cannot satisfy.

## Why not put the release back behind a pull request

`releaseCommitMode: "pr"` is the other way to make protection and releases
coexist, and it was rejected for one reason: this repository releases several
times a day under calendar versioning, and a required merge step on every
release is friction paid constantly by one maintainer. Policies that are
expensive to follow get bypassed, and a policy someone routes around is worth
less than a narrow, written exemption.

It stays the right answer if the project grows enough reviewers that a release
PR costs nothing, in which case delete `main-review`'s bypass rather than
inventing something new.

## Why not one ruleset with a bypass

A bypass actor skips the *whole* ruleset it is listed on. Putting force-push
protection and the pull-request requirement in the same ruleset would hand the
bot both. Splitting them costs one extra ruleset and keeps the exemption to the
single rule that needs it.

## Consequences

* Anyone with push access can land on `main` without review, exactly as today:
  this ADR does not make the repository stricter for humans, it makes protection
  possible to enable at all (see #24).
* The exemption is tied to the App's identity. Anything able to act as
  `ferrflow[bot]` inherits it, so its installation scope is worth reviewing
  whenever repository permissions change.
* If a release ever needs to be reverted, the revert goes through the normal
  path like any other change.
