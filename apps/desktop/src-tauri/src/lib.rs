// SPDX-License-Identifier: MPL-2.0
//! The desktop shell (ADR-0004).
//!
//! Everything in this crate is an adapter: it turns UI gestures into
//! `idlewarden_core::Command` and hands `idlewarden_core` types back to the
//! web view. No decision about a session is taken here.

mod session;
mod updates;

#[cfg(test)]
mod tests;

pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.manage(updates::Updates::new(app.handle()));
            Ok(())
        })
        .manage(session::SessionHandle::default())
        .invoke_handler(tauri::generate_handler![
            session::session_state,
            session::dispatch,
            updates::update_settings,
            updates::set_update_channel,
            updates::check_for_update,
            updates::install_update
        ])
        .run(tauri::generate_context!())
        .expect("the tauri runtime failed to start");
}
