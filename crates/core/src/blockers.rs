// SPDX-License-Identifier: MPL-2.0
use idlewarden_agent::Condition;
use idlewarden_plugin_api::{Observation, Value};
use serde::Serialize;

use crate::rules::IntentRule;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Unmet {
    pub condition: Condition,
    pub actual: Option<Value>,
}

impl IntentRule {
    pub fn unmet(&self, observation: &Observation) -> Vec<Unmet> {
        self.when
            .iter()
            .filter(|condition| !condition.met(observation))
            .map(|condition| Unmet {
                condition: condition.clone(),
                actual: observation
                    .get(condition.signal())
                    .map(|signal| signal.value.clone()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlewarden_plugin_api::manifest::SignalId;
    use idlewarden_plugin_api::{Confidence, Signal};

    fn observation(signals: &[(&str, Value)]) -> Observation {
        Observation {
            frame_id: 1,
            captured_at_ms: 0,
            signals: signals
                .iter()
                .map(|(id, value)| Signal {
                    id: SignalId((*id).to_owned()),
                    value: value.clone(),
                    confidence: Confidence::new(1.0),
                })
                .collect(),
        }
    }

    fn rule(when: Vec<Condition>) -> IntentRule {
        IntentRule {
            name: "buy".to_owned(),
            when,
            commands: Vec::new(),
            post_condition: Vec::new(),
            params: Default::default(),
            min_confidence: 0.7,
            within_ms: 0,
        }
    }

    fn is_true(signal: &str) -> Condition {
        Condition::IsTrue {
            signal: signal.to_owned(),
        }
    }

    #[test]
    fn a_rule_whose_conditions_all_hold_has_nothing_unmet() {
        let seen = observation(&[("battle.active", Value::Bool(true))]);
        assert!(rule(vec![is_true("battle.active")]).unmet(&seen).is_empty());
    }

    #[test]
    fn only_the_conditions_that_fail_are_reported_with_what_was_read() {
        let seen = observation(&[
            ("save.loaded", Value::Bool(true)),
            ("research.affordable", Value::Int(0)),
        ]);
        let wants = rule(vec![
            is_true("save.loaded"),
            Condition::AtLeast {
                signal: "research.affordable".to_owned(),
                value: 1.0,
            },
        ]);

        let unmet = wants.unmet(&seen);

        assert_eq!(unmet.len(), 1);
        assert_eq!(unmet[0].condition.signal(), "research.affordable");
        assert_eq!(unmet[0].actual, Some(Value::Int(0)));
    }

    #[test]
    fn a_signal_the_plugin_never_reports_is_unmet_with_no_value() {
        let seen = observation(&[]);
        let unmet = rule(vec![is_true("battle.active")]).unmet(&seen);

        assert_eq!(unmet.len(), 1);
        assert_eq!(
            unmet[0].actual, None,
            "a missing signal is a different problem from a false one"
        );
    }
}
