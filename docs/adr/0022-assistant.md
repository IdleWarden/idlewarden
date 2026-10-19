# ADR-0022: The assistant answers questions and never acts

**Status:** Accepted · **Date:** 2026-10-19

## Context

The Core watches a game and acts on it. A user playing the same game wants
something else from the same data: someone to ask "what should I do next here?"
and get an answer that knows the current screen, resources and progress.

[ADR-0008](0008-agent.md) already keeps an LLM out of the hot loop and lets it
propose goals rarely. A chat is a different use: it is driven by the user, it is
slow on purpose, and nobody expects it to press anything.

## Decision

**The assistant is a chat that reads the live signals and answers in text.** The
command line it starts passes `--tools ""` (no built-in tools),
`--strict-mcp-config` (no MCP servers), `--disable-slash-commands` and
`--setting-sources user` (no project settings), and runs in an app-owned folder holding only the prompt and settings files. The model cannot read a file, run a command or send an `Intent`, and
nothing it says reaches the Governor, because nothing it says is a command
([ADR-0009](0009-governor.md)). Hooks are turned off with a settings file that
sets `disableAllHooks`; the user's other Claude Code settings still load, since
`--bare` would require API-key authentication. The `init` line of the stream
lists the tools, and the app kills the run and reports an error if it is not
empty. Checked against Claude Code 2.1.299: the init line reports `"tools":[]`,
`"mcp_servers":[]` and no hook starts. On a timeout the app ends the whole
process tree on Windows (`taskkill /T`); elsewhere only the `claude` process
itself is killed.

**Its backend is the Claude Code installation the user already has.** The desktop
app starts the `claude` executable it finds on the `PATH` or in the Claude
desktop app folder, passes the question and the current observation on stdin, and
returns the answer as an event as each message completes. IdleWarden stores no key and holds no
account: the user's own login decides who is billed and what leaves the machine.
When Claude Code is missing or signed out, the chat says so and nothing else
changes.

**Game knowledge is data in the plugin.** A plugin may carry an `assistant.md`
next to its `plugin.json`. Its text is appended to the assistant's prompt. The
crates learn nothing about any game ([CONTRIBUTING](../../CONTRIBUTING.md)), and
the file is read, never executed, so it needs no capability.

**It is a screen of the main window, not part of the overlay.** Typing needs
keyboard focus, and [ADR-0020](0020-overlay.md) makes the overlay unable to take
it, so a chat there would break the property the overlay is built on. The main
window is a normal window and takes focus like any other.

## Why

* A model that cannot act cannot be argued into acting. The safety story is one
  flag, tested, instead of a policy about which tools are acceptable.
* Reusing the user's Claude Code costs the project nothing to run and ships no
  secret.
* The answer is only as good as the signals. A bridged mod, whose signals are
  certain, gives much better answers than a screen reader, and that is a reason
  to write mods, not a reason to let the assistant read the game itself.

## Consequences

* One session id is kept per app run, so a conversation continues until the user
  resets it.
* Resuming a session means Claude Code keeps the conversation, game signals
  included, in its own transcript store under the user's home folder, in one
  project entry, because every conversation runs in the same app-owned folder
  (`assistant` under the app's local data directory). The chat hint says so. "Nouvelle conversation" only forgets the session id:
  the transcript stays in Claude Code's history until the user deletes it there.
* The app trusts any executable that sits where Claude Code installs itself (the
  Claude desktop app folder, then the `PATH`) and prints a "Claude Code"
  version banner. Those folders are writable by the user, so a planted binary
  would receive the prompt and the signals; this is accepted, since such a
  program already runs with the user's rights.
* Because `--setting-sources user` stays, the user's own settings and memory
  (`CLAUDE.md`, `env`, permission rules) apply to the Gardien as they would to
  any Claude Code run, so answers are not reproducible across users.
* The observation is sent as it is, truncated to a fixed size. A plugin that
  puts something private in a signal puts it in the prompt.
* An assistant that edits a mod and rebuilds it is a different feature with a
  different risk, and is not what this ADR allows.
