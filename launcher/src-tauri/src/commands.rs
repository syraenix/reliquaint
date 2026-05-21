use crate::discovery::{discover, find_by_id};
use crate::doctor::{run_all, ProbeKind, ProbeStatus};
use crate::installer::run_install;
use crate::runner::{run, RunOpts};
use crate::setup::{action_for, build_commands, detect_distro};
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize, Clone)]
pub struct GameEntry {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub collection: String,
    pub artwork_path: Option<String>,
}

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

pub fn games_from_repo(repo_root: &Path) -> Vec<GameEntry> {
    let mut games = discover(repo_root);
    games.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
    games
        .into_iter()
        .map(|(path, m)| {
            let artwork_path = m
                .ui
                .as_ref()
                .and_then(|ui| ui.artwork.as_deref())
                .map(|rel| {
                    let base = path.parent().unwrap_or(Path::new("."));
                    base.join(rel).to_string_lossy().into_owned()
                });
            GameEntry {
                id: m.id,
                title: m.title,
                platform: format!("{:?}", m.platform).to_lowercase(),
                collection: m.collection,
                artwork_path,
            }
        })
        .collect()
}

pub fn doctor_from_repo(repo_root: &Path) -> Vec<DoctorResult> {
    run_all(repo_root)
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
        .collect()
}

#[tauri::command]
pub fn list_games(state: State<'_, AppState>) -> Result<Vec<GameEntry>, String> {
    Ok(games_from_repo(&state.repo_root))
}

#[tauri::command]
pub async fn launch_game(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let repo_root = state.repo_root.clone();
    let (path, manifest) = find_by_id(&repo_root, &id)
        .ok_or_else(|| format!("no manifest found for id '{id}'"))?;
    let opts = RunOpts {
        dry_run: false,
        windowed: false,
    };

    tauri::async_runtime::spawn_blocking(move || {
        run(&path, &manifest, &repo_root, &opts)
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn run_doctor(state: State<'_, AppState>) -> Vec<DoctorResult> {
    doctor_from_repo(&state.repo_root)
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
