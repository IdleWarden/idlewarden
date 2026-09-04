// SPDX-License-Identifier: MPL-2.0
use std::collections::BTreeMap;

use idlewarden_plugin_api::{Intent, Observation, Value};
use serde::{Deserialize, Serialize};

use crate::Decider;

/// One test against a signal the plugin declared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Condition {
    IsTrue { signal: String },
    IsFalse { signal: String },
    Equals { signal: String, value: Value },
    AtLeast { signal: String, value: f64 },
    AtMost { signal: String, value: f64 },
}

impl Condition {
    pub fn signal(&self) -> &str {
        match self {
            Condition::IsTrue { signal }
            | Condition::IsFalse { signal }
            | Condition::Equals { signal, .. }
            | Condition::AtLeast { signal, .. }
            | Condition::AtMost { signal, .. } => signal,
        }
    }

    fn holds(&self, value: &Value) -> bool {
        match self {
            Condition::IsTrue { .. } => matches!(value, Value::Bool(true)),
            Condition::IsFalse { .. } => matches!(value, Value::Bool(false)),
            Condition::Equals {
                value: expected, ..
            } => value == expected,
            Condition::AtLeast { value: floor, .. } => {
                number(value).is_some_and(|found| found >= *floor)
            }
            Condition::AtMost { value: ceiling, .. } => {
                number(value).is_some_and(|found| found <= *ceiling)
            }
        }
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Int(found) => Some(*found as f64),
        Value::Float(found) | Value::Ratio(found) => Some(*found),
        _ => None,
    }
}

/// A declarative rule: when every condition holds with enough confidence,
/// propose an intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleSpec {
    pub name: String,
    #[serde(default)]
    pub when: Vec<Condition>,
    pub intent: String,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    #[serde(default = "default_floor")]
    pub min_confidence: f64,
}

fn default_floor() -> f64 {
    0.7
}

/// Turns declared rules into intents, and abstains rather than guessing.
///
/// Abstention has three causes, all of which mean the same thing to the tree:
/// this rule has nothing to say. A signal is missing, a signal is read with too
/// little confidence, or the observation is one this rule has already acted on.
pub struct RuleDecider {
    spec: RuleSpec,
    acted_on: Option<u64>,
}

impl RuleDecider {
    pub fn new(spec: RuleSpec) -> Self {
        RuleDecider {
            spec,
            acted_on: None,
        }
    }
}

impl Decider for RuleDecider {
    fn decide(&mut self, obs: &Observation) -> Option<Intent> {
        if self.acted_on == Some(obs.frame_id) {
            return None;
        }

        if self.spec.when.is_empty() {
            return None;
        }

        for condition in &self.spec.when {
            let signal = obs.get(condition.signal())?;
            if signal.confidence.get() < self.spec.min_confidence {
                return None;
            }
            if !condition.holds(&signal.value) {
                return None;
            }
        }

        self.acted_on = Some(obs.frame_id);
        Some(Intent {
            name: self.spec.intent.clone(),
            params: self.spec.params.clone(),
        })
    }

