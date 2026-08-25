// SPDX-License-Identifier: MPL-2.0

use std::sync::Mutex;

use idlewarden_core::{Command, Refusal, Session};
use serde::Serialize;
use tauri::State;

#[derive(Default)]
pub struct SessionHandle(Mutex<Session>);

#[derive(Debug, Serialize)]
pub struct Refused {
    refusal: Refusal,
    message: String,
}

impl From<Refusal> for Refused {
    fn from(refusal: Refusal) -> Self {
        Refused {
            message: refusal.to_string(),
            refusal,
        }
    }
}

#[tauri::command]
pub fn session_state(handle: State<'_, SessionHandle>) -> Session {
    handle.0.lock().expect("session lock").clone()
}

#[tauri::command]
pub fn dispatch(handle: State<'_, SessionHandle>, command: Command) -> Result<Session, Refused> {
    let mut session = handle.0.lock().expect("session lock");
    session.apply(&command)?;
    Ok(session.clone())
}
