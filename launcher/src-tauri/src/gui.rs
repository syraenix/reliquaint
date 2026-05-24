use crate::commands::AppState;
use crate::paths::find_repo_root;
use std::path::PathBuf;

pub fn run_gui() {
    crate::logging::init_gui();
    crate::error::install_panic_hook();
    let repo_root = resolve_repo_root().unwrap_or_else(|| {
        eprintln!("warning: cannot locate repo root; set RELIQUAINT_REPO_ROOT");
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { repo_root })
        .setup(|app| {
            crate::logging::set_gui_app_handle(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::list_catalog,
            crate::commands::install_game,
            crate::commands::launch_game,
            crate::commands::run_doctor,
            crate::commands::install_dependency,
            crate::commands::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}

fn resolve_repo_root() -> Option<PathBuf> {
    if let Ok(override_root) = std::env::var("RELIQUAINT_REPO_ROOT") {
        return Some(PathBuf::from(override_root));
    }
    let cwd = std::env::current_dir().ok()?;
    find_repo_root(&cwd)
}
