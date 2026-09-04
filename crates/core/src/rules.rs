// SPDX-License-Identifier: MPL-2.0
use idlewarden_agent::{Condition, NodeSpec, RuleSpec};
use idlewarden_plugin_api::{InputCommand, Value};
use idlewarden_vision::{Anchor, SignalRule};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::recipe::Recipe;

/// One intent as the plugin declares it: when to want it, how to carry it out,
/// and how to tell it worked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentRule {
    pub name: String,
    #[serde(default)]
    pub when: Vec<Condition>,
    #[serde(default)]
    pub commands: Vec<InputCommand>,
    #[serde(default)]
    pub post_condition: Vec<Condition>,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    #[serde(default = "default_floor")]
    pub min_confidence: f64,
}

fn default_floor() -> f64 {
    0.7
}

/// A plugin's `rules.json`: how to see, what to want, and how to act.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PluginRules {
    #[serde(default)]
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub signals: Vec<SignalRule>,
    #[serde(default)]
    pub intents: Vec<IntentRule>,
}

#[derive(Debug, thiserror::Error)]
pub enum RulesError {
    #[error("rules could not be read: {0}")]
    Unreadable(String),
    #[error("rules are not valid: {0}")]
    Invalid(String),
    #[error("intent `{0}` declares no post-condition, so nothing could confirm it")]
    Unverifiable(String),
}

