# ADR-0016: Two rulesets on `main`, and the release bot bypasses only one

**Status:** Accepted · **Date:** 2026-09-04

## Context

FerrFlow pushes its release commit straight to `main`; the commits are authored
by the `ferrflow[bot]` GitHub App. A single ruleset that requires a pull request
rejects that push, and the failure surfaces as a FerrFlow error rather than as
the policy conflict it is.

## Decision

Keep releases committing directly. Protect `main` with **two** rulesets, because
a bypass actor in GitHub skips the whole ruleset it is listed on:

* **`main-integrity`** (id 22327785), no bypass at all: block force pushes, block deletion.
  Neither rule stands in the way of an ordinary push, so the release bot obeys
  them like everyone else.
* **`main-review`** (id 22327791), bypassed by the FerrFlow GitHub App (id 3455369) and by
  repository admins: require a pull request, and require the status checks
  `check (ubuntu-latest)`, `check (windows-latest)`, `desktop`,
  `licence-headers`, `no-native-plugins`, `build` and `signed-off`.

## Why required checks sit with the pull-request rule, not against it

The tempting split is integrity plus checks on one side, review on the other.
It does not work: a ruleset's required status checks apply to **direct pushes**
as well as merges, and checks only run *after* a commit exists. A release pushed
straight to `main` can never have passing checks at push time, so that split
rejects every release. `signed-off` makes it worse, since `dco.yml` only runs on
`pull_request` and therefore never reports for a pushed commit at all.

Checks and the pull-request requirement are the same policy, "changes are
reviewed and green before they land", so they belong in the same ruleset and
share its bypass.

## Why the check names are spelled out

`check` is a matrix job. GitHub reports it as `check (ubuntu-latest)` and
`check (windows-latest)`, and a required check whose name matches nothing is
never satisfied, which locks the branch permanently. The list above was read off
an actual pull request rather than derived from job ids.

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

## Consequences

* Admins keep pushing straight to `main`, exactly as today. The review rule
  binds contributors, not the maintainer, which is honest for a single-maintainer
  repository and is one line to tighten the day that changes.
* Force pushes and branch deletion are blocked for **everyone**, including
  admins and the bot. That is the half worth having immediately, because it
  guards against accidents rather than against people.
* The exemption is tied to the App's identity. Anything able to act as
  `ferrflow[bot]` inherits it, so its installation scope is worth reviewing
  whenever repository permissions change.
* If a release ever needs to be reverted, the revert goes through the normal
  path like any other change.
