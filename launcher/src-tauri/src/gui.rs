use crate::commands::AppState;
use crate::discovery::find_repo_root;
use crate::paths::expand_tilde;
use std::path::PathBuf;

pub fn run_gui() {
    crate::logging::init_gui();
    crate::error::install_panic_hook();
    let repo_root = resolve_repo_root().unwrap_or_else(|| {
        eprintln!("warning: cannot locate repo root; set RELIQUAINT_REPO_ROOT");
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });
    let games_base = resolve_games_base();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { repo_root, games_base })
        .invoke_handler(tauri::generate_handler![
            crate::commands::list_games,
            crate::commands::list_catalog,
            crate::commands::launch_game,
            crate::commands::run_doctor,
            crate::commands::install_dependency,
            crate::commands::default_installers_dir,
            crate::commands::discover_qfg_installers,
            crate::commands::build_kq_entry,
            crate::commands::install_games,
            crate::commands::install_amiga_game,
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

fn resolve_games_base() -> PathBuf {
    if let Ok(base) = std::env::var("RELIQUAINT_GAMES_DIR") {
        return PathBuf::from(base);
    }
    expand_tilde("~/games")
}