impl PluginRules {
    pub fn parse(json: &str) -> Result<Self, RulesError> {
        let rules: PluginRules =
            serde_json::from_str(json).map_err(|error| RulesError::Invalid(error.to_string()))?;

        for intent in &rules.intents {
            if intent.post_condition.is_empty() {
                return Err(RulesError::Unverifiable(intent.name.clone()));
            }
        }
        Ok(rules)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, RulesError> {
        let json = std::fs::read_to_string(path)
            .map_err(|error| RulesError::Unreadable(error.to_string()))?;
        Self::parse(&json)
    }

    /// The behaviour tree, derived from the declared intents in order: the
    /// first whose conditions hold wins. Declaration order is the priority.
    pub fn tree(&self) -> NodeSpec {
        NodeSpec::Selector {
            children: self
                .intents
                .iter()
                .map(|intent| {
                    NodeSpec::Rule(RuleSpec {
                        name: intent.name.clone(),
                        when: intent.when.clone(),
                        intent: intent.name.clone(),
                        params: intent.params.clone(),
                        min_confidence: intent.min_confidence,
                    })
                })
                .collect(),
        }
    }

    pub fn recipes(&self) -> Vec<Recipe> {
        self.intents
            .iter()
            .map(|intent| Recipe {
                intent: intent.name.clone(),
                commands: intent.commands.clone(),
                post_condition: intent.post_condition.clone(),
                min_confidence: intent.min_confidence,
            })
            .collect()
    }

    /// Template asset names referenced by anchors and rules, so a caller knows
    /// exactly what to decode and nothing more.
    pub fn templates(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .anchors
            .iter()
            .map(|anchor| anchor.template.clone())
            .chain(
                self.signals
                    .iter()
                    .filter_map(|rule| match &rule.extractor {
                        idlewarden_vision::Extractor::TemplateMatch { template, .. } => {
                            Some(template.clone())
                        }
                        _ => None,
                    }),
            )
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: &str = r#"
{
  "anchors": [
    {
      "name": "top_bar_logo",
      "search_area": { "x": 0.0, "y": 0.0, "w": 0.3, "h": 0.12 },
      "template": "assets/logo.png",
      "min_score": 0.85
    }
  ],
  "signals": [
    {
      "id": "ui.reward_ready",
      "extractor": {
        "method": "color_probe",
        "roi": { "x": 0.8, "y": 0.6, "w": 0.02, "h": 0.02 },
        "rgb": [64, 200, 96],
        "tolerance": 24
      }
    },
    {
      "id": "ui.screen_id",
      "extractor": {
        "method": "template_match",
        "roi": { "x": 0.0, "y": 0.0, "w": 1.0, "h": 0.15 },
        "template": "assets/header_main.png",
        "min_score": 0.8
      }
    }
  ],
  "intents": [
    {
      "name": "collect_reward",
      "when": [{ "op": "is_true", "signal": "ui.reward_ready" }],
      "commands": [
        { "op": "click", "at": { "x": 0.5, "y": 0.72 }, "button": "left" },
        { "op": "wait", "ms": 300 }
      ],
      "post_condition": [{ "op": "is_false", "signal": "ui.reward_ready" }],
      "min_confidence": 0.8
    },
    {
      "name": "buy_upgrade",
      "when": [{ "op": "at_least", "signal": "resource.gold", "value": 500.0 }],
      "commands": [{ "op": "click", "at": { "x": 0.81, "y": 0.61 }, "button": "left" }],
      "post_condition": [{ "op": "is_false", "signal": "upgrade.affordable" }]
    }
  ]
}
"#;

    #[test]
    fn a_declared_file_parses_into_every_half_of_the_pipeline() {
        let rules = PluginRules::parse(RULES).expect("valid rules");

        assert_eq!(rules.anchors.len(), 1);
        assert_eq!(rules.signals.len(), 2);
        assert_eq!(rules.recipes().len(), 2);
    }

    #[test]
    fn the_tree_follows_declaration_order() {
        let rules = PluginRules::parse(RULES).expect("valid rules");

        let NodeSpec::Selector { children } = rules.tree() else {
            panic!("the derived tree must be a selector");
        };
        let names: Vec<String> = children
            .iter()
            .map(|child| match child {
                NodeSpec::Rule(rule) => rule.intent.clone(),
                _ => panic!("intents derive rule nodes"),
            })
            .collect();

        assert_eq!(names, vec!["collect_reward", "buy_upgrade"]);
    }

    #[test]
    fn a_recipe_carries_the_commands_and_the_post_condition() {
        let rules = PluginRules::parse(RULES).expect("valid rules");
        let collect = &rules.recipes()[0];

        assert_eq!(collect.intent, "collect_reward");
        assert_eq!(collect.commands.len(), 2);
        assert_eq!(collect.post_condition.len(), 1);
        assert_eq!(collect.min_confidence, 0.8);
    }

    #[test]
    fn an_intent_without_a_post_condition_is_refused_at_load() {
        let json = r#"
{
  "intents": [
    {
      "name": "collect_reward",
      "when": [{ "op": "is_true", "signal": "a" }],
      "commands": [{ "op": "wait", "ms": 10 }]
    }
  ]
}
"#;

        let error = PluginRules::parse(json).expect_err("unverifiable intents are rejected");

        assert!(
            matches!(error, RulesError::Unverifiable(name) if name == "collect_reward"),
            "ADR-0003 calls this a bug, so it fails at load rather than at 3am"
        );
    }

    #[test]
    fn only_the_templates_actually_referenced_are_reported() {
        let rules = PluginRules::parse(RULES).expect("valid rules");

        assert_eq!(
            rules.templates(),
            vec!["assets/header_main.png", "assets/logo.png"],
            "a colour probe references no template and must not invent one"
        );
    }

    #[test]
    fn malformed_json_says_so_rather_than_yielding_empty_rules() {
        let error = PluginRules::parse("{ not json ").expect_err("refused");

        assert!(matches!(error, RulesError::Invalid(_)));
    }

    #[test]
    fn an_empty_file_is_valid_and_produces_an_empty_tree() {
        let rules = PluginRules::parse("{}").expect("an empty plugin is not an error");

        assert!(rules.recipes().is_empty());
        assert!(rules.templates().is_empty());
        let NodeSpec::Selector { children } = rules.tree() else {
            panic!("still a selector");
        };
        assert!(children.is_empty());
    }
}
