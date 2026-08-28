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

use idlewarden_agent::Node;
use idlewarden_capture::{CaptureBackend, CaptureError};
use idlewarden_input::{GuardedInput, InputBackend, InputError, KillSwitch};
use idlewarden_plugin_api::{ActionOutcome, InputCommand, Intent, Observation, PluginId, Signal};
use idlewarden_vision::Perceiver;

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
    capture: Box<dyn CaptureBackend>,
    perceiver: Box<dyn Perceiver>,
    tree: Box<dyn Node>,
    actuator: Box<dyn Actuator>,
    input: GuardedInput<Box<dyn InputBackend>>,
    kill: KillSwitch,
    governor: Governor,
    session: Session,
    /// An intent whose commands have run and whose post-condition is waiting on
    /// the next observation.
    in_flight: Option<Intent>,
    events: Vec<Event>,
}

pub struct Parts {
    pub capture: Box<dyn CaptureBackend>,
    pub perceiver: Box<dyn Perceiver>,
    pub tree: Box<dyn Node>,
    pub actuator: Box<dyn Actuator>,
    pub input: Box<dyn InputBackend>,
    pub kill: KillSwitch,
    pub governor: Governor,
    pub session: Session,
}

impl Runner {
    pub fn new(parts: Parts) -> Self {
        Runner {
            capture: parts.capture,
            perceiver: parts.perceiver,
            tree: parts.tree,
            actuator: parts.actuator,
            input: GuardedInput::new(parts.input, parts.kill.clone()),
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
        let frame = match self.capture.next_frame() {
            Ok(frame) => frame,
            // The window going away is a state change, not a failure: the
            // session drops back to searching and waits for it to return.
            Err(CaptureError::WindowNotFound) => {
                self.game_lost();
                return None;
            }
            Err(error) => {
                self.halt(error.to_string());
                return None;
            }
        };

        match self.perceiver.perceive(&frame) {
            Ok(extracted) => Some(Observation {
                frame_id: frame.id,
                captured_at_ms: now_ms,
                signals: extracted
                    .into_iter()
                    .map(|e| Signal {
                        id: e.id,
                        value: e.value,
                        confidence: e.confidence,
                    })
                    .collect(),
            }),
            // Perception failing structurally costs this tick, not the session.
            // A degraded read is the Governor's business through the confidence
            // floor, not an error.
            Err(error) => {
                self.emit(Event::Error {
                    message: error.to_string(),
                });
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

        for command in &commands {
            match self.input.execute(command) {
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
