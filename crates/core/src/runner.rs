// SPDX-License-Identifier: MPL-2.0
//! The loop that actually plays: capture, perceive, decide, govern, act.
//!
//! It exists as a **tick** rather than a thread so that a whole session can be
//! driven deterministically in a test, with no timing and no sleeping. Owning a
//! thread is a separate, thin concern; see [`spawn`].
//!
//! Nothing here decides anything about a game. It sequences the pieces and
//! enforces the two rules the rest of the architecture rests on: every intent
//! goes through the Governor (ADR-0009), and nothing is reported as having
//! succeeded until it has been checked (ADR-0003).

mod link;

use idlewarden_agent::Node;
use idlewarden_bridge::BridgeError;
use idlewarden_input::{InputError, KillSwitch};
use idlewarden_plugin_api::{ActionOutcome, InputCommand, Intent, Observation, PluginId};

pub use link::Link;
use link::{is_lost, Miss, Wired};

use crate::event::Event;
use crate::governor::{Governor, Verdict};
use crate::session::{Session, SessionState};

/// Turns an intent into the commands that carry it out, and afterwards decides
/// whether it worked.
///
/// The plugin owns both halves (ADR-0003): the translation is game knowledge,
/// and so is the post-condition. An implementation that cannot check its own
/// post-condition should say `Failed`, never `Succeeded`.
pub trait Actuator: Send {
    fn plan(&mut self, intent: &Intent) -> Vec<InputCommand>;

    /// Called with the first observation taken *after* the commands ran.
    fn verify(&mut self, intent: &Intent, after: &Observation) -> ActionOutcome;
}

pub struct Runner {
    link: Wired,
    tree: Box<dyn Node>,
    actuator: Box<dyn Actuator>,
    kill: KillSwitch,
    governor: Governor,
    session: Session,
    /// An intent whose commands have run and whose post-condition is waiting on
    /// the next observation.
    in_flight: Option<Intent>,
    events: Vec<Event>,
}

pub struct Parts {
    pub link: Link,
    pub tree: Box<dyn Node>,
    pub actuator: Box<dyn Actuator>,
    pub kill: KillSwitch,
    pub governor: Governor,
    pub session: Session,
}

impl Runner {
    pub fn new(parts: Parts) -> Self {
        Runner {
            link: Wired::new(parts.link, &parts.kill),
            tree: parts.tree,
            actuator: parts.actuator,
            kill: parts.kill,
            governor: parts.governor,
            session: parts.session,
            in_flight: None,
            events: Vec::new(),
        }
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Everything the runner has published since the last drain. The caller
    /// forwards these; the runner has no opinion about where they go.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn apply(&mut self, command: &crate::event::Command) {
        if let Err(refusal) = self.session.apply(command) {
            self.emit(Event::Error {
                message: refusal.to_string(),
            });
            return;
        }
        match self.session.state {
            SessionState::Paused => self.emit(Event::AgentPaused {
                reason: self
                    .session
                    .last_reason
                    .clone()
                    .unwrap_or_else(|| "paused".to_owned()),
            }),
            SessionState::Running => self.emit(Event::AgentResumed),
            _ => {}
        }
    }

    pub fn game_detected(&mut self, plugin: PluginId, window_title: String) {
        self.session.game_detected(plugin.clone());
        self.emit(Event::GameDetected {
            plugin,
            window_title,
        });
    }

    pub fn game_lost(&mut self) {
        self.session.game_lost();
        self.tree.reset();
        self.in_flight = None;
        self.emit(Event::GameLost);
    }

    /// One pass. `now_ms` is the session clock, not wall time, so a test can
    /// drive an hour of session in a handful of calls.
    pub fn tick(&mut self, now_ms: u64) {
        if self.session.state == SessionState::Halted {
            return;
        }

        let Some(observation) = self.observe(now_ms) else {
            return;
        };
        self.emit(Event::Observed {
            observation: observation.clone(),
        });

        self.settle_in_flight(&observation);

        if !self.session.can_act() {
            return;
        }

        let Some(intent) = self.tree.tick(&observation).intent else {
            return;
        };
        self.emit(Event::IntentProposed {
            intent: intent.clone(),
        });

        match self.governor.review(&intent, &observation, now_ms) {
            Verdict::Reject { reason } => self.emit(Event::IntentRejected { intent, reason }),
            Verdict::Halt { reason } => self.halt(reason),
            Verdict::Allow => self.act(intent),
        }
    }

