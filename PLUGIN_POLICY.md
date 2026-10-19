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

This section and the advisory wording under Refused, the Review layer and Grey
areas apply from the merge that introduces them, which is also what accepts
[ADR-0023](docs/adr/0023-companion-first.md).
An advisory plugin is judged separately from one that automates, because it
cannot press anything. It is accepted when all of these hold:

* It is advisory: its `intents` list is empty and it requests no input
  capability (`InputMouse`, `InputKeyboard` or `InputGamepad`), so it can only be
  observed and talked about. `Capture`, `FsRead`, `Net` and `Bridge` are not
  input capabilities and stay subject to their own rules. There is no
  manifest field for this. The reviewer checks it first, in `plugin.json`.
* The game has no competitive or ranked mode. Cooperative and single-player games
  qualify, a game with ranked play does not, whatever the plugin does. The
  reviewer checks this against the game's store page and documentation. A game
  with online leaderboards is held until a discussion thread decides it (see
  Grey areas).
* The publisher's terms or modding policy allow mods and overlays, and the pull
  request cites them. IdleWarden cannot grant that permission, and the manifest
  line is a claim a reviewer checks, not a guarantee.
* It surfaces nothing the player's own client would not already show them. The
  pull request lists every signal the plugin exposes, and the reviewer checks
  that each one is visible in the game's own interface.

These conditions apply whatever the `multiplayer` field says: it is declared
by the author, so the reviewer checks the ranked or competitive mode against the
game itself.

Registry CI will reject a plugin submitted as advisory that declares an intent or
an input capability (tracked in #112). Until that check exists, the registry
does not check this: a plugin is reviewed by hand against the three
conditions above, and if it meets them it is merged with no automated
guarantee.
Any update that changes `intents` or capabilities needs a new review, and until
#112 lands the reviewer diffs both on every update pull request.
Everything else in this policy still applies to it. A bridge is still a mod the
user installs by hand, and the registry still does not carry the binary. A
bridge only executes the `Intent`s the plugin declares
([ADR-0014](docs/adr/0014-bridge.md)), so an empty `intents` list leaves it
nothing to execute.

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

1. **Review.** Every registry submission is a pull request. `multiplayer: true`
   in the manifest triggers a check of whether the game has a ranked or
   competitive mode. A game with such a mode is refused, for every plugin. A
   cooperative-only game is held until a discussion thread decides it only if
   the plugin declares intents; an advisory plugin for it goes through the
   three advisory conditions instead. Reviewers check the claim.
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
