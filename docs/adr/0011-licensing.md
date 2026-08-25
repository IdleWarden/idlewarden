# ADR-0011 — MPL-2.0 core, Apache-2.0 plugin API

**Status:** Accepted · **Date:** 2026-08-25

## Decision

* `crates/plugin-api` → **Apache-2.0**
* Everything else → **MPL-2.0**
* Contributions under the **DCO**, not a CLA.
* The name is held as a **trademark** independently of the code licence.

## Why MPL-2.0 for the core

The threat model is: *someone takes the core, closes it, rebrands it, sells it.*
MPL-2.0 blocks the "closes it" half — its copyleft is per-file, so modifications
to our files must be published — while leaving the surrounding product free.

What GPL would add is a ban on bundling the core into a larger proprietary
product. For a desktop platform whose value is its ecosystem, that is a scenario
we would mostly want to *permit*.

MPL's file-level scope is also **unambiguous at the plugin boundary**. Under GPL
our plugin model probably escapes contamination anyway — declarative plugins are
data, out-of-process plugins are the textbook non-combined case — but ecosystems
die of friction, not of lawsuits, and "ask your lawyer" is friction. Rust's
static linking adds a second cost: under GPL every statically linked dependency
becomes a licence audit, and Apache-2.0 is incompatible with GPLv2.

Decisively: **MPL-2.0 preserves the GPL option; GPL destroys the MPL option.**
MPL-2.0 is GPL-compatible by default, so a larger work can be relicensed to GPL
later without anyone's permission. The reverse needs unanimous consent from
every contributor, which with a DCO is effectively never. Starting permissive
keeps the door open; starting strict welds it shut.

## Why Apache-2.0 for the plugin API

Anything that must be adopted widely should be maximally permissive, and the
patent grant matters for a contract third parties build against. A plugin author
must never have to think about our licence.

## Why AGPL is not used

Its distinguishing mechanism — section 13, source offered to users interacting
over a network — never fires for a local desktop application. It would function
only as a corporate-policy deterrent. It becomes genuinely appropriate the day
a network service exists (a registry backend, profile sync); that component can
be AGPL on its own.

## Consequences

* Every file carries an SPDX header, and CI checks it.
* Revisit before v1.0. After the first external contributor, changing this needs
  their agreement.
