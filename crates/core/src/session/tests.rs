// SPDX-License-Identifier: MPL-2.0

use super::*;

fn plugin() -> PluginId {
    PluginId("dev.example.cookie-clicker".into())
}

fn ready() -> Session {
    let mut session = Session::default();
    session.game_detected(plugin());
    session
}

fn running() -> Session {
    let mut session = ready();
    session
        .apply(&Command::Start {
            plugin: plugin(),
            profile: "default".into(),
        })
        .unwrap();
    session
}

#[test]
fn a_fresh_session_is_searching_and_dry_run() {
    let session = Session::default();
    assert_eq!(session.state, SessionState::Searching);
    assert!(session.dry_run);
    assert!(!session.can_act());
}

#[test]
fn starting_without_a_detected_game_is_refused() {
    let mut session = Session::default();
    let command = Command::Start {
        plugin: plugin(),
        profile: "default".into(),
    };

    assert_eq!(session.apply(&command), Err(Refusal::NoGameReady));
    assert_eq!(session.state, SessionState::Searching);
}

#[test]
fn detection_makes_a_session_startable_and_records_the_profile() {
    let session = running();
    assert_eq!(session.state, SessionState::Running);
    assert!(session.can_act());
    assert_eq!(session.plugin, Some(plugin()));
    assert_eq!(session.profile.as_deref(), Some("default"));
}

#[test]
fn pause_and_resume_round_trip() {
    let mut session = running();

    session.apply(&Command::Pause).unwrap();
    assert_eq!(session.state, SessionState::Paused);
    assert!(!session.can_act());

    session.apply(&Command::Resume).unwrap();
    assert_eq!(session.state, SessionState::Running);
    assert_eq!(session.last_reason, None);
}

#[test]
fn resuming_a_session_that_is_not_paused_is_refused() {
    let mut session = running();
    assert_eq!(session.apply(&Command::Resume), Err(Refusal::NotPaused));
}

#[test]
fn stopping_returns_to_ready_so_the_game_stays_detected() {
    let mut session = running();

    session.apply(&Command::Stop).unwrap();
    assert_eq!(session.state, SessionState::Ready);
    assert_eq!(session.plugin, Some(plugin()));
}

#[test]
fn dry_run_cannot_be_disabled_while_the_agent_is_acting() {
    let mut session = running();

    assert_eq!(
        session.apply(&Command::SetDryRun { enabled: false }),
        Err(Refusal::RunningDryRunChange)
    );
    assert!(session.dry_run);

    session.apply(&Command::Pause).unwrap();
    session
        .apply(&Command::SetDryRun { enabled: false })
        .unwrap();
    assert!(!session.dry_run);
}

#[test]
fn a_halted_session_refuses_every_command() {
    let mut session = running();
    session.halt("kill switch");

    for command in [
        Command::Start {
            plugin: plugin(),
            profile: "default".into(),
        },
        Command::Stop,
        Command::Pause,
        Command::Resume,
        Command::SetDryRun { enabled: true },
    ] {
        assert_eq!(session.apply(&command), Err(Refusal::Halted), "{command:?}");
    }
    assert_eq!(session.state, SessionState::Halted);
}

#[test]
fn losing_the_game_window_stops_the_agent_acting() {
    let mut session = running();

    session.game_lost();
    assert_eq!(session.state, SessionState::Searching);
    assert!(!session.can_act());
    assert_eq!(session.plugin, None);
}

#[test]
fn detection_never_revives_a_halted_session() {
    let mut session = Session::default();
    session.halt("fatal");

    session.game_detected(plugin());
    assert_eq!(session.state, SessionState::Halted);
}
