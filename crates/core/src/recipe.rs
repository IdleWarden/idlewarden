// SPDX-License-Identifier: MPL-2.0
use idlewarden_agent::Condition;
use idlewarden_plugin_api::{ActionOutcome, InputCommand, Intent, Observation};
use serde::{Deserialize, Serialize};

use crate::runner::Actuator;

/// How a plugin says one intent is carried out, and how to tell it worked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub intent: String,
    pub commands: Vec<InputCommand>,
    /// Every condition must hold on the observation taken after the commands
    /// ran. An empty post-condition is a bug the plugin author has to fix
    /// (ADR-0003), so it verifies as failed rather than succeeded.
    #[serde(default)]
    pub post_condition: Vec<Condition>,
    #[serde(default = "default_floor")]
    pub min_confidence: f64,
}

fn default_floor() -> f64 {
    0.7
}

/// Turns intents into input commands from the plugin's declared recipes.
pub struct RecipeActuator {
    recipes: Vec<Recipe>,
}

impl RecipeActuator {
    pub fn new(recipes: Vec<Recipe>) -> Self {
        RecipeActuator { recipes }
    }

    fn recipe(&self, intent: &Intent) -> Option<&Recipe> {
        self.recipes
            .iter()
            .find(|recipe| recipe.intent == intent.name)
    }
}

impl Actuator for RecipeActuator {
    fn plan(&mut self, intent: &Intent) -> Vec<InputCommand> {
        self.recipe(intent)
            .map(|recipe| recipe.commands.clone())
            .unwrap_or_default()
    }

    fn verify(&mut self, intent: &Intent, after: &Observation) -> ActionOutcome {
        let Some(recipe) = self.recipe(intent) else {
            return ActionOutcome::Failed {
                reason: format!("no recipe declares `{}`", intent.name),
            };
        };

        if recipe.post_condition.is_empty() {
            return ActionOutcome::Failed {
                reason: format!("`{}` declares no post-condition", intent.name),
            };
        }

        for condition in &recipe.post_condition {
            let Some(signal) = after.get(condition.signal()) else {
                return ActionOutcome::Failed {
                    reason: format!("`{}` was not observed after acting", condition.signal()),
                };
            };
            if signal.confidence.get() < recipe.min_confidence {
                return ActionOutcome::Failed {
                    reason: format!("`{}` was read too weakly to confirm", condition.signal()),
                };
            }
            if !condition.met(after) {
                return ActionOutcome::Failed {
                    reason: format!("`{}` did not change as expected", condition.signal()),
                };
            }
        }

        ActionOutcome::Succeeded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlewarden_plugin_api::{Confidence, MouseButton, Point, Signal, SignalId, Value};

    fn observation(signals: Vec<(&str, Value, f64)>) -> Observation {
        Observation {
            frame_id: 1,
            captured_at_ms: 250,
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

    fn collect() -> Recipe {
        Recipe {
            intent: "collect_reward".to_owned(),
            commands: vec![
                InputCommand::Click {
                    at: Point { x: 0.5, y: 0.72 },
                    button: MouseButton::Left,
                },
                InputCommand::Wait { ms: 300 },
            ],
            post_condition: vec![Condition::IsFalse {
                signal: "ui.reward_ready".to_owned(),
            }],
            min_confidence: 0.7,
        }
    }

    #[test]
    fn planning_returns_the_declared_commands() {
        let mut actuator = RecipeActuator::new(vec![collect()]);

        let commands = actuator.plan(&Intent::new("collect_reward"));

        assert_eq!(commands.len(), 2);
        assert!(matches!(commands[0], InputCommand::Click { .. }));
    }

    #[test]
    fn an_unknown_intent_plans_nothing_and_never_reports_success() {
        let mut actuator = RecipeActuator::new(vec![collect()]);
        let unknown = Intent::new("open_the_pod_bay_doors");

        assert!(actuator.plan(&unknown).is_empty());
        assert!(matches!(
            actuator.verify(&unknown, &observation(vec![])),
            ActionOutcome::Failed { .. }
        ));
    }

    #[test]
    fn a_met_post_condition_verifies_as_succeeded() {
        let mut actuator = RecipeActuator::new(vec![collect()]);

        let outcome = actuator.verify(
            &Intent::new("collect_reward"),
            &observation(vec![("ui.reward_ready", Value::Bool(false), 0.95)]),
        );

        assert_eq!(outcome, ActionOutcome::Succeeded);
    }

    #[test]
    fn an_unmet_post_condition_names_the_signal_that_did_not_move() {
        let mut actuator = RecipeActuator::new(vec![collect()]);

        let outcome = actuator.verify(
            &Intent::new("collect_reward"),
            &observation(vec![("ui.reward_ready", Value::Bool(true), 0.95)]),
        );

        assert!(
            matches!(outcome, ActionOutcome::Failed { reason } if reason.contains("ui.reward_ready"))
        );
    }

    #[test]
    fn a_recipe_without_a_post_condition_can_never_succeed() {
        let mut blind = collect();
        blind.post_condition.clear();
        let mut actuator = RecipeActuator::new(vec![blind]);

        let outcome = actuator.verify(
            &Intent::new("collect_reward"),
            &observation(vec![("ui.reward_ready", Value::Bool(false), 0.95)]),
        );

        assert!(
            matches!(outcome, ActionOutcome::Failed { reason } if reason.contains("post-condition")),
            "ADR-0003: an intent that cannot be checked must not be reported as done"
        );
    }

    #[test]
    fn a_post_condition_read_too_weakly_is_not_a_confirmation() {
        let mut actuator = RecipeActuator::new(vec![collect()]);

        let outcome = actuator.verify(
            &Intent::new("collect_reward"),
            &observation(vec![("ui.reward_ready", Value::Bool(false), 0.2)]),
        );

        assert!(
            matches!(outcome, ActionOutcome::Failed { reason } if reason.contains("too weakly")),
            "a barely visible screen must not confirm an action"
        );
    }

    #[test]
    fn a_signal_that_vanished_after_acting_is_a_failure_not_a_success() {
        let mut actuator = RecipeActuator::new(vec![collect()]);

        let outcome = actuator.verify(
            &Intent::new("collect_reward"),
            &observation(vec![("something.else", Value::Bool(true), 0.95)]),
        );

        assert!(
            matches!(outcome, ActionOutcome::Failed { reason } if reason.contains("not observed"))
        );
    }

    #[test]
    fn recipes_survive_a_round_trip_through_json() {
        let written = serde_json::to_string(&collect()).expect("serialises");
        let read: Recipe = serde_json::from_str(&written).expect("parses back");

        assert_eq!(read, collect());
    }
}
