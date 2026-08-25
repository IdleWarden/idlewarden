// SPDX-License-Identifier: MPL-2.0
//! The desktop shell (ADR-0004).
//!
//! Everything in this crate is an adapter: it turns UI gestures into
//! `idlewarden_core::Command` and hands `idlewarden_core` types back to the
//! web view. No decision about a session is taken here.

mod session;

pub fn run() {
    tauri::Builder::default()
        .manage(session::SessionHandle::default())
        .invoke_handler(tauri::generate_handler![
            session::session_state,
            session::dispatch
        ])
        .run(tauri::generate_context!())
        .expect("the tauri runtime failed to start");
}
