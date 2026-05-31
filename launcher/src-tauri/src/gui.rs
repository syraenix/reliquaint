use crate::commands::AppState;
use crate::companion_protocol::{resolve_image, ImageError};
use tauri::http::{header, Request, Response, StatusCode};
use tauri::Manager;

pub fn run_gui() {
    crate::logging::init_gui();
    crate::error::install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("reliquaint-content", |_ctx, request| {
            serve_companion_image(&request)
        })
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
            crate::commands::list_companion,
            crate::commands::render_companion,
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

/// Serve a `reliquaint-content://<tap-id>/<game-id>/<rel-path>` request.
///
/// The tap id rides in the URL authority and the game id is the first path
/// segment (both are `^[a-z][a-z0-9-]*[a-z0-9]$`, so they survive URL
/// normalization). All boundary/format enforcement lives in
/// [`crate::companion_protocol::resolve_image`]; this is just request parsing
/// and status mapping.
fn serve_companion_image(request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let uri = request.uri();
    let tap_id = uri.host().unwrap_or_default().to_string();
    let path = uri.path().trim_start_matches('/');
    let (game_id, rel_path) = path.split_once('/').unwrap_or((path, ""));

    match resolve_image(&tap_id, game_id, rel_path) {
        Ok((bytes, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .body(bytes)
            .unwrap(),
        Err(err) => {
            let status = match err {
                ImageError::NotFound => StatusCode::NOT_FOUND,
                ImageError::OutsideBoundary => StatusCode::FORBIDDEN,
                ImageError::BadFormat => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ImageError::Io => StatusCode::INTERNAL_SERVER_ERROR,
            };
            Response::builder().status(status).body(Vec::new()).unwrap()
        }
    }
}
