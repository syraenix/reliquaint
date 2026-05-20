use crate::discovery::{discover, find_by_id};
use crate::doctor::{run_all, ProbeStatus};
use crate::runner::{run, RunOpts};
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tauri::State;

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
