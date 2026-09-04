// SPDX-License-Identifier: MPL-2.0
use serde::{Deserialize, Serialize};

use crate::rule::{RuleDecider, RuleSpec};
use crate::{DeciderNode, Node, Selector, Sequence};

/// A behaviour tree as the plugin declares it.
///
/// The tree is data, not code: a profile carries it, the UI can render it, and
/// a new strategy is a new node type rather than a new agent (ADR-0008).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum NodeSpec {
    /// Children in order until one does not fail.
    Selector {
        children: Vec<NodeSpec>,
    },
    /// Children in order until one does not succeed.
    Sequence {
        children: Vec<NodeSpec>,
    },
    Rule(RuleSpec),
}

impl NodeSpec {
    pub fn build(&self) -> Box<dyn Node> {
        match self {
            NodeSpec::Selector { children } => Box::new(Selector {
                children: children.iter().map(NodeSpec::build).collect(),
            }),
            NodeSpec::Sequence { children } => Box::new(Sequence::new(
                children.iter().map(NodeSpec::build).collect(),
            )),
            NodeSpec::Rule(spec) => Box::new(DeciderNode {
                decider: Box::new(RuleDecider::new(spec.clone())),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Status;
    use idlewarden_plugin_api::{Confidence, Observation, Signal, SignalId, Value};

    const TREE: &str = r#"
{
  "node": "selector",
  "children": [
    {
      "node": "rule",
      "name": "collect",
      "when": [{ "op": "is_true", "signal": "ui.reward_ready" }],
      "intent": "collect_reward",
      "min_confidence": 0.8
    },
    {
      "node": "rule",
      "name": "upgrade",
      "when": [{ "op": "at_least", "signal": "gold", "value": 100.0 }],
      "intent": "buy_upgrade"
    }
  ]
}
"#;

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

    #[test]
    fn a_declared_tree_produces_intents_from_observations() {
        let spec: NodeSpec = serde_json::from_str(TREE).expect("the tree parses");
        let mut tree = spec.build();

        let tick = tree.tick(&observation(
            1,
            vec![
                ("ui.reward_ready", Value::Bool(true), 0.95),
                ("gold", Value::Int(500), 0.95),
            ],
        ));

        assert_eq!(
            tick.intent.map(|intent| intent.name),
            Some("collect_reward".to_owned()),
            "a selector must take the first rule that fires, not the last"
        );
    }

    #[test]
    fn the_selector_falls_through_to_the_next_rule() {
        let spec: NodeSpec = serde_json::from_str(TREE).expect("the tree parses");
        let mut tree = spec.build();

        let tick = tree.tick(&observation(
            1,
            vec![
                ("ui.reward_ready", Value::Bool(false), 0.95),
                ("gold", Value::Int(500), 0.95),
            ],
        ));

        assert_eq!(
            tick.intent.map(|intent| intent.name),
            Some("buy_upgrade".to_owned())
        );
    }

    #[test]
    fn a_tree_whose_rules_all_abstain_fails_rather_than_inventing_an_intent() {
        let spec: NodeSpec = serde_json::from_str(TREE).expect("the tree parses");
        let mut tree = spec.build();

        let tick = tree.tick(&observation(
            1,
            vec![
                ("ui.reward_ready", Value::Bool(true), 0.3),
                ("gold", Value::Int(10), 0.95),
            ],
        ));

        assert_eq!(tick.status, Status::Failure);
        assert!(tick.intent.is_none());
    }

    #[test]
    fn a_declared_floor_overrides_the_default() {
        let spec: NodeSpec = serde_json::from_str(TREE).expect("the tree parses");
        let mut tree = spec.build();

        let tick = tree.tick(&observation(
            1,
            vec![
                ("ui.reward_ready", Value::Bool(true), 0.75),
                ("gold", Value::Int(10), 0.95),
            ],
        ));

        assert_eq!(
            tick.status,
            Status::Failure,
            "0.75 is under the 0.8 this rule declared, even though it clears the 0.7 default"
        );
    }

    #[test]
    fn a_tree_survives_a_round_trip_through_json() {
        let spec: NodeSpec = serde_json::from_str(TREE).expect("the tree parses");

        let written = serde_json::to_string(&spec).expect("serialises");
        let read: NodeSpec = serde_json::from_str(&written).expect("parses back");

        assert_eq!(
            read, spec,
            "a profile has to be able to carry the tree and give it back unchanged"
        );
    }

    #[test]
    fn a_sequence_advances_one_step_per_tick() {
        let spec: NodeSpec = serde_json::from_str(
            r#"
{
  "node": "sequence",
  "children": [
    { "node": "rule", "name": "open", "when": [{ "op": "is_true", "signal": "a" }], "intent": "open" },
    { "node": "rule", "name": "close", "when": [{ "op": "is_true", "signal": "a" }], "intent": "close" }
  ]
}
"#,
        )
        .expect("the tree parses");
        let mut tree = spec.build();

        let first = tree.tick(&observation(1, vec![("a", Value::Bool(true), 1.0)]));
        let second = tree.tick(&observation(2, vec![("a", Value::Bool(true), 1.0)]));

        assert_eq!(first.intent.map(|i| i.name), Some("open".to_owned()));
        assert_eq!(
            second.intent.map(|i| i.name),
            Some("open".to_owned()),
            "the first child returns Running while it acts, so the sequence stays on it"
        );
    }
}
