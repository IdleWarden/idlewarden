use super::{exposed_tools, is_resume_failure, parse_stream_line, Reply, StreamLine};

const LOST_NOTE: &str = " La conversation repart de zéro.";

pub fn lost_note(resumed: bool, detail: &str) -> Option<&'static str> {
    (resumed && is_resume_failure(detail)).then_some(LOST_NOTE)
}

#[derive(Debug, Default, PartialEq)]
pub struct Effects {
    pub partial: Option<String>,
    pub kill: bool,
    pub remember: Option<String>,
    pub forget: bool,
}

pub struct Turn {
    resumed: bool,
    blocked: bool,
    pending_session: Option<String>,
    shown: String,
    verdict: Option<Reply>,
}

impl Turn {
    pub fn new(resumed: bool) -> Self {
        Self {
            resumed,
            blocked: false,
            pending_session: None,
            shown: String::new(),
            verdict: None,
        }
    }

    pub fn into_verdict(self) -> Option<Reply> {
        self.verdict
    }

    pub fn feed(&mut self, line: &str) -> Effects {
        let mut effects = Effects::default();
        if self.blocked {
            return effects;
        }

        let tools = exposed_tools(line);
        if !tools.is_empty() {
            self.blocked = true;
            self.verdict = Some(Reply::Failed {
                message: format!(
                    "Claude Code expose des outils ({}) malgré --tools vide, le Gardien s'arrête.",
                    tools.join(", ")
                ),
            });
            effects.kill = true;
            return effects;
        }

        match parse_stream_line(line) {
            Some(StreamLine::Session(id)) => self.pending_session = Some(id),
            Some(StreamLine::Text(text)) => {
                if !self.shown.is_empty() {
                    self.shown.push_str("\n\n");
                }
                self.shown.push_str(&text);
                effects.partial = Some(self.shown.clone());
            }
            Some(StreamLine::Result { ok: true, text }) => {
                effects.remember = self.pending_session.take();
                let text = if self.shown.trim().is_empty() {
                    text
                } else {
                    self.shown.clone()
                };
                self.verdict = Some(Reply::Done { text });
            }
            Some(StreamLine::Result { ok: false, text }) => {
                let note = lost_note(self.resumed, &text);
                effects.forget = note.is_some();
                let message = if text.trim().is_empty() {
                    "Claude a répondu par une erreur sans détail.".to_owned()
                } else {
                    text
                };
                self.verdict = Some(Reply::Failed {
                    message: format!("{message}{}", note.unwrap_or_default()),
                });
            }
            None => {}
        }
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INIT_WITH_TOOLS: &str =
        r#"{"type":"system","subtype":"init","session_id":"s","tools":["Bash"]}"#;
    const INIT: &str = r#"{"type":"system","subtype":"init","session_id":"s","tools":[]}"#;
    const OK: &str = r#"{"type":"result","is_error":false,"result":"fin"}"#;

    #[test]
    fn exposed_tools_kill_the_run_and_everything_after_is_ignored() {
        let mut turn = Turn::new(false);
        let effects = turn.feed(INIT_WITH_TOOLS);
        assert!(effects.kill);
        assert_eq!(turn.feed(OK), Effects::default());
        assert!(matches!(
            turn.into_verdict(),
            Some(Reply::Failed { message }) if message.contains("Bash")
        ));
    }

    #[test]
    fn the_session_is_remembered_only_on_a_successful_result() {
        let mut turn = Turn::new(false);
        assert_eq!(turn.feed(INIT), Effects::default());
        let effects = turn.feed(OK);
        assert_eq!(effects.remember.as_deref(), Some("s"));
        assert_eq!(
            turn.into_verdict(),
            Some(Reply::Done {
                text: "fin".to_owned()
            })
        );

        let mut failed = Turn::new(false);
        failed.feed(INIT);
        let effects = failed.feed(r#"{"type":"result","is_error":true,"result":"overloaded"}"#);
        assert_eq!(effects.remember, None);
        assert!(!effects.forget);
    }

    #[test]
    fn partial_text_accumulates_and_wins_over_the_result_field() {
        let mut turn = Turn::new(false);
        let first = turn
            .feed(r#"{"type":"assistant","message":{"content":[{"type":"text","text":"un"}]}}"#);
        assert_eq!(first.partial.as_deref(), Some("un"));
        let second = turn
            .feed(r#"{"type":"assistant","message":{"content":[{"type":"text","text":"deux"}]}}"#);
        assert_eq!(second.partial.as_deref(), Some("un\n\ndeux"));
        turn.feed(OK);
        assert_eq!(
            turn.into_verdict(),
            Some(Reply::Done {
                text: "un\n\ndeux".to_owned()
            })
        );
    }

    #[test]
    fn only_a_failed_resume_forgets_the_session_and_says_so() {
        let lost = r#"{"type":"result","is_error":true,"result":"No conversation found with session ID: x"}"#;
        let mut resumed = Turn::new(true);
        let effects = resumed.feed(lost);
        assert!(effects.forget);
        assert!(matches!(
            resumed.into_verdict(),
            Some(Reply::Failed { message }) if message.ends_with(LOST_NOTE)
        ));

        let mut fresh = Turn::new(false);
        assert!(!fresh.feed(lost).forget);

        let mut transient = Turn::new(true);
        let effects =
            transient.feed(r#"{"type":"result","is_error":true,"result":"529 overloaded"}"#);
        assert!(!effects.forget);
    }

    #[test]
    fn an_empty_error_gets_a_generic_message() {
        let mut turn = Turn::new(false);
        turn.feed(r#"{"type":"result","is_error":true,"result":""}"#);
        assert!(matches!(
            turn.into_verdict(),
            Some(Reply::Failed { message }) if message.contains("sans détail")
        ));
    }
}
