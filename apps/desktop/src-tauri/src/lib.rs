// SPDX-License-Identifier: MPL-2.0
//! The desktop shell (ADR-0004).
//!
//! Everything in this crate is an adapter: it turns UI gestures into
//! `idlewarden_core::Command` and hands `idlewarden_core` types back to the
//! web view. No decision about a session is taken here.

mod logs;
mod profiles;
mod session;
mod updates;

#[cfg(test)]
mod tests;

fn data_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

pub fn run() {
    use std::sync::Arc;
    use tauri::Manager;
    use tracing_subscriber::prelude::*;

    // Everything below DEBUG is noise the user cannot act on; the Logs screen
    // filters the rest.
    let buffered = Arc::new(logs::LogBuffer::default());
    tracing_subscriber::registry()
        .with(logs::BufferLayer::new(Arc::clone(&buffered)))
        .with(tracing_subscriber::filter::LevelFilter::DEBUG)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            app.manage(updates::Updates::new(app.handle()));
            app.manage(session::SessionHandle::new(data_dir(app.handle())));
            app.manage(Arc::clone(&buffered));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            session::session_state,
            session::session_events,
            session::dispatch,
            session::engage_kill_switch,
            session::plugins,
            session::window_candidates,
            session::set_intent_enabled,
            logs::drain_logs,
            session::profile,
            session::set_profile,
            updates::update_settings,
            updates::set_update_channel,
            updates::check_for_update,
            updates::install_update
        ])
        .run(tauri::generate_context!())
        .expect("the tauri runtime failed to start");
}
