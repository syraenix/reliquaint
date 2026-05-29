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
    pub collection_name: Option<String>,
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
    pub amiga_forever: Option<String>,
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
    let mut taps: Vec<crate::tap::LoadedTap> = Vec::new();

    let tap_root = crate::paths::tap_root(repo_root);
    match crate::tap::load_tap(&tap_root) {
        Ok(t) => taps.push(t),
        Err(crate::tap::TapError::MissingRoot { .. }) => {
            tracing::warn!(
                root = %tap_root.display(),
                "bundled tap not found; treating catalog as empty"
            );
        }
        Err(e) => return Err(format!("failed to load bundled tap: {e}")),
    }

    let user_tap_root = crate::paths::user_tap_dir();
    match crate::tap::load_user_tap(&user_tap_root) {
        Ok(t) => taps.push(t),
        Err(crate::tap::TapError::MissingRoot { .. }) => {}
        Err(e) => {
            tracing::warn!(error = %e, "user tap failed to load");
        }
    }

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
        collection_name: e.catalog.game.collection_name.clone(),
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
            amiga_forever: acq.amiga_forever.clone(),
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
/// the Svelte side can `switch` on. `install_path` is where the game will
/// live once committed; on `MissingFiles` the staged copy is held and the
/// frontend calls `commit_install` (install anyway) or `discard_install`.
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

