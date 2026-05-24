use crate::doctor::{ProbeKind, ProbeStatus};
use crate::installer::run_install;
use crate::setup::{action_for, build_commands, detect_distro};
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
pub struct DoctorResult {
    pub name: String,
    pub status: String,
    pub detail: Option<String>,
    pub kind: String,
}

pub fn kind_tag(kind: &ProbeKind) -> String {
    match kind {
        ProbeKind::DosboxFlatpak => "dosbox-flatpak".into(),
        ProbeKind::Fluidsynth => "fluidsynth".into(),
        ProbeKind::Soundfont => "soundfont".into(),
        ProbeKind::Innoextract => "innoextract".into(),
        ProbeKind::FsUae => "fs-uae".into(),
        ProbeKind::Unzip => "unzip".into(),
        ProbeKind::GameInstallDir(id) => format!("game-install-dir:{id}"),
    }
}

pub fn parse_kind_tag(tag: &str) -> Option<ProbeKind> {
    match tag {
        "dosbox-flatpak" => Some(ProbeKind::DosboxFlatpak),
        "fluidsynth" => Some(ProbeKind::Fluidsynth),
        "soundfont" => Some(ProbeKind::Soundfont),
        "innoextract" => Some(ProbeKind::Innoextract),
        "fs-uae" => Some(ProbeKind::FsUae),
        "unzip" => Some(ProbeKind::Unzip),
        other => other
            .strip_prefix("game-install-dir:")
            .map(|id| ProbeKind::GameInstallDir(id.to_string())),
    }
}

pub struct AppState {
    pub repo_root: PathBuf,
}

/// One row of the catalog as the Svelte frontend consumes it. Flat
/// rather than mirroring the nested CatalogEntry types so the IPC
/// payload is straightforward to read on the JS side.
#[derive(Serialize, Clone)]
pub struct CatalogEntryDto {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub collection: Option<String>,
    pub year: Option<u32>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Vec<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub acquisition: AcquisitionDto,
    pub tap_id: String,
    pub installed: bool,
    pub install_path: Option<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct AcquisitionDto {
    pub gog: Option<String>,
    pub steam: Option<String>,
    pub developer_site: Option<String>,
    pub archive: Option<String>,
    pub notes: Option<String>,
}

pub fn load_catalog_view(repo_root: &Path) -> Result<crate::catalog_view::CatalogView, String> {
    load_catalog_view_with(repo_root, &crate::paths::installs_dir())
}

/// Like [`load_catalog_view`] but with an explicit installs directory, so
/// tests can isolate from the developer's real `paths::installs_dir()`.
fn load_catalog_view_with(
    repo_root: &Path,
    installs_dir: &Path,
) -> Result<crate::catalog_view::CatalogView, String> {
    let tap_root = crate::paths::tap_root(repo_root);
    let taps = match crate::tap::load_tap(&tap_root) {
        Ok(t) => vec![t],
        Err(crate::tap::TapError::MissingRoot { .. }) => {
            tracing::warn!(
                root = %tap_root.display(),
                "bundled tap not found; treating catalog as empty"
            );
            Vec::new()
        }
        Err(e) => return Err(format!("failed to load bundled tap: {e}")),
    };
    let installs = crate::install_record::load_all(installs_dir);
    Ok(crate::catalog_view::CatalogView::assemble(taps, installs))
}

pub fn entry_to_dto(e: &crate::catalog_view::CatalogViewEntry) -> CatalogEntryDto {
    let acq = &e.catalog.acquisition;
    CatalogEntryDto {
        id: e.catalog.game.id.clone(),
        title: e.catalog.game.title.clone(),
        platform: match e.catalog.game.platform {
            crate::catalog::Platform::Dos => "dos".to_string(),
            crate::catalog::Platform::Amiga => "amiga".to_string(),
        },
        collection: e.catalog.game.collection.clone(),
        year: e.catalog.meta.year,
        developer: e.catalog.meta.developer.clone(),
        publisher: e.catalog.meta.publisher.clone(),
        genre: e.catalog.meta.genre.clone(),
        tags: e.catalog.meta.tags.clone(),
        description: e.catalog.meta.description.clone(),
        acquisition: AcquisitionDto {
            gog: acq.gog.clone(),
            steam: acq.steam.clone(),
            developer_site: acq.developer_site.clone(),
            archive: acq.archive.clone(),
            notes: acq.notes.clone(),
        },
        tap_id: e.tap_id.clone(),
        installed: e.install.is_some(),
        install_path: e
            .install
            .as_ref()
            .map(|i| i.install.install_path.to_string_lossy().into_owned()),
    }
}

#[tauri::command]
pub fn list_catalog(state: State<'_, AppState>) -> Result<Vec<CatalogEntryDto>, String> {
    let view = load_catalog_view(&state.repo_root)?;
    Ok(view.all().iter().map(entry_to_dto).collect())
}

/// Result payload for `install_game`. Serializes as a tagged JSON object
/// the Svelte side can `switch` on. `install_path` is the directory the
/// game was copied/extracted into; on `MissingFiles` the frontend hands it
/// back to `register_install` if the user chooses to install anyway.
#[derive(Serialize, Clone)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InstallGameOutcome {
    Installed {
        record_path: String,
        install_path: String,
    },
    MissingFiles {
        missing: Vec<String>,
        install_path: String,
    },
}

