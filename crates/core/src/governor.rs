// SPDX-License-Identifier: MPL-2.0
//! The Governor (ADR-0009).
//!
//! **An agent cannot police itself**, so the limits live in the Core and every
//! intent passes through here before it can become input. This is the component
//! the project is named after.

use idlewarden_plugin_api::{Confidence, Intent, Observation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Allow,
    Reject {
        reason: String,
    },
    /// Stop the agent entirely until a human resumes it.
    Halt {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernorConfig {
    /// Hard ceiling on action rate. Idle games need single digits.
    pub max_actions_per_minute: u32,
    /// Below this confidence, we stop rather than guess.
    pub min_confidence: f64,
    /// An observation older than this is not a basis for acting.
    pub max_observation_age_ms: u64,
    /// Wall-clock ceiling for one session.
    pub max_session_minutes: u32,
    /// Intents the active profile is allowed to emit. Empty means "all".
    pub allowed_intents: Vec<String>,
}

impl Default for GovernorConfig {
    fn default() -> Self {
        GovernorConfig {
            max_actions_per_minute: 60,
            min_confidence: 0.75,
            max_observation_age_ms: 2_000,
            max_session_minutes: 480,
            allowed_intents: Vec::new(),
        }
    }
}

pub struct Governor {
    config: GovernorConfig,
    /// Timestamps (ms since session start) of recently allowed actions.
    recent: Vec<u64>,
    started_at_ms: u64,
}

impl Governor {
    pub fn new(config: GovernorConfig, started_at_ms: u64) -> Self {
        Governor {
            config,
            recent: Vec::new(),
            started_at_ms,
        }
    }

    pub fn config(&self) -> &GovernorConfig {
        &self.config
    }

    /// The single choke point. Order matters: halts are checked before
    /// rejections, so a failing session stops instead of spinning on refusals.
    pub fn review(&mut self, intent: &Intent, obs: &Observation, now_ms: u64) -> Verdict {
        let elapsed_min = (now_ms.saturating_sub(self.started_at_ms)) / 60_000;
        if elapsed_min >= self.config.max_session_minutes as u64 {
            return Verdict::Halt {
                reason: "session time budget exhausted".into(),
            };
        }

        let weakest = obs.weakest_confidence();
        if weakest < Confidence::new(self.config.min_confidence) {
            return Verdict::Halt {
                reason: format!(
                    "perception confidence {:.2} below floor {:.2}",
                    weakest.get(),
                    self.config.min_confidence
                ),
            };
        }

        let age = obs.age_ms(now_ms);
        if age > self.config.max_observation_age_ms {
            return Verdict::Reject {
                reason: format!("observation is {age}ms stale"),
            };
        }

        if !self.config.allowed_intents.is_empty()
            && !self.config.allowed_intents.contains(&intent.name)
        {
            return Verdict::Reject {
                reason: format!("intent `{}` is not allowed by this profile", intent.name),
            };
        }

        self.recent.retain(|t| now_ms.saturating_sub(*t) < 60_000);
        if self.recent.len() >= self.config.max_actions_per_minute as usize {
            return Verdict::Reject {
                reason: "action rate limit reached".into(),
            };
        }

        self.recent.push(now_ms);
        Verdict::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlewarden_plugin_api::{Signal, SignalId, Value};

    fn obs(conf: f64, captured_at_ms: u64) -> Observation {
        Observation {
            frame_id: 1,
            captured_at_ms,
            signals: vec![Signal {
                id: SignalId("ui.screen_id".into()),
                value: Value::Enum("main".into()),
                confidence: Confidence::new(conf),
            }],
        }
    }

    #[test]
    fn allows_a_normal_action() {
        let mut g = Governor::new(GovernorConfig::default(), 0);
        assert_eq!(
            g.review(&Intent::new("click"), &obs(0.9, 100), 200),
            Verdict::Allow
        );
    }

    #[test]
    fn halts_when_confidence_collapses() {
        let mut g = Governor::new(GovernorConfig::default(), 0);
        assert!(matches!(
            g.review(&Intent::new("click"), &obs(0.10, 100), 200),
            Verdict::Halt { .. }
        ));
    }

    #[test]
    fn rejects_a_stale_observation() {
        let mut g = Governor::new(GovernorConfig::default(), 0);
        assert!(matches!(
            g.review(&Intent::new("click"), &obs(0.9, 0), 10_000),
            Verdict::Reject { .. }
        ));
    }

    #[test]
    fn enforces_the_rate_limit() {
        let cfg = GovernorConfig {
            max_actions_per_minute: 2,
            ..Default::default()
        };
        let mut g = Governor::new(cfg, 0);
        assert_eq!(
            g.review(&Intent::new("a"), &obs(1.0, 1000), 1000),
            Verdict::Allow
        );
        assert_eq!(
            g.review(&Intent::new("a"), &obs(1.0, 1001), 1001),
            Verdict::Allow
        );
        assert!(matches!(
            g.review(&Intent::new("a"), &obs(1.0, 1002), 1002),
            Verdict::Reject { .. }
        ));
    }

    #[test]
    fn rejects_intents_outside_the_profile() {
        let cfg = GovernorConfig {
            allowed_intents: vec!["buy_upgrade".into()],
            ..Default::default()
        };
        let mut g = Governor::new(cfg, 0);
        assert!(matches!(
            g.review(&Intent::new("sell_everything"), &obs(1.0, 100), 200),
            Verdict::Reject { .. }
        ));
    }
}