/// Install the catalog entry `id` by staging `source` into the managed
/// library (default `~/games/<id>`, or `<dest>/<id>` when `dest` is given) and
/// committing it. Streams copy/extract output as `install-output` events.
///
/// Stage-then-commit: the source is copied/extracted into a sibling staging
/// dir; only when the expected files are present is it committed (atomic
/// rename) and a record written. If files are missing, returns `MissingFiles`
/// with the staging left in place — the frontend then calls `commit_install`
/// ("install anyway") or `discard_install` (cancel).
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

    let expects = entry.catalog.install.expects_files.clone();
    let tap_id = entry.tap_id.clone();
    let catalog_id = entry.catalog.game.id.clone();

    // Stage the copy/extract on a blocking thread; stream each line as an event.
    let plan_for_thread = plan.clone();
    let app_for_thread = app.clone();
    let id_for_emit = id.clone();
    let staged = tauri::async_runtime::spawn_blocking(move || {
        crate::game_install::stage(&plan_for_thread, move |cmds| {
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
    if let Err(e) = staged {
        let _ = crate::game_install::discard_staging(&plan.staging_dir);
        return Err(e.to_string());
    }

    let install_path_str = plan.install_path.to_string_lossy().into_owned();
    let missing = crate::install_record::missing_expects_files(&plan.staged_install_path, &expects);
    if !missing.is_empty() {
        // Leave staging in place; the frontend decides commit vs discard.
        return Ok(InstallGameOutcome::MissingFiles {
            missing,
            install_path: install_path_str,
        });
    }

    crate::game_install::commit(&plan.staging_dir, &plan.staged_install_path, &plan.dest_dir)
        .map_err(|e| e.to_string())?;
    let record_path = match crate::install_record::register(
        &catalog_id,
        &tap_id,
        &plan.install_path,
        &crate::paths::installs_dir(),
    ) {
        Ok(rp) => rp,
        Err(e) => {
            // Roll back the just-committed dir so it doesn't block retries.
            let _ = crate::game_install::discard_staging(&plan.dest_dir);
            return Err(e.to_string());
        }
    };
    Ok(InstallGameOutcome::Installed {
        record_path: record_path.to_string_lossy().into_owned(),
        install_path: install_path_str,
    })
}

/// Commit a staged install the user chose to keep despite missing expected
/// files: rename the staging dir into place and write the record. Backs the
/// GUI's "install anyway" path after `install_game` returns `MissingFiles`.
#[tauri::command]
pub fn commit_install(
    id: String,
    dest: Option<PathBuf>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let view = load_catalog_view(&state.repo_root)?;
    let entry = view
        .by_id(&id)
        .ok_or_else(|| format!("no catalog entry for '{id}'"))?;
    let dest_base = dest.unwrap_or_else(crate::paths::default_library_dir);
    let loc = crate::game_install::locations(
        &entry.catalog.game.id,
        entry.catalog.install.subdir.as_deref(),
        &dest_base,
    );
    crate::game_install::commit(&loc.staging_dir, &loc.staged_install_path, &loc.dest_dir)
        .map_err(|e| e.to_string())?;
    let record_path = match crate::install_record::register(
        &entry.catalog.game.id,
        &entry.tap_id,
        &loc.install_path,
        &crate::paths::installs_dir(),
    ) {
        Ok(rp) => rp,
        Err(e) => {
            let _ = crate::game_install::discard_staging(&loc.dest_dir);
            return Err(e.to_string());
        }
    };
    Ok(record_path.to_string_lossy().into_owned())
}

/// Discard a staged install (remove the staging dir) when the user cancels or
/// backs out after `install_game` returned `MissingFiles`. Idempotent.
#[tauri::command]
pub fn discard_install(
    id: String,
    dest: Option<PathBuf>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let view = load_catalog_view(&state.repo_root)?;
    let entry = view
        .by_id(&id)
        .ok_or_else(|| format!("no catalog entry for '{id}'"))?;
    let dest_base = dest.unwrap_or_else(crate::paths::default_library_dir);
    let loc = crate::game_install::locations(
        &entry.catalog.game.id,
        entry.catalog.install.subdir.as_deref(),
        &dest_base,
    );
    crate::game_install::discard_staging(&loc.staging_dir).map_err(|e| e.to_string())
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
        crate::catalog::Platform::Dos => {
            crate::launch::compose_dosbox(&entry.catalog, &entry.source_path, install, &user_config)
        }
        crate::catalog::Platform::Amiga => {
            crate::launch::compose_fs_uae(&entry.catalog, &entry.source_path, install, &user_config)
        }
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
    let parsed = parse_kind_tag(&kind).ok_or_else(|| format!("unknown dependency kind: {kind}"))?;
    let distro = detect_distro();
    let action = action_for(&parsed, distro).ok_or_else(|| {
        "no install action available for this dependency on this distro".to_string()
    })?;
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
                stream: if is_err {
                    "stderr".into()
                } else {
                    "stdout".into()
                },
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

// --- M5: Manifest creation wizard ---

#[derive(Serialize, Clone)]
pub struct HeuristicReportDto {
    pub platform: Option<String>,
    pub confidence: String,
    pub platform_evidence: Vec<String>,
    pub dos_candidates: Vec<DosCandidateDto>,
    pub amiga: AmigaReportDto,
    pub metadata: DraftMetadataDto,
    pub source_path: String,
}

#[derive(Serialize, Clone)]
pub struct DosCandidateDto {
    pub file_name: String,
    pub kind: String,
    pub reason: String,
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AmigaReportDto {
    Floppies { files: Vec<String> },
    ManualEntry { reason: String },
}

#[derive(Serialize, Clone)]
pub struct DraftMetadataDto {
    pub id: String,
    pub title: String,
}

#[derive(serde::Deserialize)]
pub struct SaveUserManifestRequest {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub install_path: String,
    pub entry: Option<String>,
    pub floppies: Option<Vec<String>>,
    pub year: Option<u32>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct SaveUserManifestResponse {
    pub id: String,
    pub manifest_path: String,
    pub config_path: Option<String>,
    pub install_record_path: String,
}

/// Run the heuristic engine against `path` and return a serializable
/// report the frontend wizard uses to seed its form. The actual draft
/// composition runs in [`save_user_manifest`] after the user reviews.
#[tauri::command]
pub fn detect_game(path: String) -> Result<HeuristicReportDto, String> {
    let p = std::path::PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let report = crate::heuristic::analyze(&p);

    let platform = report.platform.platform.map(|p| match p {
        crate::catalog::Platform::Dos => "dos".to_string(),
        crate::catalog::Platform::Amiga => "amiga".to_string(),
    });
    let confidence = format!("{:?}", report.platform.confidence).to_ascii_lowercase();
    let dos_candidates = report
        .dos
        .iter()
        .map(|c| DosCandidateDto {
            file_name: c.file_name.clone(),
            kind: match c.kind {
                crate::heuristic::EntryPointKind::Bat => "bat".to_string(),
                crate::heuristic::EntryPointKind::Exe => "exe".to_string(),
                crate::heuristic::EntryPointKind::Com => "com".to_string(),
            },
            reason: c.reason.clone(),
        })
        .collect();
    let amiga = match &report.amiga {
        crate::heuristic::AmigaEntryPoints::Floppies(v) => {
            AmigaReportDto::Floppies { files: v.clone() }
        }
        crate::heuristic::AmigaEntryPoints::ManualEntry { reason } => AmigaReportDto::ManualEntry {
            reason: reason.clone(),
        },
    };
    Ok(HeuristicReportDto {
        platform,
        confidence,
        platform_evidence: report.platform.evidence,
        dos_candidates,
        amiga,
        metadata: DraftMetadataDto {
            id: report.metadata.id,
            title: report.metadata.title,
        },
        source_path: p.to_string_lossy().into_owned(),
    })
}

/// Take the (possibly user-edited) wizard form payload, compose a
/// `CatalogEntry`, validate it, and commit to the user tap.
#[tauri::command]
pub fn save_user_manifest(
    req: SaveUserManifestRequest,
    state: State<'_, AppState>,
) -> Result<SaveUserManifestResponse, String> {
    let view = load_catalog_view(&state.repo_root)?;
    let bundled_ids: Vec<String> = view
        .all()
        .iter()
        .filter(|e| e.tap_id != crate::tap::RESERVED_USER_TAP_ID)
        .map(|e| e.catalog.game.id.clone())
        .collect();
    let user_ids: Vec<String> = view
        .all()
        .iter()
        .filter(|e| e.tap_id == crate::tap::RESERVED_USER_TAP_ID)
        .map(|e| e.catalog.game.id.clone())
        .collect();

    let platform = match req.platform.as_str() {
        "dos" => crate::catalog::Platform::Dos,
        "amiga" => crate::catalog::Platform::Amiga,
        other => return Err(format!("unknown platform: {other}")),
    };

    let meta = crate::catalog::Meta {
        year: req.year,
        developer: req.developer.clone(),
        publisher: req.publisher.clone(),
        description: req.description.clone(),
        ..Default::default()
    };

    let runtime = match platform {
        crate::catalog::Platform::Dos => {
            let entry = req
                .entry
                .clone()
                .ok_or_else(|| "DOS entry: missing `entry` field".to_string())?;
            crate::catalog::Runtime {
                emulator: crate::catalog::Emulator::DosboxStaging,
                sidecars: vec![],
                dosbox: Some(crate::catalog::DosboxRuntime {
                    config: format!("{}.conf", req.id),
                    entry,
                    mount: "c".to_string(),
                }),
                fs_uae: None,
            }
        }
        crate::catalog::Platform::Amiga => {
            let floppies = req.floppies.clone().unwrap_or_default();
            crate::catalog::Runtime {
                emulator: crate::catalog::Emulator::FsUae,
                sidecars: vec![],
                dosbox: None,
                fs_uae: Some(crate::catalog::FsUaeRuntime {
                    model: crate::catalog::AmigaModel::A500,
                    config: None,
                    floppies,
                    hard_drives: vec![],
                }),
            }
        }
    };

    let draft = crate::catalog::CatalogEntry {
        schema_version: 1,
        game: crate::catalog::Game {
            id: req.id.clone(),
            title: req.title.clone(),
            platform,
            collection: None,
            collection_name: None,
        },
        meta,
        acquisition: crate::catalog::Acquisition::default(),
        install: crate::catalog::Install::default(),
        runtime,
    };

    crate::wizard::validate(&draft).map_err(|e| e.to_string())?;
    let result = crate::wizard::commit(
        &crate::wizard::AddOptions {
            source: std::path::PathBuf::from(&req.install_path),
            user_tap_root: crate::paths::user_tap_dir(),
            installs_dir: crate::paths::installs_dir(),
            platform_override: None,
            bundled_ids: &bundled_ids,
            user_ids: &user_ids,
        },
        &draft,
    )
    .map_err(|e| e.to_string())?;

    Ok(SaveUserManifestResponse {
        id: result.id,
        manifest_path: result.manifest_path.to_string_lossy().into_owned(),
        config_path: result.config_path.map(|p| p.to_string_lossy().into_owned()),
        install_record_path: result.install_record_path.to_string_lossy().into_owned(),
    })
}

// --- M6: Upstream submission ---

#[derive(Serialize, Clone)]
pub struct SubmitManifestResponse {
    pub content: String,
    pub warnings: Vec<String>,
    pub github_url: String,
    pub target_path: String,
}

/// Produce a submission-ready manifest for the user-tap entry `id`.
/// Refuses bundled entries (they are immutable per ADR-0001 / 0003).
#[tauri::command]
pub fn submit_manifest(
    id: String,
    state: State<'_, AppState>,
) -> Result<SubmitManifestResponse, String> {
    let view = load_catalog_view(&state.repo_root)?;
    let entry = view
        .by_id(&id)
        .ok_or_else(|| format!("no catalog entry for {id:?}"))?;
    if entry.tap_id != crate::tap::RESERVED_USER_TAP_ID {
        return Err(format!(
            "{id:?} belongs to the bundled tap; only user-created entries are submission candidates"
        ));
    }
    let exported = crate::export::export_manifest(&entry.catalog).map_err(|e| e.to_string())?;
    let platform = match entry.catalog.game.platform {
        crate::catalog::Platform::Dos => "dos",
        crate::catalog::Platform::Amiga => "amiga",
    };
    let github_url = crate::export::github_new_file_url(platform, "develop");
    Ok(SubmitManifestResponse {
        content: exported.content,
        warnings: exported.warnings,
        github_url,
        target_path: format!("tap/catalog/{platform}/{id}.toml"),
    })
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
        let ids: Vec<&str> = view
            .all()
            .iter()
            .map(|e| e.catalog.game.id.as_str())
            .collect();
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
            assert!(validate_external_url(url).is_err(), "should reject {url:?}");
        }
    }
}
