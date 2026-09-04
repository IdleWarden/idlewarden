// SPDX-License-Identifier: MPL-2.0
//! The agent (ADR-0008).
//!
//! There is exactly **one** execution model, a ticked behaviour tree, and the
//! "kinds of agent" (rules, state machine, model, LLM) are node types inside
//! it. Building five parallel agent implementations would mean maintaining five
//! loops; this way a new decision strategy is a new [`Decider`].
//!
//! The LLM is never in the hot loop. It proposes a *goal*, rarely; the tree
//! executes it.

mod rule;
mod spec;

pub use rule::{Condition, RuleDecider, RuleSpec};
pub use spec::NodeSpec;

use idlewarden_plugin_api::{Intent, Observation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Failure,
    /// Still working; the tree will be ticked again.
    Running,
}

/// What a node produced this tick.
#[derive(Debug, Clone)]
pub struct Tick {
    pub status: Status,
    pub intent: Option<Intent>,
}

impl Tick {
    pub fn success() -> Self {
        Tick {
            status: Status::Success,
            intent: None,
        }
    }
    pub fn failure() -> Self {
        Tick {
            status: Status::Failure,
            intent: None,
        }
    }
    pub fn running() -> Self {
        Tick {
            status: Status::Running,
            intent: None,
        }
    }
    pub fn act(intent: Intent) -> Self {
        Tick {
            status: Status::Running,
            intent: Some(intent),
        }
    }
}

/// Anything that can choose an intent from an observation: a rule table, a
/// state machine, an ONNX model, or a cached LLM plan.
pub trait Decider: Send {
    fn decide(&mut self, obs: &Observation) -> Option<Intent>;
    fn name(&self) -> &str;
}

pub trait Node: Send {
    fn tick(&mut self, obs: &Observation) -> Tick;
    fn reset(&mut self) {}
}

/// Runs children in order until one does not fail.
pub struct Selector {
    pub children: Vec<Box<dyn Node>>,
}

impl Node for Selector {
    fn tick(&mut self, obs: &Observation) -> Tick {
        for child in self.children.iter_mut() {
            let t = child.tick(obs);
            if t.status != Status::Failure {
                return t;
            }
        }
        Tick::failure()
    }

    fn reset(&mut self) {
        for c in self.children.iter_mut() {
            c.reset();
        }
    }
}

/// Runs children in order until one does not succeed.
pub struct Sequence {
    pub children: Vec<Box<dyn Node>>,
    cursor: usize,
}

impl Sequence {
    pub fn new(children: Vec<Box<dyn Node>>) -> Self {
        Sequence {
            children,
            cursor: 0,
        }
    }
}

impl Node for Sequence {
    fn tick(&mut self, obs: &Observation) -> Tick {
        while self.cursor < self.children.len() {
            let t = self.children[self.cursor].tick(obs);
            match t.status {
                Status::Success => self.cursor += 1,
                Status::Running | Status::Failure => return t,
            }
        }
        self.cursor = 0;
        Tick::success()
    }

    fn reset(&mut self) {
        self.cursor = 0;
        for c in self.children.iter_mut() {
            c.reset();
        }
    }
}

/// Leaf node wrapping any [`Decider`].
pub struct DeciderNode {
    pub decider: Box<dyn Decider>,
}

impl Node for DeciderNode {
    fn tick(&mut self, obs: &Observation) -> Tick {
        match self.decider.decide(obs) {
            Some(intent) => Tick::act(intent),
            None => Tick::failure(),
        }
    }
}
