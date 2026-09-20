// SPDX-License-Identifier: MPL-2.0
use std::collections::{BTreeMap, HashSet};

use idlewarden_agent::Condition;
use idlewarden_plugin_api::{InputCommand, MouseButton, Point};
use serde::{Deserialize, Serialize};

use super::{well_formed, AuthoringError, Region, RegionKind};
use crate::rules::IntentRule;

const SETTLE_MS: u64 = 300;
const MIN_CONFIDENCE: f64 = 0.8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentDraft {
    pub name: String,
    pub when: Vec<Condition>,
    pub click: Point,
    pub post_condition: Vec<Condition>,
}

pub(super) fn validate(intents: &[IntentDraft], regions: &[Region]) -> Result<(), AuthoringError> {
    let signals: HashSet<&str> = regions
        .iter()
        .filter(|region| region.kind != RegionKind::Anchor)
        .map(|region| region.name.as_str())
        .collect();

    let mut seen = HashSet::new();
    for intent in intents {
        let name = || intent.name.clone();

        if !well_formed(&intent.name) || !seen.insert(intent.name.as_str()) {
            return Err(AuthoringError::BadIntentName(name()));
        }
        if intent.when.is_empty() {
            return Err(AuthoringError::NoTrigger(name()));
        }
        if intent.post_condition.is_empty() {
            return Err(AuthoringError::NoPostCondition(name()));
        }

        let decided = intent.when.iter().map(|condition| (condition, true));
        let verified = intent
            .post_condition
            .iter()
            .map(|condition| (condition, false));

        for (condition, decides) in decided.chain(verified) {
            let signal = condition.signal().to_owned();
            let allowed = match condition {
                Condition::IsTrue { .. } | Condition::IsFalse { .. } => true,
                Condition::Increased { .. }
                | Condition::Decreased { .. }
                | Condition::Changed { .. } => !decides,
                _ => false,
            };
            if !allowed {
                return Err(if condition.is_standalone() {
                    AuthoringError::UnsupportedCondition(name(), signal)
                } else {
                    AuthoringError::DeltaWhenDeciding(name(), signal)
                });
            }
            if !signals.contains(signal.as_str()) {
                return Err(AuthoringError::UnknownSignal(name(), signal));
            }
        }

        if intent
            .post_condition
            .iter()
            .all(|condition| intent.when.contains(condition))
        {
            return Err(AuthoringError::UnprovablePostCondition(name()));
        }

        let inside = |value: f64| (0.0..=1.0).contains(&value);
        if !inside(intent.click.x) || !inside(intent.click.y) {
            return Err(AuthoringError::ClickOutside(name()));
        }
    }
    Ok(())
}

pub(super) fn rules(intents: &[IntentDraft]) -> Vec<IntentRule> {
    intents
        .iter()
        .map(|intent| IntentRule {
            name: intent.name.clone(),
            when: intent.when.clone(),
            commands: vec![
                InputCommand::Click {
                    at: intent.click,
                    button: MouseButton::Left,
                },
                InputCommand::Wait { ms: SETTLE_MS },
            ],
            post_condition: intent.post_condition.clone(),
            params: BTreeMap::new(),
            min_confidence: MIN_CONFIDENCE,
        })
        .collect()
}
