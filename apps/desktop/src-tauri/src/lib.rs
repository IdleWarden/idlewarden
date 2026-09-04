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

fn plugin_root(app: &tauri::AppHandle) -> std::path::PathBuf {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("plugins"))
        .unwrap_or_else(|_| std::path::PathBuf::from("plugins"))
}

pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.manage(updates::Updates::new(app.handle()));
            app.manage(session::SessionHandle::new(plugin_root(app.handle())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            session::session_state,
            session::session_events,
            session::dispatch,
            session::engage_kill_switch,
            updates::update_settings,
            updates::set_update_channel,
            updates::check_for_update,
            updates::install_update
        ])
        .run(tauri::generate_context!())
        .expect("the tauri runtime failed to start");
}
