use crate::commands::AppState;
use crate::paths::find_repo_root;
use std::path::PathBuf;
use tauri::Manager;

pub fn run_gui() {
    crate::logging::init_gui();
    crate::error::install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            crate::logging::set_gui_app_handle(app.handle().clone());
            // Resolved here (not on the builder) so the Tauri app handle is
            // available to locate the bundled tap in a packaged install.
            app.manage(AppState {
                repo_root: resolve_repo_root(app),
            });
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
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}

/// Resolution order: `RELIQUAINT_REPO_ROOT` override → walk up from the cwd
/// (dev workflow) → the tap bundled as a Tauri resource (packaged install,
/// authoritative via the app handle) → exe-relative packaged fallback →
/// cwd (with a warning).
fn resolve_repo_root(app: &tauri::App) -> PathBuf {
    if let Ok(override_root) = std::env::var("RELIQUAINT_REPO_ROOT") {
        return PathBuf::from(override_root);
    }
    if let Some(root) = std::env::current_dir()
        .ok()
        .and_then(|cwd| find_repo_root(&cwd))
    {
        return root;
    }
    if let Ok(res) = app.path().resource_dir() {
        if res.join("tap/tap.toml").is_file() {
            return res;
        }
    }
    if let Some(root) = crate::paths::packaged_repo_root() {
        return root;
    }
    eprintln!("warning: cannot locate repo root; set RELIQUAINT_REPO_ROOT");
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
