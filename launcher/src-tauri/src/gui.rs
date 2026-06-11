use crate::commands::AppState;
use crate::companion_protocol::{resolve_image, ImageError};
use tauri::http::{header, Request, Response, StatusCode};
use tauri::Manager;

pub fn run_gui() {
    configure_webkit_renderer();
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

/// WebKitGTK's DMABUF renderer fails to create an EGL display on a number of
/// Linux graphics stacks (Fedora's Mesa builds, some NVIDIA/VM/Wayland combos),
/// aborting the web process with "Could not create default EGL display:
/// EGL_BAD_PARAMETER" and leaving the window blank. Disabling it forces the SHM
/// compositing fallback, which needs no EGL/GBM. Set only when unset so an
/// operator can opt back in (or apply a heavier mitigation) by exporting the
/// variable themselves.
fn configure_webkit_renderer() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
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
    // The path segments are percent-encoded; decode each exactly once so the
    // real on-disk path reaches the resolver (which re-checks the boundary).
    let (game_id, rel_path) = crate::companion_render::split_and_decode_content_path(uri.path());

    match resolve_image(&tap_id, &game_id, &rel_path) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    const VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

    /// Run `body` with `VAR` restored to its original value afterwards, so the
    /// process-global env stays hermetic for the rest of the suite.
    fn with_restored_var(body: impl FnOnce()) {
        let original = std::env::var_os(VAR);
        body();
        match original {
            Some(value) => std::env::set_var(VAR, value),
            None => std::env::remove_var(VAR),
        }
    }

    #[test]
    #[serial]
    fn sets_the_var_when_unset() {
        with_restored_var(|| {
            std::env::remove_var(VAR);
            configure_webkit_renderer();
            assert_eq!(std::env::var(VAR).as_deref(), Ok("1"));
        });
    }

    #[test]
    #[serial]
    fn respects_an_operator_override() {
        with_restored_var(|| {
            std::env::set_var(VAR, "0");
            configure_webkit_renderer();
            assert_eq!(std::env::var(VAR).as_deref(), Ok("0"));
        });
    }
}
