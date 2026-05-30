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

pub struct AppState {}

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
    /// Priority of the contributing tap (lower wins). Lets the frontend pick
    /// the conflict winner and order alternates.
    pub priority: i32,
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

pub fn load_catalog_view() -> Result<crate::catalog_view::CatalogView, String> {
    load_catalog_view_with(&crate::paths::installs_dir())
}

/// Like [`load_catalog_view`] but with an explicit installs directory, so
/// tests can isolate from the developer's real `paths::installs_dir()`.
/// Subscription, tap-cache, and user-tap locations still come from `paths`.
fn load_catalog_view_with(installs_dir: &Path) -> Result<crate::catalog_view::CatalogView, String> {
    assemble_catalog_view(
        installs_dir,
        &crate::paths::subscriptions_path(),
        &crate::paths::user_taps_dir(),
        &crate::paths::user_tap_dir(),
    )
}

/// Fully-injectable catalog assembly. Every external location is a parameter,
/// so tests can run hermetically without touching the developer's real config.
///
/// The catalog is sourced entirely from subscribed taps and the local user
/// tap — there is no bundled content (`reliquaint-core` is now a separate
/// repository the user subscribes to).
fn assemble_catalog_view(
    installs_dir: &Path,
    subscriptions_path: &Path,
    taps_cache_dir: &Path,
    user_tap_dir: &Path,
) -> Result<crate::catalog_view::CatalogView, String> {
    let mut taps: Vec<crate::tap::LoadedTap> = Vec::new();
    let mut priorities: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    // 1. Subscribed taps
    let subs = crate::subscriptions::SubscriptionManifest::load_or_empty(subscriptions_path)
        .unwrap_or_else(|e| {
            tracing::warn!("could not load subscriptions.toml: {e}");
            crate::subscriptions::SubscriptionManifest::empty()
        });
    for sub in &subs.taps {
        let cache_dir = taps_cache_dir.join(&sub.id);
        match crate::tap::load_tap(&cache_dir) {
            Ok(t) => {
                priorities.insert(t.metadata.id.clone(), sub.priority as i32);
                taps.push(t);
            }
            Err(crate::tap::TapError::MissingRoot { .. }) => {
                tracing::warn!(tap = %sub.id, "subscribed tap cache missing");
            }
            Err(e) => tracing::warn!(tap = %sub.id, "subscribed tap failed: {e}"),
        }
    }

    // 2. Local user tap (always wins)
    match crate::tap::load_user_tap(user_tap_dir) {
        Ok(t) => {
            priorities.insert(t.metadata.id.clone(), -1);
            taps.push(t);
        }
        Err(crate::tap::TapError::MissingRoot { .. }) => {}
        Err(e) => tracing::warn!(error = %e, "user tap failed to load"),
    }

    let installs = crate::install_record::load_all(installs_dir);
    Ok(crate::catalog_view::CatalogView::assemble_with_priorities(
        taps, installs, priorities,
    ))
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
        priority: e.priority,
        installed: e.install.is_some(),
        install_path: e
            .install
            .as_ref()
            .map(|i| i.install.install_path.to_string_lossy().into_owned()),
    }
}

