# Plugin policy

This document governs what the **official registry** distributes. It does not,
and cannot, govern what people build privately: the engine is generic. What we
curate is what we ship.

## Accepted

* Single-player and offline games.
* Idle, incremental, clicker and management games, the genre where automating
  repetition is the intended experience.
* Games whose terms of service explicitly permit automation, macros or bots.
* Games with an official, documented automation or modding interface.

## Advisory plugins

An advisory plugin is judged separately from one that automates, because it
cannot press anything. It is accepted when all of these hold:

* It is advisory: its `intents` list is empty and it requests no input
  capability (`InputMouse`, `InputKeyboard` or `InputGamepad`), so it can only be
  observed and talked about. `Capture`, `FsRead`, `Net` and `Bridge` are not
  input capabilities and stay subject to their own rules. There is no
  manifest field for this. The reviewer checks it first, in `plugin.json`.
* The game has no competitive or ranked mode. Cooperative and single-player games
  qualify for an advisory plugin, a game with ranked play does not, whatever the
  plugin does. A cooperative game with an automating plugin is not covered by
  this and goes to Grey areas. The
  reviewer checks this against the game's store page and documentation. A game
  with online leaderboards is held until a discussion thread decides it (see
  Grey areas).
* The publisher's terms or modding policy allow mods and overlays, and the pull
  request cites them. IdleWarden cannot grant that permission, and the
  citation in the pull request is a claim a reviewer checks, not a guarantee.
* It surfaces nothing the player's own client would not already show them. The
  pull request lists every signal the plugin exposes, and the reviewer checks
  that each one is visible in the game's own interface.

The ranked or competitive check does not depend on the `multiplayer` field; see
the Review layer under Enforcement. The reviewer records the answer, with its
source, in the pull request.

Advisory status is derived from the manifest, never declared: there is no
advisory flag, and `multiplayer` only triggers the ranked check. Registry CI
will classify a plugin as advisory when its manifest has empty `intents` and no
input capability, and apply the conditions above to it (tracked in #112). Until
that check exists, the registry classifies nothing: the reviewer applies the
conditions by hand, and a plugin that meets them is merged with no automated
guarantee. When #112 lands, every advisory plugin already in the registry is
re-checked against it.

An update to an advisory plugin needs a new review if it changes `intents`,
capabilities or, for a bridge, the accepted command set, which the update pull
request re-states and the reviewer re-verifies against the source. Until #112 lands, the reviewer who merges an update pull request
owns checking that its diff leaves both unchanged or triggers that review, and
does not merge it otherwise.

An advisory plugin that requests `Net` needs the same justification as any
other plugin that does, and the reviewer checks that it sends no game state off
the machine beyond what that justification covers.

An advisory plugin that uses a `Bridge` is reviewed as a bridge plugin as well:
the advisory conditions are in addition to those rules, never a replacement. A
bridge is still a mod the user installs by hand, and the registry still does not
carry the binary. The Core sends a bridge only the `Intent`s built from the
plugin's declared intents ([ADR-0014](docs/adr/0014-bridge.md)), so with an
empty list it sends nothing. That is how the Core is built, not a registry
check. A `Bridge` with empty `intents` is the weakest link of the advisory path,
because only a human reading the mod's source stands between it and commands
outside the Intent protocol. The pull request therefore states the full set of
commands the bridge accepts, and the reviewer verifies that set against the
source. #112 makes that statement a required field of the registry pull request
template, so the check does not depend on reviewer memory.

## Refused

* Any game with a competitive or ranked multiplayer mode, whether or not the
  plugin targets that mode, advisory or not, because advice is an advantage over
  other players too. We do not adjudicate "but I only
  use it in single-player": the answer is no.
* Anything that circumvents, disables, probes or works around an anti-cheat
  system or any other protection mechanism.
* Plugins relying on memory reading, DLL injection, code hooking or driver
  installation. This includes **bridge plugins** (ADR-0014), which depend on a
  mod running inside the game process. Such a plugin is perfectly legitimate to
  build and to install by hand; it is the *registry* that will not carry it,
  because we cannot review a binary we do not host and would not want to host.
  Requests to relax this will be declined.
* Games whose terms explicitly forbid automation, regardless of the mode.
* Anything designed to obtain a competitive advantage over other human players,
  or to acquire tradeable goods for sale.

## How this is enforced

Three layers, in order of strength:

1. **Review.** Every registry submission is a pull request. The reviewer
   checks whether the game has a ranked or competitive mode on every
   submission; `multiplayer: true` in the manifest is an additional trigger,
   not the only one. A game with such a mode is refused, for every plugin. A
   cooperative-only game is held for a discussion thread if the plugin declares
   intents. A single-player game with online leaderboards is held for a
   discussion thread for every plugin, advisory or not. An advisory plugin for a
   cooperative-only game goes through the advisory conditions instead. Reviewers
   check the claim. Until the registry's own copy of this policy is updated
   (tracked in #112), its checks may still refuse `multiplayer: true`
   mechanically; an affected author opens a discussion thread.
2. **Capabilities.** A plugin declares what it needs. `Unverified` plugins get
   nothing granted silently; network access is never granted silently below
   `Official`.
3. **Architecture.** There is no code path in IdleWarden that could load a
   native plugin, read another process's memory, inject into another process or
   install a driver. This is the strongest layer precisely because it is not a
   rule anyone can waive. A bridge does not weaken it: the Core connects to an
   endpoint the user created, it never creates one, and
   `Capability::Bridge` is the single capability no trust level grants
   silently.

## Grey areas

Bring them to a discussion thread before writing the plugin. Three cases are
open: a cooperative-only multiplayer game that wants an automating plugin, a
single-player game with online leaderboards (undecided for advisory plugins
too), and a game whose terms are silent rather than permissive. They are real
cases and they get decided individually, in public, and recorded here.

## If you disagree with a refusal

Open a discussion. Decisions get revisited when the facts change, a game adds
an official API, a publisher clarifies its terms. They do not get revisited by
reopening the same pull request.
