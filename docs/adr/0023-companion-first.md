# ADR-0023: IdleWarden is a companion first, and automation is opt-in

**Status:** Accepted · **Date:** 2026-10-19

## Context

The project was written as an automation platform: it watches a game, decides
and presses. That is the right tool for an idle game, where the repetition is
the chore.

It is not the only thing a player wants from the same data. Most of the time a
player wants someone beside them who knows the game: what is worth doing in the
stage they are in, what a number means, what they are missing. That needs the
same signals, the same plugins and the same bridge, and none of the input path.

[ADR-0022](0022-assistant.md) added a chat that answers from the live signals
and cannot act. Once it exists, "automation platform" no longer describes what
the program is for.

## Decision

**IdleWarden is a companion that stays with the player while they play. Helping
by acting is something the player turns on.** The session has three modes:

* **Watch.** It reads and says nothing. The user switches to it to silence the Gardien.
* **Advise.** The Gardien talks: answers questions, and speaks up on events a
  plugin declares. It presses nothing and has no tool that could.
* **Assist.** The existing agent acts through the Governor, exactly as before
  ([ADR-0008](0008-agent.md), [ADR-0009](0009-governor.md)). Nothing about the
  agent, the Governor or the kill switch changes.

**Every session will start in Advise.** Assist is never implied by
installing a plugin: it is a switch the user flips for a session.

**Current state.** The mode switch is not implemented yet. Until it lands, a
session behaves as Assist does today, and the Gardien chat of
[ADR-0022](0022-assistant.md) is the only Advise behaviour.

**A plugin that cannot act can be advisory.** A plugin whose `intents` list is
empty and which asks for no input capability can only be observed and talked
about; the Core sends a bridge only `Intent`s built from the declared ones
([ADR-0014](0014-bridge.md)), so with an empty list it sends nothing. The Core
cannot see what a user-installed mod does on its own, which is why the policy
has a reviewer read it. [PLUGIN_POLICY](../../PLUGIN_POLICY.md) treats it
differently from one that automates, because the risk it carries is different.

## What does not change

* The red line stays. The Core still never injects, reads another process's
  memory, hooks it or installs a driver ([ADR-0001](0001-plugin-model.md),
  [ADR-0014](0014-bridge.md)). A bridge is still a mod the user installed.
* An advisory plugin is still a plugin: it is loaded as data, it declares its
  signals, and `Capability::Bridge` still asks every time.
* The registry still does not carry mod binaries.

## What this does not claim

IdleWarden cannot give anyone permission from a game's publisher. Whether a mod
that reads a game's state, or an overlay that talks about it, is allowed is a
question about that publisher's terms, and the answer differs per game. The
project does not say that an advisory plugin is within any terms. It asks the
plugin's author to cite the terms or the modding policy that apply, and the
reviewer checks the citation. A game with a competitive or ranked mode is still
refused, whatever the plugin does, for the reason in the policy: advice that is
worth anything in that mode is an advantage over other players.

## Why

* The same signals serve both uses, so a companion costs one screen and a prompt,
  not a second engine.
* Starting every session in Advise means the first thing a user sees is
  help, not a program that moves their mouse.
* An assistant that cannot act is an easier thing to put in front of a game's
  rules than a bot is, and the policy can say so honestly instead of treating
  every plugin as an automation.

## Consequences

* The session state grows a mode, and the UI shows it.
* Until [#112](https://github.com/IdleWarden/idlewarden/issues/112) lands, the
  advisory classification and the review of updates are manual, so that issue
  is the real enforcement point for the advisory rules.
* The overlay needs a way to type to the Gardien. That is a change to
  [ADR-0020](0020-overlay.md), which makes the overlay unable to take focus, and
  it is made in its own decision.
* Proactive remarks come from rules declared in the plugin and triggered by
  signals, never from a model called on every tick ([ADR-0008](0008-agent.md)).
* The README and the plugin policy describe the project in this order.
