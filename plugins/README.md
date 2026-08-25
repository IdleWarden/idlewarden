# First-party plugins

One folder per game. These live inside the monorepo on purpose: at this stage a
single CI pipeline and a single release cadence beat twenty repositories.

A plugin moves to its own repository the day it has a distinct maintainer and a
distinct release rhythm — not before.

Third-party plugins are never here. They live in their authors' repositories and
are indexed by [`idlewarden/registry`](https://github.com/idlewarden/registry).

Start from [`example-game/`](example-game/) — it is a complete tier-1
declarative plugin with no code in it.