#[tauri::command]
pub fn list_catalog(_state: State<'_, AppState>) -> Result<Vec<CatalogEntryDto>, String> {
    let view = load_catalog_view()?;
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
    _state: State<'_, AppState>,
) -> Result<InstallGameOutcome, String> {
    let view = load_catalog_view()?;
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
    _state: State<'_, AppState>,
) -> Result<String, String> {
    let view = load_catalog_view()?;
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
    _state: State<'_, AppState>,
) -> Result<(), String> {
    let view = load_catalog_view()?;
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
    _state: State<'_, AppState>,
) -> Result<(), String> {
    let view = load_catalog_view()?;
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
pub fn run_doctor(_state: State<'_, AppState>) -> Result<Vec<DoctorResult>, String> {
    let view = load_catalog_view()?;
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
    _state: State<'_, AppState>,
) -> Result<SaveUserManifestResponse, String> {
    let view = load_catalog_view()?;
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
/// Refuses subscribed-tap entries (only user-created entries are candidates).
#[tauri::command]
pub fn submit_manifest(
    id: String,
    _state: State<'_, AppState>,
) -> Result<SubmitManifestResponse, String> {
    let view = load_catalog_view()?;
    let entry = view
        .by_id(&id)
        .ok_or_else(|| format!("no catalog entry for {id:?}"))?;
    if entry.tap_id != crate::tap::RESERVED_USER_TAP_ID {
        return Err(format!(
            "{id:?} belongs to a subscribed tap; only user-created entries are submission candidates"
        ));
    }
    let exported = crate::export::export_manifest(&entry.catalog).map_err(|e| e.to_string())?;
    let platform = match entry.catalog.game.platform {
        crate::catalog::Platform::Dos => "dos",
        crate::catalog::Platform::Amiga => "amiga",
    };
    let github_url = crate::export::github_new_file_url(platform, "main");
    Ok(SubmitManifestResponse {
        content: exported.content,
        warnings: exported.warnings,
        github_url,
        target_path: format!("catalog/{platform}/{id}.toml"),
    })
}

// --- M5: Tap management Tauri commands ---

#[derive(Serialize, Clone)]
pub struct TapInfo {
    pub id: String,
    pub source: String,
    pub priority: Option<u32>,
    pub cache_ok: bool,
    pub entry_count: usize,
    pub is_local: bool,
}

/// List all subscribed taps plus the implicit local tap.
#[tauri::command]
pub fn list_taps() -> Vec<TapInfo> {
    let mut result = vec![TapInfo {
        id: "local".into(),
        source: "(your custom games)".into(),
        priority: None,
        cache_ok: true,
        entry_count: crate::tap::load_user_tap(&crate::paths::user_tap_dir())
            .map(|t| t.entries.len())
            .unwrap_or(0),
        is_local: true,
    }];
    let manifest = crate::subscriptions::SubscriptionManifest::load_or_empty(
        &crate::paths::subscriptions_path(),
    )
    .unwrap_or_else(|_| crate::subscriptions::SubscriptionManifest::empty());
    for sub in &manifest.taps {
        let cache_dir = crate::paths::tap_cache_dir_for(&sub.id);
        let cache_ok = cache_dir.join("tap.toml").exists();
        let entry_count = if cache_ok {
            crate::tap::load_tap(&cache_dir)
                .map(|t| t.entries.len())
                .unwrap_or(0)
        } else {
            0
        };
        result.push(TapInfo {
            id: sub.id.clone(),
            source: sub.source.clone(),
            priority: Some(sub.priority),
            cache_ok,
            entry_count,
            is_local: false,
        });
    }
    result
}

/// Subscribe to a tap by short name or URL. Runs synchronously (may be slow on first add).
#[tauri::command]
pub fn add_tap(name_or_url: String, priority: Option<u32>) -> Result<TapInfo, String> {
    use crate::known_taps::resolve_tap_source;
    use crate::tap::validate_tap_dir;
    use crate::tap_fetch::clone_tap;

    crate::tap_fetch::check_git().map_err(|e| e.to_string())?;

    let source = resolve_tap_source(&name_or_url).to_string();
    let sub_path = crate::paths::subscriptions_path();
    let mut manifest = crate::subscriptions::SubscriptionManifest::load_or_empty(&sub_path)
        .map_err(|e| e.to_string())?;

    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    clone_tap(&source, tmp.path()).map_err(|e| e.to_string())?;
    let meta = validate_tap_dir(tmp.path(), None).map_err(|e| e.to_string())?;

    if manifest.taps.iter().any(|t| t.id == meta.id) {
        return Err(format!("already subscribed to {:?}", meta.id));
    }

    let cache_dest = crate::paths::tap_cache_dir_for(&meta.id);
    if let Some(p) = cache_dest.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::rename(tmp.path(), &cache_dest)
        .or_else(|_| copy_dir_recursive(tmp.path(), &cache_dest))
        .map_err(|e| format!("failed to move tap cache: {e}"))?;

    let p = priority.unwrap_or_else(|| manifest.next_priority());
    let added_at = toml::value::Datetime::from_str(&crate::install_record::now_iso8601())
        .expect("now_iso8601 always produces valid datetime");

    manifest.taps.push(crate::subscriptions::TapSubscription {
        id: meta.id.clone(),
        source: source.clone(),
        added_at,
        priority: p,
    });
    manifest.write(&sub_path).map_err(|e| e.to_string())?;

    let entry_count = crate::tap::load_tap(&cache_dest)
        .map(|t| t.entries.len())
        .unwrap_or(0);
    Ok(TapInfo {
        id: meta.id,
        source,
        priority: Some(p),
        cache_ok: true,
        entry_count,
        is_local: false,
    })
}

/// Unsubscribe from a tap and remove its cache.
#[tauri::command]
pub fn remove_tap(id: String) -> Result<(), String> {
    if id == "local" {
        return Err("'local' is the user tap and cannot be removed via this command".into());
    }
    let sub_path = crate::paths::subscriptions_path();
    let mut manifest = crate::subscriptions::SubscriptionManifest::load_or_empty(&sub_path)
        .map_err(|e| e.to_string())?;
    if !manifest.taps.iter().any(|t| t.id == id) {
        return Err(format!("not subscribed to {id:?}"));
    }
    manifest.taps.retain(|t| t.id != id);
    manifest.write(&sub_path).map_err(|e| e.to_string())?;
    let cache = crate::paths::tap_cache_dir_for(&id);
    if cache.exists() {
        std::fs::remove_dir_all(&cache)
            .unwrap_or_else(|e| tracing::warn!("could not remove tap cache: {e}"));
    }
    Ok(())
}

/// Pull latest commits for one tap (by id) or all subscribed taps (id = None).
#[tauri::command]
pub fn sync_tap(id: Option<String>) -> Result<Vec<String>, String> {
    let manifest = crate::subscriptions::SubscriptionManifest::load_or_empty(
        &crate::paths::subscriptions_path(),
    )
    .unwrap_or_else(|_| crate::subscriptions::SubscriptionManifest::empty());

    let taps_to_sync: Vec<_> = manifest
        .taps
        .iter()
        .filter(|t| id.as_deref().is_none_or(|needle| t.id == needle))
        .collect();

    let mut messages = Vec::new();
    let mut had_error = false;
    for sub in taps_to_sync {
        let cache = crate::paths::tap_cache_dir_for(&sub.id);
        if !cache.exists() {
            messages.push(format!(
                "{}: cache missing — use 'tap remove' then 'tap add' to reset",
                sub.id
            ));
            had_error = true;
            continue;
        }
        match crate::tap_fetch::pull_tap(&cache) {
            Ok(r) if r.already_up_to_date => messages.push(format!("{}: up to date", sub.id)),
            Ok(r) => messages.push(format!(
                "{}: updated {} -> {}",
                sub.id,
                &r.before_hash[..7.min(r.before_hash.len())],
                &r.after_hash[..7.min(r.after_hash.len())]
            )),
            Err(e) => {
                messages.push(format!("{}: error — {e}", sub.id));
                had_error = true;
            }
        }
    }
    if had_error {
        Err(messages.join("\n"))
    } else {
        Ok(messages)
    }
}

/// Pure core of [`reorder_tap`]: set `id`'s priority in `manifest`, rejecting
/// `local`, unknown ids, and duplicate priorities.
fn set_tap_priority(
    manifest: &mut crate::subscriptions::SubscriptionManifest,
    id: &str,
    priority: u32,
) -> Result<(), String> {
    if id == "local" {
        return Err("the local tap always wins and has no editable priority".into());
    }
    if !manifest.taps.iter().any(|t| t.id == id) {
        return Err(format!("not subscribed to {id:?}"));
    }
    if let Some(c) = manifest
        .taps
        .iter()
        .find(|t| t.id != id && t.priority == priority)
    {
        return Err(format!("priority {priority} is already used by {:?}", c.id));
    }
    for t in &mut manifest.taps {
        if t.id == id {
            t.priority = priority;
        }
    }
    Ok(())
}

/// Pure core of [`make_tap_default`]: renumber priorities so `id` sorts first
/// (priority 0), preserving the relative order of the other taps. `local` is a
/// no-op (it already wins). Returns whether a change should be persisted.
fn promote_tap_to_default(
    manifest: &mut crate::subscriptions::SubscriptionManifest,
    id: &str,
) -> Result<bool, String> {
    if id == "local" {
        return Ok(false);
    }
    if !manifest.taps.iter().any(|t| t.id == id) {
        return Err(format!("{id:?} is not a subscribed tap"));
    }
    let mut order: Vec<String> = vec![id.to_string()];
    let mut others: Vec<&crate::subscriptions::TapSubscription> =
        manifest.taps.iter().filter(|t| t.id != id).collect();
    others.sort_by_key(|t| t.priority);
    order.extend(others.into_iter().map(|t| t.id.clone()));

    for (rank, tid) in order.iter().enumerate() {
        for t in &mut manifest.taps {
            if &t.id == tid {
                t.priority = rank as u32;
            }
        }
    }
    Ok(true)
}

/// Set a subscribed tap's priority (lower wins). Rejects `local` (its
/// precedence is fixed) and duplicate priorities.
#[tauri::command]
pub fn reorder_tap(id: String, priority: u32) -> Result<(), String> {
    let sub_path = crate::paths::subscriptions_path();
    let mut manifest = crate::subscriptions::SubscriptionManifest::load_or_empty(&sub_path)
        .map_err(|e| e.to_string())?;
    set_tap_priority(&mut manifest, &id, priority)?;
    manifest.write(&sub_path).map_err(|e| e.to_string())
}

/// Make `id` win conflict resolution among subscribed taps. Backs the detail
/// view's "Make this version the default" button.
#[tauri::command]
pub fn make_tap_default(id: String) -> Result<(), String> {
    let sub_path = crate::paths::subscriptions_path();
    let mut manifest = crate::subscriptions::SubscriptionManifest::load_or_empty(&sub_path)
        .map_err(|e| e.to_string())?;
    if promote_tap_to_default(&mut manifest, &id)? {
        manifest.write(&sub_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

use std::str::FromStr;

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_tap_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tap")
    }

    /// Seed the fixture catalog as a subscribed `reliquaint-core` tap under
    /// `tmp`. Returns (subscriptions_path, taps_cache_dir, user_tap_dir) for
    /// passing to [`assemble_catalog_view`].
    fn seed_core_subscription(tmp: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let taps_cache = tmp.join("taps");
        copy_dir_recursive(&fixture_tap_dir(), &taps_cache.join("reliquaint-core")).unwrap();
        let subs = tmp.join("subscriptions.toml");
        std::fs::write(
            &subs,
            "schema_version = 1\n\n[[tap]]\nid = \"reliquaint-core\"\nsource = \"file:///x\"\nadded_at = 2026-01-01T00:00:00Z\npriority = 0\n",
        )
        .unwrap();
        (subs, taps_cache, tmp.join("user-tap"))
    }

    #[test]
    fn load_catalog_view_finds_fixture_tap_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let installs = tempfile::tempdir().unwrap();
        let (subs, cache, user_tap) = seed_core_subscription(tmp.path());
        let view = assemble_catalog_view(installs.path(), &subs, &cache, &user_tap).unwrap();
        let ids: Vec<&str> = view
            .all()
            .iter()
            .map(|e| e.catalog.game.id.as_str())
            .collect();
        assert!(ids.contains(&"qfg1-ega"), "expected qfg1-ega in {ids:?}");
        assert!(ids.contains(&"fatman"), "expected fatman in {ids:?}");
    }

    fn manifest_with(taps: &[(&str, u32)]) -> crate::subscriptions::SubscriptionManifest {
        let mut m = crate::subscriptions::SubscriptionManifest::empty();
        for (id, priority) in taps {
            m.taps.push(crate::subscriptions::TapSubscription {
                id: (*id).to_string(),
                source: "file:///x".to_string(),
                added_at: toml::value::Datetime::from_str("2026-01-01T00:00:00Z").unwrap(),
                priority: *priority,
            });
        }
        m
    }

    fn priority_of(m: &crate::subscriptions::SubscriptionManifest, id: &str) -> u32 {
        m.taps.iter().find(|t| t.id == id).unwrap().priority
    }

    #[test]
    fn set_tap_priority_updates_and_rejects_conflicts() {
        let mut m = manifest_with(&[("a", 0), ("b", 1)]);
        set_tap_priority(&mut m, "a", 5).unwrap();
        assert_eq!(priority_of(&m, "a"), 5);
        // b already holds 1.
        assert!(set_tap_priority(&mut m, "a", 1).is_err());
        // local is not editable.
        assert!(set_tap_priority(&mut m, "local", 0).is_err());
        // unknown id.
        assert!(set_tap_priority(&mut m, "nope", 9).is_err());
    }

    #[test]
    fn promote_tap_to_default_makes_target_win() {
        let mut m = manifest_with(&[("a", 0), ("b", 1), ("c", 2)]);
        let changed = promote_tap_to_default(&mut m, "c").unwrap();
        assert!(changed);
        assert_eq!(priority_of(&m, "c"), 0, "target should win");
        // Others keep their relative order below the winner.
        assert!(priority_of(&m, "a") < priority_of(&m, "b"));
        assert!(priority_of(&m, "a") > 0 && priority_of(&m, "b") > 0);
        // Priorities remain unique.
        let mut seen = std::collections::HashSet::new();
        assert!(m.taps.iter().all(|t| seen.insert(t.priority)));
    }

    #[test]
    fn promote_tap_to_default_local_is_noop() {
        let mut m = manifest_with(&[("a", 0)]);
        assert!(!promote_tap_to_default(&mut m, "local").unwrap());
        assert!(promote_tap_to_default(&mut m, "missing").is_err());
    }

    #[test]
    fn entry_to_dto_carries_metadata_and_acquisition() {
        // Empty installs dir → qfg1-ega is deterministically "not installed".
        let tmp = tempfile::tempdir().unwrap();
        let installs = tempfile::tempdir().unwrap();
        let (subs, cache, user_tap) = seed_core_subscription(tmp.path());
        let view = assemble_catalog_view(installs.path(), &subs, &cache, &user_tap).unwrap();
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
        // Every source is an empty temp dir, so the view is hermetically
        // empty regardless of the developer's real config/installs/taps.
        let empty = tempfile::tempdir().unwrap();
        let installs = tempfile::tempdir().unwrap();
        let view = assemble_catalog_view(
            installs.path(),
            &empty.path().join("subscriptions.toml"),
            &empty.path().join("taps"),
            &empty.path().join("user-tap"),
        )
        .unwrap();
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