/// Install the catalog entry `id` by copying/extracting `source` into the
/// managed library (default `~/games/<id>`, or `<dest>/<id>` when `dest` is
/// given). Streams copy/extract output as `install-output` events. Writes
/// the install record only when the expected files are present afterward;
/// otherwise returns `MissingFiles` and the frontend confirms via
/// `register_install`.
#[tauri::command]
pub async fn install_game(
    id: String,
    source: PathBuf,
    dest: Option<PathBuf>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<InstallGameOutcome, String> {
    let view = load_catalog_view(&state.repo_root)?;
    let entry = view
        .by_id(&id)
        .ok_or_else(|| format!("no catalog entry for '{id}'"))?;

    let dest_base = dest.unwrap_or_else(crate::paths::default_library_dir);
    let spec = crate::game_install::EntrySpec::from_entry(&entry.catalog);
    let plan =
        crate::game_install::plan_install(&spec, &source, &dest_base).map_err(|e| e.to_string())?;

    let install_path = plan.install_path.clone();
    let expects = entry.catalog.install.expects_files.clone();
    let tap_id = entry.tap_id.clone();
    let catalog_id = entry.catalog.game.id.clone();

    // Run the copy/extract on a blocking thread; stream each line as an event.
    let app_for_thread = app.clone();
    let id_for_emit = id.clone();
    let exec = tauri::async_runtime::spawn_blocking(move || {
        crate::game_install::execute(&plan, move |cmds| {
            let app_cb = app_for_thread.clone();
            let emit_id = id_for_emit.clone();
            crate::installer::run_install(cmds.to_vec(), move |line, is_err| {
                let _ = app_cb.emit(
                    "install-output",
                    serde_json::json!({
                        "id": emit_id.clone(),
                        "stream": if is_err { "stderr" } else { "stdout" },
                        "line": line,
                    }),
                );
            })
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    exec.map_err(|e| e.to_string())?;

    let missing = crate::install_record::missing_expects_files(&install_path, &expects);
    let install_path_str = install_path.to_string_lossy().into_owned();
    if !missing.is_empty() {
        return Ok(InstallGameOutcome::MissingFiles {
            missing,
            install_path: install_path_str,
        });
    }

    let record_path = crate::install_record::register(
        &catalog_id,
        &tap_id,
        &install_path,
        &crate::paths::installs_dir(),
    )
    .map_err(|e| e.to_string())?;
    Ok(InstallGameOutcome::Installed {
        record_path: record_path.to_string_lossy().into_owned(),
        install_path: install_path_str,
    })
}

/// Write the install record for an already-populated directory. Backs the
/// GUI's "Install anyway" confirm path after `install_game` reports
/// `MissingFiles`.
#[tauri::command]
pub fn register_install(
    id: String,
    install_path: PathBuf,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let view = load_catalog_view(&state.repo_root)?;
    let entry = view
        .by_id(&id)
        .ok_or_else(|| format!("no catalog entry for '{id}'"))?;
    let record_path = crate::install_record::register(
        &entry.catalog.game.id,
        &entry.tap_id,
        &install_path,
        &crate::paths::installs_dir(),
    )
    .map_err(|e| e.to_string())?;
    Ok(record_path.to_string_lossy().into_owned())
}

/// The default destination shown in the install dialog: `~/games/<id>`.
#[tauri::command]
pub fn default_install_dest(id: String) -> String {
    crate::paths::games_dir(&crate::paths::default_library_dir(), &id)
        .to_string_lossy()
        .into_owned()
}

/// Open an `http://` or `https://` URL in the user's browser via
/// `xdg-open`. Rejects other schemes (e.g. `file:`, `javascript:`) as
/// defense in depth — the URL ultimately came from a catalog entry,
/// but the renderer hands us a string and we shouldn't blindly trust it.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    validate_external_url(&url)?;
    std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("failed to spawn xdg-open: {e}"))?;
    Ok(())
}

fn validate_external_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err(format!("refusing to open non-http(s) URL: {url}"))
    }
}

/// Spawn the game in a background thread; emit `emulator-output`
/// events for each stdout/stderr line and `emulator-exit` when the
/// emulator exits. Returns immediately after the spawn — the frontend
/// tracks the launch lifecycle via events.
#[tauri::command]
pub async fn launch_game(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let view = load_catalog_view(&state.repo_root)?;
    let entry = view
        .by_id(&id)
        .ok_or_else(|| format!("no catalog entry for '{id}'"))?;
    let install = entry
        .install
        .as_ref()
        .ok_or_else(|| format!("'{id}' has no installation record"))?;
    let user_config = crate::user_config::load_or_default(&crate::paths::user_config_path());

    let plan = match entry.catalog.game.platform {
        crate::catalog::Platform::Dos => crate::launch::compose_dosbox(
            &entry.catalog,
            &entry.source_path,
            install,
            &user_config,
        ),
        crate::catalog::Platform::Amiga => crate::launch::compose_fs_uae(
            &entry.catalog,
            &entry.source_path,
            install,
            &user_config,
        ),
    }
    .map_err(|e| e.to_string())?;

    let app_for_thread = app.clone();
    let id_for_thread = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let app_for_cb = app_for_thread.clone();
        let result = crate::sidecar::run_plan_with_callback(plan, move |source, line| {
            let stream = match source {
                crate::sidecar::OutputSource::Stdout => "stdout",
                crate::sidecar::OutputSource::Stderr => "stderr",
            };
            let _ = app_for_cb.emit(
                "emulator-output",
                serde_json::json!({
                    "stream": stream,
                    "line": line,
                }),
            );
        });
        let (exit_code, error_msg) = match &result {
            Ok(s) => (s.code().unwrap_or(-1), None),
            Err(e) => (-1, Some(e.to_string())),
        };
        let _ = app_for_thread.emit(
            "emulator-exit",
            serde_json::json!({
                "id": id_for_thread,
                "code": exit_code,
                "error": error_msg,
            }),
        );
    });
    Ok(())
}

