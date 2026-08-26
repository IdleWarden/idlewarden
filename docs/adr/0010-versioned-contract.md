# ADR-0010: The plugin contract is a data schema, not a Rust ABI

**Status:** Accepted · **Date:** 2026-08-25

## Decision

The contract between IdleWarden and a plugin is:

* the **`plugin.json` manifest schema**, and
* the **message shapes** (`Observation`, `Intent`, `InputCommand`,
  `ActionOutcome`).

It is **not** a set of Rust traits a plugin links against. `api_version` is a
semver *requirement* in the manifest; the host declares the version it
implements and **refuses incompatible plugins loudly at load time**, with a
message distinguishing "this plugin is broken" from "this plugin needs a newer
IdleWarden".

Plugins declare **capabilities** (`capture`, `input.mouse`, `fs.read:<path>`,
`net:<host>`). The trust level decides which are granted silently:

| Trust | Source | Auto-update | Silent grants |
|-------|--------|-------------|---------------|
| `Official` | built and signed by the project | yes | all |
| `Verified` | reviewed PR, signed by a registered author key | yes | all but `net` |
| `Unverified` | local file or arbitrary URL | **no** | **none** |

## Why

A stable API is achievable here **only** because the contract is data. Rust
traits across a dynamic boundary have no stable ABI (ADR-0001), and a trait is
also far harder to version: adding a method is breaking, while adding an
optional manifest field is not.

Declared capabilities are what make a third-party plugin ecosystem
approvable-by-a-human rather than trust-by-hope.

## Consequences

* The JSON Schema in the `registry` repository is a **release artefact**. It
  changes under semver like any other public interface.
* Adding a field is a minor bump; changing a meaning is a major one. Reviewers
  must police that distinction, because nothing else will.
