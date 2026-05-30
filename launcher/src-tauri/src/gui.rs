use crate::commands::AppState;
use tauri::Manager;

pub fn run_gui() {
    crate::logging::init_gui();
    crate::error::install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            crate::logging::set_gui_app_handle(app.handle().clone());
            // The catalog is sourced entirely from subscribed taps + the user
            // tap (see commands::load_catalog_view); there is no bundled tap to
            // locate, so AppState carries no state.
            app.manage(AppState {});
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::list_catalog,
            crate::commands::install_game,
            crate::commands::commit_install,
            crate::commands::discard_install,
            crate::commands::default_install_dest,
            crate::commands::launch_game,
            crate::commands::run_doctor,
            crate::commands::install_dependency,
            crate::commands::open_url,
            crate::commands::detect_game,
            crate::commands::save_user_manifest,
            crate::commands::submit_manifest,
            crate::commands::list_taps,
            crate::commands::add_tap,
            crate::commands::remove_tap,
            crate::commands::sync_tap,
            crate::commands::reorder_tap,
            crate::commands::make_tap_default,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