    fn observe(&mut self, now_ms: u64) -> Option<Observation> {
        match self.link.sense(now_ms) {
            Ok(observation) => Some(observation),
            Err(Miss::WindowGone) => {
                self.game_lost();
                None
            }
            Err(Miss::Broken(reason)) => {
                self.halt(reason);
                None
            }
            Err(Miss::Unreadable(message)) => {
                self.emit(Event::Error { message });
                None
            }
            Err(Miss::BridgeLost(reason)) => {
                self.pause_for(reason);
                None
            }
        }
    }

    fn settle_in_flight(&mut self, observation: &Observation) {
        let Some(intent) = self.in_flight.take() else {
            return;
        };
        let outcome = self.actuator.verify(&intent, observation);
        self.emit(Event::ActionFinished { intent, outcome });
    }

    fn act(&mut self, intent: Intent) {
        if matches!(self.link, Wired::Bridged(_)) {
            self.act_through_bridge(intent);
        } else {
            self.act_through_input(intent);
        }
    }

    fn act_through_bridge(&mut self, intent: Intent) {
        if self.kill.is_engaged() {
            self.emit(Event::KillSwitch);
            self.halt("the kill switch was engaged".to_owned());
            self.emit(Event::ActionFinished {
                intent,
                outcome: ActionOutcome::Aborted,
            });
            return;
        }

        self.emit(Event::ActionStarted {
            intent: intent.clone(),
        });

        if self.session.dry_run {
            tracing::info!(
                intent = intent.name.as_str(),
                "dry-run: not sending to the mod"
            );
            self.session.actions_taken += 1;
            self.in_flight = Some(intent);
            return;
        }

        let Wired::Bridged(bridge) = &mut self.link else {
            return;
        };
        match bridge.act(&intent) {
            Ok(ActionOutcome::Succeeded) => {
                self.session.actions_taken += 1;
                self.in_flight = Some(intent);
            }
            Ok(outcome) => self.emit(Event::ActionFinished { intent, outcome }),
            Err(error) if is_lost(&error) => {
                self.emit(Event::ActionFinished {
                    intent,
                    outcome: ActionOutcome::Aborted,
                });
                self.pause_for(error.to_string());
            }
            Err(BridgeError::Refused(reason)) => self.emit(Event::ActionFinished {
                intent,
                outcome: ActionOutcome::Rejected { reason },
            }),
            Err(error) => self.emit(Event::ActionFinished {
                intent,
                outcome: ActionOutcome::Rejected {
                    reason: error.to_string(),
                },
            }),
        }
    }

    fn act_through_input(&mut self, intent: Intent) {
        let commands = self.actuator.plan(&intent);
        if commands.is_empty() {
            self.emit(Event::ActionFinished {
                intent,
                outcome: ActionOutcome::Rejected {
                    reason: "the plugin produced no commands for this intent".to_owned(),
                },
            });
            return;
        }

        self.emit(Event::ActionStarted {
            intent: intent.clone(),
        });

        let Wired::Perceived { input, .. } = &mut self.link else {
            return;
        };
        for command in &commands {
            match input.execute(command) {
                Ok(()) => {}
                Err(InputError::KillSwitchEngaged) => {
                    self.emit(Event::KillSwitch);
                    self.halt("the kill switch was engaged".to_owned());
                    self.emit(Event::ActionFinished {
                        intent,
                        outcome: ActionOutcome::Aborted,
                    });
                    return;
                }
                Err(error) => {
                    self.emit(Event::ActionFinished {
                        intent,
                        outcome: ActionOutcome::Rejected {
                            reason: error.to_string(),
                        },
                    });
                    return;
                }
            }
        }

        self.session.actions_taken += 1;
        // Nothing is called succeeded here. The post-condition is checked
        // against the next observation, which is the only thing that can say
        // whether the world actually changed.
        self.in_flight = Some(intent);
    }

    fn pause_for(&mut self, reason: String) {
        let already = self.session.state == SessionState::Paused
            && self.session.last_reason.as_deref() == Some(reason.as_str());
        if already {
            return;
        }
        self.session.pause(reason.clone());
        self.tree.reset();
        self.in_flight = None;
        self.emit(Event::AgentPaused { reason });
    }

    fn halt(&mut self, reason: String) {
        self.session.halt(reason.clone());
        self.tree.reset();
        self.in_flight = None;
        self.emit(Event::AgentPaused { reason });
    }

    pub fn kill_switch(&self) -> &KillSwitch {
        &self.kill
    }

    fn emit(&mut self, event: Event) {
        self.events.push(event);
    }
}

#[cfg(test)]
mod tests;
