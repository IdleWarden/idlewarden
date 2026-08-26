// SPDX-License-Identifier: MPL-2.0
//! Hot reload (ADR-0012).
//!
//! Plugin data is swapped live; the Core is not, and capabilities never are.
//! Two invariants make this safe rather than merely convenient:
//!
//! 1. **A swap only happens between agent ticks, with no action in flight.**
//!    Swapping mid-sequence would leave a half-executed macro running against
//!    coordinates that no longer exist.
//! 2. **A failed validation keeps the running version.** Saving a broken
//!    `rules.json` must never take a live session down.

use std::sync::Arc;

/// What changed on disk. Each variant carries a different blast radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// `rules.json`, extractors, intents, tree.
    Rules,
    /// Anything under `assets/`.
    Assets,
    /// `profiles/*.json`.
    Profiles,
    /// The `presentation` block; the runtime never reads it.
    Presentation,
    /// `signals` / `intents` schema. Live, but the tree must reset.
    Schema,
    /// `capabilities` or `api_version`. **Never** applied live.
    Privileged,
}

impl ChangeKind {
    /// Privileged changes need the same consent prompt as a fresh install: the
    /// host is already watching this directory, so honouring capability grants
    /// from it would let a plugin escalate itself by writing to its own file.
    pub fn is_hot_reloadable(self) -> bool {
        !matches!(self, ChangeKind::Privileged)
    }

    /// A schema change invalidates the tree's cursor, which may point at an
    /// intent that no longer exists.
    pub fn requires_tree_reset(self) -> bool {
        matches!(self, ChangeKind::Schema)
    }
}

/// Why a swap is not being applied right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapBlocked {
    /// An intent is mid-execution. The reload waits for a terminal outcome.
    ActionInFlight,
    /// Needs explicit user consent, not a file watcher.
    NeedsConsent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapDecision {
    ApplyNow { reset_tree: bool },
    Defer(SwapBlocked),
}

/// The gate every pending reload passes through.
///
/// Deliberately a pure function of (what changed, is an action running): it is
/// the kind of logic that is easy to get subtly wrong and trivial to test.
pub fn decide_swap(kind: ChangeKind, action_in_flight: bool) -> SwapDecision {
    if !kind.is_hot_reloadable() {
        return SwapDecision::Defer(SwapBlocked::NeedsConsent);
    }
    if action_in_flight {
        return SwapDecision::Defer(SwapBlocked::ActionInFlight);
    }
    SwapDecision::ApplyNow {
        reset_tree: kind.requires_tree_reset(),
    }
}

/// The live, swappable half of a loaded plugin. Held behind an `Arc` so a swap
/// is a pointer store and readers never block.
#[derive(Debug)]
pub struct PluginRuntime {
    /// Monotonic, bumped on every successful swap. Surfaced in the activity log
    /// so "did my edit actually apply" is an answerable question.
    pub generation: u64,
    pub rules_json: String,
    pub profile_json: String,
}

#[derive(Debug)]
pub struct ReloadOutcome {
    pub generation: u64,
    pub reset_tree: bool,
}

/// Applies a validated swap. Note what is *absent*: the Governor is not passed
/// in and cannot be reset here. Its counters deliberately survive a reload,
/// otherwise "touch a file" would be an escape hatch from the rate limit, and
/// every limit in ADR-0009 would become advisory.
pub fn apply(
    current: &Arc<PluginRuntime>,
    next: PluginRuntime,
    reset_tree: bool,
) -> (Arc<PluginRuntime>, ReloadOutcome) {
    let generation = current.generation + 1;
    let swapped = Arc::new(PluginRuntime { generation, ..next });
    (
        swapped,
        ReloadOutcome {
            generation,
            reset_tree,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_swap_immediately_when_idle() {
        assert_eq!(
            decide_swap(ChangeKind::Rules, false),
            SwapDecision::ApplyNow { reset_tree: false }
        );
    }

    #[test]
    fn a_swap_waits_for_an_action_to_finish() {
        assert_eq!(
            decide_swap(ChangeKind::Rules, true),
            SwapDecision::Defer(SwapBlocked::ActionInFlight)
        );
    }

    #[test]
    fn a_schema_change_resets_the_tree() {
        assert_eq!(
            decide_swap(ChangeKind::Schema, false),
            SwapDecision::ApplyNow { reset_tree: true }
        );
    }

    #[test]
    fn capabilities_never_hot_reload_even_when_idle() {
        assert_eq!(
            decide_swap(ChangeKind::Privileged, false),
            SwapDecision::Defer(SwapBlocked::NeedsConsent)
        );
    }

    #[test]
    fn generation_increments_so_the_log_can_prove_the_swap() {
        let cur = Arc::new(PluginRuntime {
            generation: 7,
            rules_json: "{}".into(),
            profile_json: "{}".into(),
        });
        let next = PluginRuntime {
            generation: 0,
            rules_json: "{\"a\":1}".into(),
            profile_json: "{}".into(),
        };
        let (swapped, outcome) = apply(&cur, next, false);
        assert_eq!(swapped.generation, 8);
        assert_eq!(outcome.generation, 8);
    }
}
