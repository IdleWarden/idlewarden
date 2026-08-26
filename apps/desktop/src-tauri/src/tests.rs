// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use crate::updates::{
    check_url, install_id, interpret, load_settings, save_settings, Channel, CheckResult, Settings,
    UpdateError,
};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("idlewarden-desktop-tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_url_matches_what_the_endpoint_parses() {
    assert_eq!(
        check_url(
            "https://idlewarden.com/api",
            "windows-x86_64",
            "26.8.1",
            Channel::Stable
        ),
        "https://idlewarden.com/api/v1/update/windows-x86_64/26.8.1?channel=stable"
    );
}

#[test]
fn a_trailing_slash_on_the_endpoint_does_not_double_up() {
    assert_eq!(
        check_url(
            "https://idlewarden.com/api/",
            "linux-x86_64",
            "26.8.1",
            Channel::Beta
        ),
        "https://idlewarden.com/api/v1/update/linux-x86_64/26.8.1?channel=beta"
    );
}

#[test]
fn a_204_means_up_to_date_rather_than_a_failure() {
    assert_eq!(interpret(204, "").unwrap(), CheckResult::UpToDate);
}

#[test]
fn a_200_carries_the_offer() {
    let body = r#"{"version":"26.9.0","pub_date":"2026-09-01T10:00:00Z",
        "url":"https://example.test/a.zip","signature":"sig","notes":"Fixes things."}"#;

    let CheckResult::Available { offer } = interpret(200, body).unwrap() else {
        panic!("expected an offer");
    };
    assert_eq!(offer.version, "26.9.0");
    assert_eq!(offer.notes.as_deref(), Some("Fixes things."));
}

#[test]
fn an_offer_without_notes_is_still_an_offer() {
    let body = r#"{"version":"26.9.0","pub_date":"x","url":"u","signature":"s"}"#;

    let CheckResult::Available { offer } = interpret(200, body).unwrap() else {
        panic!("expected an offer");
    };
    assert!(offer.notes.is_none());
}

#[test]
fn a_garbled_offer_is_an_error_rather_than_a_silent_up_to_date() {
    assert!(matches!(
        interpret(200, "not json"),
        Err(UpdateError::Malformed(_))
    ));
}

#[test]
fn any_other_status_is_reported_with_its_code() {
    assert!(matches!(interpret(500, ""), Err(UpdateError::Status(500))));
    assert!(matches!(
        interpret(400, "{}"),
        Err(UpdateError::Status(400))
    ));
}

#[test]
fn the_install_id_is_generated_once_and_then_reused() {
    let dir = scratch("install-id");

    let first = install_id(&dir).unwrap();
    assert!(!first.is_empty());

    for _ in 0..5 {
        assert_eq!(install_id(&dir).unwrap(), first);
    }
}

#[test]
fn two_installs_get_different_ids_so_a_rollout_can_split_them() {
    let a = install_id(&scratch("install-a")).unwrap();
    let b = install_id(&scratch("install-b")).unwrap();

    assert_ne!(a, b);
}

#[test]
fn an_empty_install_id_file_is_replaced_rather_than_trusted() {
    let dir = scratch("install-empty");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("install-id"), "   \n").unwrap();

    let id = install_id(&dir).unwrap();

    assert!(!id.trim().is_empty());
    assert_eq!(install_id(&dir).unwrap(), id);
}

#[test]
fn the_channel_defaults_to_stable_and_survives_a_restart() {
    let dir = scratch("settings");

    assert_eq!(load_settings(&dir).channel, Channel::Stable);

    save_settings(
        &dir,
        &Settings {
            channel: Channel::Beta,
        },
    )
    .unwrap();
    assert_eq!(load_settings(&dir).channel, Channel::Beta);

    save_settings(
        &dir,
        &Settings {
            channel: Channel::Stable,
        },
    )
    .unwrap();
    assert_eq!(load_settings(&dir).channel, Channel::Stable);
}

#[test]
fn a_corrupt_settings_file_falls_back_to_stable_instead_of_failing() {
    let dir = scratch("settings-corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("updates.json"), "{ not json").unwrap();

    assert_eq!(load_settings(&dir).channel, Channel::Stable);
}
