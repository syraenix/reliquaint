use crate::discovery::{discover_catalog, find_by_id};
use crate::doctor::{run_all, ProbeKind, ProbeStatus};
use crate::game_install::{
    build_kq_commands, build_qfg_commands, discover_qfg, install_order, make_kq_entry,
    validate_collection_membership, Collection, InstallerEntry,
};
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
    let mut games = discover_catalog(repo_root);
    games.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
    games
        .into_iter()
        .map(|(path, m)| {
            let artwork_path: Option<String> = {
                let _ = &path; // TODO(task8): wire artwork detection from discovery
                None
            };
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

#[derive(Serialize, Clone)]
struct GameInstallStartedPayload {
    game_id: String,
}

#[derive(Serialize, Clone)]
struct GameInstallOutputPayload {
    game_id: String,
    line: String,
    stream: String,
}

#[derive(Serialize, Clone)]
struct GameInstallFinishedPayload {
    game_id: String,
    exit_code: i32,
}

#[derive(Serialize, Clone)]
struct GameInstallAbortedPayload {
    game_id: String,
    exit_code: i32,
}

#[tauri::command]
pub fn default_installers_dir(
    collection: String,
    state: State<'_, AppState>,
) -> Option<String> {
    match Collection::parse(&collection) {
        Some(Collection::QuestForGlory) => {
            let p = state.repo_root.join("dos/quest-for-glory/installers");
            p.is_dir().then(|| p.to_string_lossy().into_owned())
        }
        _ => None,
    }
}

#[tauri::command]
pub fn discover_qfg_installers(directory: String) -> Result<Vec<InstallerEntry>, String> {
    let dir = PathBuf::from(&directory);
    if !dir.is_dir() {
        return Err(format!("not a directory: {directory}"));
    }
    Ok(discover_qfg(&dir))
}

#[tauri::command]
pub fn build_kq_entry(game_id: String, directory: String) -> Result<InstallerEntry, String> {
    let dir = PathBuf::from(&directory);
    make_kq_entry(&game_id, &dir)
}

#[tauri::command]
pub async fn install_games(
    collection: String,
    entries: Vec<InstallerEntry>,
    app: AppHandle,
) -> Result<i32, String> {
    let parsed = Collection::parse(&collection)
        .ok_or_else(|| format!("unknown collection: {collection}"))?;
    validate_collection_membership(parsed, &entries)?;
    if entries.is_empty() {
        return Err("no entries to install".to_string());
    }

    let ordered = install_order(entries);

    let app_for_task = app.clone();
    let exit_code = tauri::async_runtime::spawn_blocking(move || -> Result<i32, String> {
        for entry in ordered {
            let cmds = match parsed {
                Collection::QuestForGlory => build_qfg_commands(&entry),
                Collection::KingsQuest => build_kq_commands(&entry),
            };

            let _ = app_for_task.emit(
                "game-install-started",
                GameInstallStartedPayload {
                    game_id: entry.game_id.clone(),
                },
            );

            let game_id_for_lines = entry.game_id.clone();
            let app_for_lines = app_for_task.clone();
            let game_exit = run_install(cmds, move |line, is_err| {
                let payload = GameInstallOutputPayload {
                    game_id: game_id_for_lines.clone(),
                    line: line.to_string(),
                    stream: if is_err { "stderr".into() } else { "stdout".into() },
                };
                let _ = app_for_lines.emit("game-install-output", payload);
            })?;

            let _ = app_for_task.emit(
                "game-install-finished",
                GameInstallFinishedPayload {
                    game_id: entry.game_id.clone(),
                    exit_code: game_exit,
                },
            );

            if game_exit != 0 {
                let _ = app_for_task.emit(
                    "game-install-aborted",
                    GameInstallAbortedPayload {
                        game_id: entry.game_id.clone(),
                        exit_code: game_exit,
                    },
                );
                return Ok(game_exit);
            }
        }
        Ok(0)
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(exit_code)
}