    fn name(&self) -> &str {
        &self.spec.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlewarden_plugin_api::{Confidence, Signal, SignalId};

    fn observation(frame_id: u64, signals: Vec<(&str, Value, f64)>) -> Observation {
        Observation {
            frame_id,
            captured_at_ms: frame_id * 250,
            signals: signals
                .into_iter()
                .map(|(id, value, confidence)| Signal {
                    id: SignalId(id.to_owned()),
                    value,
                    confidence: Confidence::new(confidence),
                })
                .collect(),
        }
    }

    fn spec(when: Vec<Condition>) -> RuleSpec {
        RuleSpec {
            name: "collect".to_owned(),
            when,
            intent: "collect_reward".to_owned(),
            params: BTreeMap::new(),
            min_confidence: 0.7,
        }
    }

    fn ready() -> Condition {
        Condition::IsTrue {
            signal: "ui.reward_ready".to_owned(),
        }
    }

    #[test]
    fn a_satisfied_rule_proposes_its_intent() {
        let mut decider = RuleDecider::new(spec(vec![ready()]));
        let obs = observation(1, vec![("ui.reward_ready", Value::Bool(true), 0.95)]);

        let intent = decider.decide(&obs).expect("the rule holds");

        assert_eq!(intent.name, "collect_reward");
    }

    #[test]
    fn the_intent_carries_the_declared_parameters() {
        let mut declared = spec(vec![ready()]);
        declared.params.insert("slot".to_owned(), Value::Int(3));
        let mut decider = RuleDecider::new(declared);

        let intent = decider
            .decide(&observation(
                1,
                vec![("ui.reward_ready", Value::Bool(true), 0.95)],
            ))
            .expect("the rule holds");

        assert_eq!(intent.params.get("slot"), Some(&Value::Int(3)));
    }

    #[test]
    fn a_low_confidence_signal_makes_the_rule_abstain() {
        let mut decider = RuleDecider::new(spec(vec![ready()]));
        let obs = observation(1, vec![("ui.reward_ready", Value::Bool(true), 0.4)]);

        assert!(
            decider.decide(&obs).is_none(),
            "the condition holds but the reading is not trustworthy; guessing is worse than waiting"
        );
    }

    #[test]
    fn confidence_is_judged_at_the_declared_floor_not_a_hardcoded_one() {
        let mut lenient = spec(vec![ready()]);
        lenient.min_confidence = 0.3;
        let mut decider = RuleDecider::new(lenient);

        assert!(decider
            .decide(&observation(
                1,
                vec![("ui.reward_ready", Value::Bool(true), 0.4)]
            ))
            .is_some());
    }

    #[test]
    fn a_missing_signal_makes_the_rule_abstain() {
        let mut decider = RuleDecider::new(spec(vec![ready()]));

        assert!(decider
            .decide(&observation(
                1,
                vec![("ui.something_else", Value::Bool(true), 1.0)]
            ))
            .is_none());
    }

    #[test]
    fn the_same_frame_is_never_acted_on_twice() {
        let mut decider = RuleDecider::new(spec(vec![ready()]));
        let obs = observation(7, vec![("ui.reward_ready", Value::Bool(true), 0.95)]);

        assert!(decider.decide(&obs).is_some());
        assert!(
            decider.decide(&obs).is_none(),
            "a stalled perception pass re-presents the same frame; acting again would double-click"
        );

        let fresh = observation(8, vec![("ui.reward_ready", Value::Bool(true), 0.95)]);
        assert!(
            decider.decide(&fresh).is_some(),
            "a new frame must be actionable again"
        );
    }

    #[test]
    fn a_rule_with_no_conditions_claims_nothing() {
        let mut decider = RuleDecider::new(spec(Vec::new()));

        assert!(
            decider
                .decide(&observation(
                    1,
                    vec![("ui.reward_ready", Value::Bool(true), 1.0)]
                ))
                .is_none(),
            "an empty rule would otherwise fire on every observation"
        );
    }

    #[test]
    fn every_condition_has_to_hold() {
        let mut decider = RuleDecider::new(spec(vec![
            ready(),
            Condition::IsFalse {
                signal: "ui.busy".to_owned(),
            },
        ]));

        let obs = observation(
            1,
            vec![
                ("ui.reward_ready", Value::Bool(true), 0.95),
                ("ui.busy", Value::Bool(true), 0.95),
            ],
        );

        assert!(decider.decide(&obs).is_none());
    }

    #[test]
    fn numeric_thresholds_read_ints_floats_and_ratios() {
        let at_least = Condition::AtLeast {
            signal: "gold".to_owned(),
            value: 100.0,
        };

        assert!(at_least.holds(&Value::Int(150)));
        assert!(at_least.holds(&Value::Float(100.0)));
        assert!(!at_least.holds(&Value::Int(99)));
        assert!(
            !at_least.holds(&Value::Text("lots".to_owned())),
            "a threshold against text must fail, not coerce"
        );

        let at_most = Condition::AtMost {
            signal: "health".to_owned(),
            value: 0.3,
        };
        assert!(at_most.holds(&Value::Ratio(0.25)));
        assert!(!at_most.holds(&Value::Ratio(0.5)));
    }

    #[test]
    fn is_true_does_not_accept_a_non_boolean() {
        assert!(!ready().holds(&Value::Int(1)));
        assert!(!ready().holds(&Value::Text("true".to_owned())));
    }
}