#[tauri::command]
pub fn run_doctor(state: State<'_, AppState>) -> Result<Vec<DoctorResult>, String> {
    let view = load_catalog_view(&state.repo_root)?;
    let user_config = crate::user_config::load_or_default(&crate::paths::user_config_path());
    let results = crate::doctor::check_install(&view, &user_config);
    Ok(results
        .into_iter()
        .map(|r| DoctorResult {
            name: r.name,
            status: match r.status {
                ProbeStatus::Ok => "ok".into(),
                ProbeStatus::Missing => "missing".into(),
                ProbeStatus::Unknown => "unknown".into(),
            },
            detail: r.detail,
            kind: kind_tag(&r.kind),
        })
        .collect())
}

#[derive(Serialize, Clone)]
struct InstallOutputPayload {
    kind: String,
    line: String,
    stream: String,
}

#[derive(Serialize, Clone)]
struct InstallFinishedPayload {
    kind: String,
    exit_code: i32,
}

#[tauri::command]
pub async fn install_dependency(kind: String, app: AppHandle) -> Result<i32, String> {
    let parsed =
        parse_kind_tag(&kind).ok_or_else(|| format!("unknown dependency kind: {kind}"))?;
    let distro = detect_distro();
    let action = action_for(&parsed, distro)
        .ok_or_else(|| "no install action available for this dependency on this distro".to_string())?;
    let cmds = build_commands(&action);
    if cmds.is_empty() {
        return Err("this dependency cannot be installed automatically".to_string());
    }

    let kind_for_emit = kind.clone();
    let app_for_emit = app.clone();

    let exit_code = tauri::async_runtime::spawn_blocking(move || {
        let emit_kind = kind_for_emit.clone();
        let emit_app = app_for_emit.clone();
        run_install(cmds, move |line, is_err| {
            let payload = InstallOutputPayload {
                kind: emit_kind.clone(),
                line: line.to_string(),
                stream: if is_err { "stderr".into() } else { "stdout".into() },
            };
            let _ = emit_app.emit("install-output", payload);
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    let _ = app.emit(
        "install-finished",
        InstallFinishedPayload {
            kind: kind.clone(),
            exit_code,
        },
    );

    Ok(exit_code)
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn load_catalog_view_finds_fixture_tap_entries() {
        // Isolate from the developer's real installs dir so the test is
        // deterministic regardless of what is installed on this machine.
        let installs = tempfile::tempdir().unwrap();
        let view = load_catalog_view_with(&fixture_repo_root(), installs.path()).unwrap();
        let ids: Vec<&str> = view.all().iter().map(|e| e.catalog.game.id.as_str()).collect();
        assert!(ids.contains(&"qfg1-ega"), "expected qfg1-ega in {ids:?}");
        assert!(ids.contains(&"fatman"), "expected fatman in {ids:?}");
    }

    #[test]
    fn entry_to_dto_carries_metadata_and_acquisition() {
        // Empty installs dir → qfg1-ega is deterministically "not installed".
        let installs = tempfile::tempdir().unwrap();
        let view = load_catalog_view_with(&fixture_repo_root(), installs.path()).unwrap();
        let qfg = view.by_id("qfg1-ega").expect("qfg1-ega fixture missing");
        let dto = entry_to_dto(qfg);

        assert_eq!(dto.id, "qfg1-ega");
        assert_eq!(dto.platform, "dos");
        assert_eq!(dto.collection.as_deref(), Some("quest-for-glory"));
        assert_eq!(dto.year, Some(1989));
        assert_eq!(dto.developer.as_deref(), Some("Sierra On-Line"));
        assert_eq!(dto.tap_id, "reliquaint-core");
        assert!(!dto.installed);
        assert!(dto.install_path.is_none());
        assert!(dto.acquisition.gog.as_deref().unwrap().contains("gog.com"));
        assert!(dto.acquisition.notes.is_some());
    }

    #[test]
    fn load_catalog_view_returns_empty_when_no_tap_present() {
        let tmp = tempfile::tempdir().unwrap();
        let view = load_catalog_view(tmp.path()).unwrap();
        assert!(view.all().is_empty());
    }

    #[test]
    fn validate_external_url_accepts_http_and_https() {
        assert!(validate_external_url("http://example.test/").is_ok());
        assert!(validate_external_url("https://example.test/").is_ok());
    }

    #[test]
    fn validate_external_url_rejects_dangerous_schemes() {
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,<script>",
            "ftp://example.test/",
            "",
            "/etc/passwd",
        ] {
            assert!(
                validate_external_url(url).is_err(),
                "should reject {url:?}"
            );
        }
    }
}
