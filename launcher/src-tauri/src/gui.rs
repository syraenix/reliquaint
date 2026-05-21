use crate::commands::AppState;
use crate::discovery::find_repo_root;
use std::path::PathBuf;

pub fn run_gui() {
    let repo_root = resolve_repo_root().unwrap_or_else(|| {
        eprintln!("warning: cannot locate repo root; set CLASSIC_LAUNCHER_REPO_ROOT");
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { repo_root })
        .invoke_handler(tauri::generate_handler![
            crate::commands::list_games,
            crate::commands::launch_game,
            crate::commands::run_doctor,
            crate::commands::install_dependency,
            crate::commands::default_installers_dir,
            crate::commands::discover_qfg_installers,
            crate::commands::build_kq_entry,
            crate::commands::install_games,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}

fn resolve_repo_root() -> Option<PathBuf> {
    if let Ok(override_root) = std::env::var("CLASSIC_LAUNCHER_REPO_ROOT") {
        return Some(PathBuf::from(override_root));
    }
    let cwd = std::env::current_dir().ok()?;
    find_repo_root(&cwd)
}
