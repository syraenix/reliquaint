# Library Rework + Unified Install — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace manifest-driven installed-state tracking with filesystem scanning of `~/games`, unify install flows, and add Amiga `.hdf` support.

**Architecture:** `~/games/{id}/` becomes the canonical install location. `discover_installed(repo_root, games_base)` scans that directory and cross-references manifests for launch config. Manifests drop `[install]` and `[ui]` sections entirely. The game grid shows only installed games; a new `CatalogPanel.svelte` provides the "Add game" entry point.

**Tech Stack:** Rust (Tauri backend), Svelte 4 (frontend), Tauri 2, Cargo test suite, tempfile crate for tests.

**Spec:** `docs/superpowers/specs/2026-05-21-library-rework-design.md`

---

### Task 1: Add `paths::games_dir`

**Files:**
- Modify: `launcher/src-tauri/src/paths.rs`

- [ ] **Write the failing test**

```rust
#[test]
fn games_dir_joins_base_and_id() {
    let base = Path::new("/home/user/games");
    let result = games_dir(base, "qfg1-ega");
    assert_eq!(result, PathBuf::from("/home/user/games/qfg1-ega"));
}

#[test]
fn games_dir_with_tilde_base() {
    let base = expand_tilde("~/games");
    let result = games_dir(&base, "kq1sci");
    let home = std::env::var("HOME").unwrap_or_default();
    assert!(result.starts_with(&home));
    assert!(result.ends_with("games/kq1sci"));
}
```

- [ ] **Run tests to confirm they fail**

```bash
cd launcher && cargo test paths::tests -- --nocapture 2>&1 | grep -E "FAILED|error"
```

- [ ] **Add the implementation** (after the existing `resolve_relative` fn in `paths.rs`)

```rust
pub fn games_dir(base: &Path, id: &str) -> PathBuf {
    base.join(id)
}
```

- [ ] **Run tests to confirm they pass**

```bash
cd launcher && cargo test paths::tests
```

Expected: all `paths::tests` pass.

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/paths.rs
git commit -m "feat(paths): add games_dir helper"
```

---

### Task 2: Update `manifest.rs` — drop `Install`, `Ui`, remove stale validation

**Files:**
- Modify: `launcher/src-tauri/src/manifest.rs`

- [ ] **Update the inline test TOML constants** (remove `[install]` and `[ui]` blocks from test strings)

Replace `DOS_MANIFEST` constant:

```rust
const DOS_MANIFEST: &str = r#"
id         = "qfg1-ega"
title      = "Quest for Glory 1 (EGA)"
platform   = "dos"
collection = "quest-for-glory"

[runtime]
emulator = "dosbox-staging"
config   = "../config/qfg1-ega.conf"
sidecars = ["fluidsynth"]
"#;
```

Replace `sidecars_defaults_to_empty` test string:

```rust
let src = r#"
id = "kq1sci"
title = "King's Quest 1"
platform = "dos"
collection = "kings-quest"
[runtime]
emulator = "dosbox-staging"
config = "../config/kq1sci.conf"
"#;
```

- [ ] **Replace the test `ui_section_is_optional` with a test that `[ui]` as unknown field now errors** and **replace `dos_missing_install_errors` with a test that missing `[install]` no longer errors**:

```rust
#[test]
fn dos_without_install_section_is_valid() {
    let src = r#"
id = "x"
title = "X"
platform = "dos"
collection = "c"
[runtime]
emulator = "dosbox-staging"
config = "../config/x.conf"
"#;
    assert!(parse_str(src).is_ok());
}

#[test]
fn amiga_without_file_field_is_valid() {
    let src = r#"
id = "x"
title = "X"
platform = "amiga"
collection = "c"
[runtime]
emulator = "fs-uae"
model = "a500"
"#;
    assert!(parse_str(src).is_ok());
}
```

Also update `dos_manifest_parses` to remove the `expects_dir` assertion:

```rust
#[test]
fn dos_manifest_parses() {
    let m = parse_str(DOS_MANIFEST).unwrap();
    assert_eq!(m.id, "qfg1-ega");
    assert_eq!(m.platform, Platform::Dos);
    assert_eq!(m.runtime.config.as_deref(), Some("../config/qfg1-ega.conf"));
    assert_eq!(m.runtime.sidecars, vec!["fluidsynth"]);
}
```

Delete tests: `ui_section_is_optional`, `dos_missing_install_errors`.

- [ ] **Remove `Install` and `Ui` structs, `install`/`ui` fields from `Manifest`, and stale errors/validation**

New `manifest.rs` structs section (replace existing):

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub emulator: String,
    pub config: Option<String>,
    #[serde(default)]
    pub sidecars: Vec<String>,
    pub file: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub title: String,
    pub platform: Platform,
    pub collection: String,
    pub runtime: Runtime,
}
```

Remove from `ManifestError`:
- `DosInstallMissing`
- `AmigaFileMissing`

New `validate`:

```rust
fn validate(m: &Manifest) -> Result<(), ManifestError> {
    match m.platform {
        Platform::Dos => {
            if m.runtime.config.is_none() {
                return Err(ManifestError::DosConfigMissing);
            }
        }
        Platform::Amiga => {
            if !m.runtime.sidecars.is_empty() {
                return Err(ManifestError::AmigaSidecarsForbidden);
            }
        }
    }
    Ok(())
}
```

- [ ] **Run tests**

```bash
cd launcher && cargo test manifest::tests
```

Expected: all pass.

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/manifest.rs
git commit -m "feat(manifest): drop Install, Ui; remove expects_dir validation"
```

---

### Task 3: Update all manifest TOML files and test fixtures

**Files:**
- Modify: `dos/quest-for-glory/manifests/qfg1-ega.toml`, `qfg1-vga.toml`, `qfg2.toml`, `qfg3.toml`, `qfg4.toml`
- Modify: `dos/kings-quest/manifests/kq1sci.toml`, `kq2.toml`, `kq3.toml`, `kq4.toml`, `kq5.toml`, `kq6.toml`
- Modify: `launcher/src-tauri/tests/fixtures/dos/quest-for-glory/manifests/qfg1-ega.toml`
- Modify: `launcher/src-tauri/tests/fixtures/dos/kings-quest/manifests/kq1sci.toml`

- [ ] **Remove `[install]` section from every DOS manifest** (repo + fixtures). Example result for each file:

```toml
id         = "qfg1-ega"
title      = "Quest for Glory 1 (EGA)"
platform   = "dos"
collection = "quest-for-glory"

[runtime]
emulator = "dosbox-staging"
config   = "../config/qfg1-ega.conf"
sidecars = ["fluidsynth"]
```

(KQ manifests have the same shape — just `id`, `title`, `platform`, `collection`, `[runtime]`.)

- [ ] **Run the full cargo test suite**

```bash
cd launcher && cargo test
```

Expected: all tests pass (the suite parses every fixture manifest).

- [ ] **Commit**

```bash
git add dos/quest-for-glory/manifests/ dos/kings-quest/manifests/ \
        launcher/src-tauri/tests/fixtures/
git commit -m "chore(manifests): remove [install] expects_dir from all manifests"
```

---

### Task 4: Update `discovery.rs` — rename, add `InstalledEntry`/`discover_installed`/`find_artwork`

**Files:**
- Modify: `launcher/src-tauri/src/discovery.rs`

- [ ] **Write failing tests** (add at bottom of the test module)

```rust
#[test]
fn discover_installed_returns_games_in_games_base() {
    let root = fixture_root();
    let temp = tempfile::tempdir().unwrap();
    // qfg1-ega dir exists and is non-empty
    let qfg_dir = temp.path().join("qfg1-ega");
    std::fs::create_dir(&qfg_dir).unwrap();
    std::fs::write(qfg_dir.join("QUEST.EXE"), b"").unwrap();

    let installed = discover_installed(&root, temp.path());
    let ids: Vec<&str> = installed.iter().map(|e| e.manifest.id.as_str()).collect();
    assert!(ids.contains(&"qfg1-ega"));
}

#[test]
fn discover_installed_ignores_empty_dirs() {
    let root = fixture_root();
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("qfg1-ega")).unwrap(); // empty

    let installed = discover_installed(&root, temp.path());
    assert!(installed.is_empty());
}

#[test]
fn discover_installed_ignores_unknown_dirs() {
    let root = fixture_root();
    let temp = tempfile::tempdir().unwrap();
    let unknown = temp.path().join("mystery-game");
    std::fs::create_dir(&unknown).unwrap();
    std::fs::write(unknown.join("game.exe"), b"").unwrap();

    let installed = discover_installed(&root, temp.path());
    assert!(installed.is_empty());
}

#[test]
fn find_artwork_prefers_named_files() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("random.png"), b"").unwrap();
    std::fs::write(temp.path().join("cover.jpg"), b"").unwrap();
    let result = find_artwork(temp.path());
    assert!(result.unwrap().ends_with("cover.jpg"));
}

#[test]
fn find_artwork_falls_back_to_any_image() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("splash.bmp"), b"").unwrap();
    let result = find_artwork(temp.path());
    assert!(result.is_some());
}

#[test]
fn find_artwork_returns_none_when_no_images() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("game.exe"), b"").unwrap();
    assert!(find_artwork(temp.path()).is_none());
}
```

- [ ] **Run to confirm they fail**

```bash
cd launcher && cargo test discovery::tests 2>&1 | grep -E "FAILED|error\[E"
```

- [ ] **Rename `discover()` → `discover_catalog()` and update the internal call in `find_by_id`**

```rust
pub fn discover_catalog(repo_root: &Path) -> Vec<(PathBuf, Manifest)> {
    // ... same body as the old `discover` ...
}

pub fn find_by_id(repo_root: &Path, id: &str) -> Option<(PathBuf, Manifest)> {
    discover_catalog(repo_root).into_iter().find(|(_, m)| m.id == id)
}
```

- [ ] **Add `find_artwork`, `InstalledEntry`, and `discover_installed`**

```rust
pub fn find_artwork(game_dir: &Path) -> Option<PathBuf> {
    let priority_stems = ["cover", "icon", "box"];
    let image_exts = ["png", "jpg", "jpeg", "bmp"];
    for stem in &priority_stems {
        for ext in &image_exts {
            let p = game_dir.join(format!("{stem}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if matches!(ext_lower.as_str(), "png" | "jpg" | "jpeg" | "bmp") {
                    return Some(path);
                }
            }
        }
    }
    None
}

pub struct InstalledEntry {
    pub manifest_path: PathBuf,
    pub manifest: Manifest,
    pub install_dir: PathBuf,
    pub artwork_path: Option<PathBuf>,
}

pub fn discover_installed(repo_root: &Path, games_base: &Path) -> Vec<InstalledEntry> {
    let catalog: std::collections::HashMap<String, (PathBuf, Manifest)> =
        discover_catalog(repo_root)
            .into_iter()
            .map(|(p, m)| (m.id.clone(), (p, m)))
            .collect();

    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(games_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let Some((manifest_path, manifest)) = catalog.get(&dir_name).cloned() else {
                continue;
            };
            let non_empty = std::fs::read_dir(&path)
                .ok()
                .and_then(|mut d| d.next())
                .is_some();
            if !non_empty {
                continue;
            }
            let artwork_path = find_artwork(&path);
            results.push(InstalledEntry { manifest_path, manifest, install_dir: path, artwork_path });
        }
    }
    results.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    results
}
```

- [ ] **Update existing tests**: rename `discover_finds_fixture_manifests` → `discover_catalog_finds_fixture_manifests` and update the call.

```rust
#[test]
fn discover_catalog_finds_fixture_manifests() {
    let root = fixture_root();
    let mut manifests = discover_catalog(&root);
    manifests.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
    let ids: Vec<&str> = manifests.iter().map(|(_, m)| m.id.as_str()).collect();
    assert_eq!(ids, vec!["kq1sci", "qfg1-ega"]);
}
```

Also rename and update `discover_skips_disabled_and_gitkeep` and `find_by_id_*` tests to call `discover_catalog`.

- [ ] **Run tests**

```bash
cd launcher && cargo test discovery::tests
```

Expected: all pass.

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/discovery.rs
git commit -m "feat(discovery): scan ~/games for installed state, add artwork detection"
```

---

### Task 5: Update `doctor.rs` — use `games_dir`, probe all games

**Files:**
- Modify: `launcher/src-tauri/src/doctor.rs`

- [ ] **Update `run_all` signature and implementation**

Change:
```rust
pub fn run_all(repo_root: &Path) -> Vec<ProbeResult> {
```
To:
```rust
pub fn run_all(repo_root: &Path, games_base: &Path) -> Vec<ProbeResult> {
```

Replace the loop over manifests at the end of `run_all`:

```rust
for (_, manifest) in crate::discovery::discover_catalog(repo_root) {
    let dir = crate::paths::games_dir(games_base, &manifest.id);
    let status = if dir.exists() {
        ProbeStatus::Ok
    } else {
        ProbeStatus::Missing
    };
    results.push(ProbeResult {
        name: format!("{} install dir", manifest.id),
        status,
        detail: Some(dir.display().to_string()),
        kind: ProbeKind::GameInstallDir(manifest.id.clone()),
    });
}
```

Also remove the `use crate::manifest::Platform;` import and the `use crate::paths::expand_tilde;` import (replaced by `games_dir`). Add `use crate::paths::games_dir;`.

- [ ] **Update tests** — update `run_all` calls in tests to pass a `games_base`:

```rust
#[test]
fn run_all_includes_host_probes() {
    let root = fixture_root();
    let games_base = tempfile::tempdir().unwrap();
    let results = run_all(&root, games_base.path());
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"dosbox-staging (flatpak)"));
    assert!(names.contains(&"fluidsynth"));
    assert!(names.contains(&"soundfont"));
    assert!(names.contains(&"innoextract"));
    assert!(names.contains(&"fs-uae"));
    assert!(names.contains(&"unzip"));
}

#[test]
fn run_all_includes_manifest_dir_checks() {
    let root = fixture_root();
    let games_base = tempfile::tempdir().unwrap();
    let results = run_all(&root, games_base.path());
    assert!(results.iter().any(|r| r.name.contains("install dir")));
}

#[test]
fn host_probes_carry_expected_kinds() {
    let root = fixture_root();
    let games_base = tempfile::tempdir().unwrap();
    let results = run_all(&root, games_base.path());
    // ... same assertions, just pass games_base.path()
}

#[test]
fn install_dir_probe_kind_carries_id() {
    let root = fixture_root();
    let games_base = tempfile::tempdir().unwrap();
    let results = run_all(&root, games_base.path());
    // ... same assertions
}
```

- [ ] **Run tests**

```bash
cd launcher && cargo test doctor::tests
```

Expected: all pass. (Other modules that call `run_all` will have compile errors — fix those in later tasks.)

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/doctor.rs
git commit -m "feat(doctor): probe ~/games/{id} for all games, drop expects_dir"
```

---

### Task 6: Update `runner.rs` — HDF support, scan `~/games/{id}` for Amiga file

**Files:**
- Modify: `launcher/src-tauri/src/runner.rs`

- [ ] **Write failing tests**

```rust
#[test]
fn amiga_hdf_command_structure() {
    let hdf = Path::new("/games/workbench.hdf");
    let model = Path::new("/repo/amiga/config/a500.fs-uae");
    let cmd = build_amiga_hdf_command(hdf, model, false);
    assert_eq!(cmd.get_program(), "fs-uae");
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(args[0], "/repo/amiga/config/a500.fs-uae");
    assert_eq!(args[1], "--hard_drive_0=/games/workbench.hdf");
    assert!(!args.contains(&"--fullscreen=0".to_string()));
}

#[test]
fn amiga_hdf_command_windowed() {
    let hdf = Path::new("/games/workbench.hdf");
    let model = Path::new("/repo/amiga/config/a500.fs-uae");
    let cmd = build_amiga_hdf_command(hdf, model, true);
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert!(args.contains(&"--fullscreen=0".to_string()));
}

#[test]
fn find_amiga_file_finds_adf() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("game.adf"), b"").unwrap();
    let result = find_amiga_file(temp.path());
    assert!(result.is_ok());
    assert!(result.unwrap().ends_with("game.adf"));
}

#[test]
fn find_amiga_file_finds_hdf() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("whdload.hdf"), b"").unwrap();
    let result = find_amiga_file(temp.path());
    assert!(result.is_ok());
}

#[test]
fn find_amiga_file_errors_when_empty() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("readme.txt"), b"").unwrap();
    assert!(find_amiga_file(temp.path()).is_err());
}
```

- [ ] **Run to confirm they fail**

```bash
cd launcher && cargo test runner::tests 2>&1 | grep -E "FAILED|error\[E"
```

- [ ] **Add `build_amiga_hdf_command` and `find_amiga_file`**

```rust
pub fn build_amiga_hdf_command(hdf_path: &Path, model_config: &Path, windowed: bool) -> Command {
    let mut cmd = Command::new("fs-uae");
    cmd.arg(model_config)
        .arg(format!("--hard_drive_0={}", hdf_path.display()));
    if windowed {
        cmd.arg("--fullscreen=0");
    }
    cmd
}

pub fn find_amiga_file(game_dir: &Path) -> anyhow::Result<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext.to_lowercase().as_str(), "adf" | "hdf" | "rp9") {
                    return Ok(path);
                }
            }
        }
    }
    Err(anyhow::anyhow!(
        "no Amiga file (.adf/.hdf/.rp9) found in {}",
        game_dir.display()
    ))
}
```

- [ ] **Update `run()` signature and `run_amiga` to scan `games_base/{id}/`**

New `run` signature:

```rust
pub fn run(
    manifest_path: &Path,
    manifest: &Manifest,
    repo_root: &Path,
    games_base: &Path,
    opts: &RunOpts,
) -> anyhow::Result<ExitCode> {
    match manifest.platform {
        Platform::Dos => run_dos(manifest_path, manifest, opts),
        Platform::Amiga => {
            let game_dir = crate::paths::games_dir(games_base, &manifest.id);
            run_amiga(manifest, &game_dir, repo_root, opts)
        }
    }
}
```

New `run_amiga` (replaces old):

```rust
fn run_amiga(
    manifest: &Manifest,
    game_dir: &Path,
    repo_root: &Path,
    opts: &RunOpts,
) -> anyhow::Result<ExitCode> {
    let model = manifest.runtime.model.as_deref().unwrap_or("a500");
    let model_config = resolve_amiga_model_config(repo_root, model)?;
    let file = find_amiga_file(game_dir)?;
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "adf" => {
            let mut cmd = build_amiga_adf_command(&file, &model_config, opts.windowed);
            if opts.dry_run {
                println!("[primary] {}", format_command(&cmd));
                return Ok(ExitCode::SUCCESS);
            }
            Ok(exit_code_from_status(
                cmd.status().map_err(|e| anyhow::anyhow!("failed to spawn fs-uae: {e}"))?,
            ))
        }
        "hdf" => {
            let mut cmd = build_amiga_hdf_command(&file, &model_config, opts.windowed);
            if opts.dry_run {
                println!("[primary] {}", format_command(&cmd));
                return Ok(ExitCode::SUCCESS);
            }
            Ok(exit_code_from_status(
                cmd.status().map_err(|e| anyhow::anyhow!("failed to spawn fs-uae: {e}"))?,
            ))
        }
        "rp9" => {
            let (mut cmd, _keep) =
                build_amiga_rp9_command(&file, &model_config, opts.windowed)?;
            if opts.dry_run {
                println!("[primary] {}", format_command(&cmd));
                return Ok(ExitCode::SUCCESS);
            }
            Ok(exit_code_from_status(
                cmd.status().map_err(|e| anyhow::anyhow!("failed to spawn fs-uae: {e}"))?,
            ))
        }
        other => Err(anyhow::anyhow!("unsupported Amiga file extension: .{other}")),
    }
}
```

- [ ] **Run tests**

```bash
cd launcher && cargo test runner::tests
```

Expected: all pass.

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/runner.rs
git commit -m "feat(runner): scan ~/games/{id} for Amiga file, add HDF support"
```

---

### Task 7: Add `game_install::build_amiga_copy_commands`

**Files:**
- Modify: `launcher/src-tauri/src/game_install.rs`

- [ ] **Write the failing test**

```rust
#[test]
fn build_amiga_copy_commands_creates_dir_and_copies() {
    let source = Path::new("/downloads/lemmings.adf");
    let target = Path::new("/home/user/games/lemmings");
    let cmds = build_amiga_copy_commands(source, target);
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0][0], "mkdir");
    assert!(cmds[0].contains(&"-p".to_string()));
    assert_eq!(cmds[0].last().unwrap(), "/home/user/games/lemmings");
    assert_eq!(cmds[1][0], "cp");
    assert!(cmds[1].contains(&"/downloads/lemmings.adf".to_string()));
    assert_eq!(cmds[1].last().unwrap(), "/home/user/games/lemmings");
}
```

- [ ] **Run to confirm failure**

```bash
cd launcher && cargo test game_install::tests::build_amiga 2>&1 | grep FAILED
```

- [ ] **Add the function** (after `build_kq_commands`)

```rust
pub fn build_amiga_copy_commands(source: &Path, target_dir: &Path) -> Vec<Vec<String>> {
    let target = target_dir.to_string_lossy().to_string();
    vec![
        vec!["mkdir".into(), "-p".into(), target.clone()],
        vec!["cp".into(), "--".into(), source.to_string_lossy().to_string(), target],
    ]
}
```

- [ ] **Run tests**

```bash
cd launcher && cargo test game_install::tests
```

Expected: all pass.

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/game_install.rs
git commit -m "feat(game_install): add Amiga file copy command builder"
```

---

### Task 8: Update `commands.rs` — `AppState`, `list_games`, add `list_catalog` and `install_amiga_game`

**Files:**
- Modify: `launcher/src-tauri/src/commands.rs`

- [ ] **Update `AppState` to include `games_base`**

```rust
pub struct AppState {
    pub repo_root: PathBuf,
    pub games_base: PathBuf,
}
```

- [ ] **Update imports at top of file**

```rust
use crate::discovery::{discover_catalog, discover_installed};
use crate::paths::games_dir;
```

Remove the old `use crate::discovery::{discover, find_by_id};` and replace with:

```rust
use crate::discovery::{discover_catalog, discover_installed, find_by_id};
```

- [ ] **Replace `games_from_repo` to use `discover_installed` and remove old artwork logic**

```rust
pub fn games_from_repo(repo_root: &Path, games_base: &Path) -> Vec<GameEntry> {
    discover_installed(repo_root, games_base)
        .into_iter()
        .map(|e| GameEntry {
            id: e.manifest.id,
            title: e.manifest.title,
            platform: format!("{:?}", e.manifest.platform).to_lowercase(),
            collection: e.manifest.collection,
            artwork_path: e.artwork_path.map(|p| p.to_string_lossy().into_owned()),
        })
        .collect()
}
```

- [ ] **Update `list_games` to pass `games_base`**

```rust
#[tauri::command]
pub fn list_games(state: State<'_, AppState>) -> Result<Vec<GameEntry>, String> {
    Ok(games_from_repo(&state.repo_root, &state.games_base))
}
```

- [ ] **Update `doctor_from_repo` and `run_doctor` to pass `games_base`**

```rust
pub fn doctor_from_repo(repo_root: &Path, games_base: &Path) -> Vec<DoctorResult> {
    run_all(repo_root, games_base)
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
pub fn run_doctor(state: State<'_, AppState>) -> Vec<DoctorResult> {
    doctor_from_repo(&state.repo_root, &state.games_base)
}
```

- [ ] **Update `launch_game` to pass `games_base`**

```rust
#[tauri::command]
pub async fn launch_game(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let repo_root = state.repo_root.clone();
    let games_base = state.games_base.clone();
    let (path, manifest) = find_by_id(&repo_root, &id)
        .ok_or_else(|| format!("no manifest found for id '{id}'"))?;
    let opts = RunOpts { dry_run: false, windowed: false };
    tauri::async_runtime::spawn_blocking(move || {
        run(&path, &manifest, &repo_root, &games_base, &opts)
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
```

- [ ] **Add `GameCatalogEntry`, `games_catalog` helper, and `list_catalog` command**

```rust
#[derive(Serialize, Clone)]
pub struct GameCatalogEntry {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub collection: String,
    pub installed: bool,
}

pub fn games_catalog(repo_root: &Path, games_base: &Path) -> Vec<GameCatalogEntry> {
    let installed_ids: std::collections::HashSet<String> =
        discover_installed(repo_root, games_base)
            .into_iter()
            .map(|e| e.manifest.id)
            .collect();

    let mut entries: Vec<GameCatalogEntry> = discover_catalog(repo_root)
        .into_iter()
        .map(|(_, m)| GameCatalogEntry {
            installed: installed_ids.contains(&m.id),
            id: m.id,
            title: m.title,
            platform: format!("{:?}", m.platform).to_lowercase(),
            collection: m.collection,
        })
        .collect();

    entries.sort_by(|a, b| a.id.cmp(&b.id));
    entries
}

#[tauri::command]
pub fn list_catalog(state: State<'_, AppState>) -> Result<Vec<GameCatalogEntry>, String> {
    Ok(games_catalog(&state.repo_root, &state.games_base))
}
```

- [ ] **Add `install_amiga_game` command** (after `install_games`)

```rust
#[tauri::command]
pub async fn install_amiga_game(
    game_id: String,
    source_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<i32, String> {
    let source = PathBuf::from(&source_path);
    if !source.is_file() {
        return Err(format!("source file not found: {source_path}"));
    }
    let target_dir = games_dir(&state.games_base, &game_id);
    let cmds = crate::game_install::build_amiga_copy_commands(&source, &target_dir);

    let _ = app.emit("game-install-started", GameInstallStartedPayload { game_id: game_id.clone() });

    let game_id_for_lines = game_id.clone();
    let app_for_lines = app.clone();
    let exit_code = tauri::async_runtime::spawn_blocking(move || {
        crate::installer::run_install(cmds, move |line, is_err| {
            let _ = app_for_lines.emit("game-install-output", GameInstallOutputPayload {
                game_id: game_id_for_lines.clone(),
                line: line.to_string(),
                stream: if is_err { "stderr".into() } else { "stdout".into() },
            });
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    let _ = app.emit("game-install-finished", GameInstallFinishedPayload { game_id: game_id.clone(), exit_code });
    Ok(exit_code)
}
```

- [ ] **Build to confirm it compiles**

```bash
cd launcher && cargo build 2>&1 | grep error
```

Expected: only errors from `gui.rs` and `cli.rs` (not yet updated — that's next).

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/commands.rs
git commit -m "feat(commands): library-only list_games, add list_catalog + install_amiga_game"
```

---

### Task 9: Update `gui.rs` — `AppState` init, register new commands

**Files:**
- Modify: `launcher/src-tauri/src/gui.rs`

- [ ] **Update `AppState` construction and command registration**

```rust
use crate::commands::AppState;
use crate::discovery::find_repo_root;
use crate::paths::expand_tilde;
use std::path::PathBuf;

pub fn run_gui() {
    let repo_root = resolve_repo_root().unwrap_or_else(|| {
        eprintln!("warning: cannot locate repo root; set CLASSIC_LAUNCHER_REPO_ROOT");
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
    if let Ok(r) = std::env::var("CLASSIC_LAUNCHER_REPO_ROOT") {
        return Some(PathBuf::from(r));
    }
    let cwd = std::env::current_dir().ok()?;
    find_repo_root(&cwd)
}

fn resolve_games_base() -> PathBuf {
    if let Ok(base) = std::env::var("CLASSIC_LAUNCHER_GAMES_DIR") {
        return PathBuf::from(base);
    }
    expand_tilde("~/games")
}
```

- [ ] **Build**

```bash
cd launcher && cargo build 2>&1 | grep error
```

Expected: only errors from `cli.rs`.

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/gui.rs
git commit -m "feat(gui): pass games_base to AppState, register list_catalog + install_amiga_game"
```

---

### Task 10: Update `cli.rs` — `resolve_games_base`, pass to runner and doctor; update integration tests

**Files:**
- Modify: `launcher/src-tauri/src/cli.rs`
- Modify: `launcher/src-tauri/tests/cli.rs`

- [ ] **Add `resolve_games_base` and update `cmd_list`, `cmd_run`, `cmd_doctor`**

```rust
use crate::discovery::{discover_installed, find_by_id, find_repo_root};
use crate::doctor::{run_all, ProbeStatus};
use crate::paths::expand_tilde;
use crate::runner::{run as launch, RunOpts};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// ... Cli/Commands structs unchanged ...

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let repo_root = match resolve_repo_root() {
        Some(r) => r,
        None => {
            eprintln!("error: cannot find repo root");
            return ExitCode::FAILURE;
        }
    };
    let games_base = resolve_games_base();
    match cli.command {
        Commands::List => cmd_list(&repo_root, &games_base),
        Commands::Run { id, dry_run, windowed } => cmd_run(&repo_root, &games_base, &id, dry_run, windowed),
        Commands::Doctor => cmd_doctor(&repo_root, &games_base),
    }
}

fn resolve_repo_root() -> Option<PathBuf> {
    if let Ok(r) = std::env::var("CLASSIC_LAUNCHER_REPO_ROOT") {
        return Some(PathBuf::from(r));
    }
    let cwd = std::env::current_dir().ok()?;
    find_repo_root(&cwd)
}

fn resolve_games_base() -> PathBuf {
    if let Ok(base) = std::env::var("CLASSIC_LAUNCHER_GAMES_DIR") {
        return PathBuf::from(base);
    }
    expand_tilde("~/games")
}

fn cmd_list(repo_root: &Path, games_base: &Path) -> ExitCode {
    let mut entries = discover_installed(repo_root, games_base);
    entries.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    for e in &entries {
        let platform = format!("{:?}", e.manifest.platform).to_lowercase();
        println!("{:<15}  {:<6}  {:<20}  {}", e.manifest.id, platform, e.manifest.collection, e.manifest.title);
    }
    ExitCode::SUCCESS
}

fn cmd_run(repo_root: &Path, games_base: &Path, id: &str, dry_run: bool, windowed: bool) -> ExitCode {
    match find_by_id(repo_root, id) {
        None => {
            eprintln!("error: no manifest found for id '{id}'");
            ExitCode::FAILURE
        }
        Some((path, manifest)) => {
            let opts = RunOpts { dry_run, windowed };
            match launch(&path, &manifest, repo_root, games_base, &opts) {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("error: {e:#}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn cmd_doctor(repo_root: &Path, games_base: &Path) -> ExitCode {
    let results = run_all(repo_root, games_base);
    let mut any_missing = false;
    for r in &results {
        let label = match r.status {
            ProbeStatus::Ok => "ok     ",
            ProbeStatus::Missing => "missing",
            ProbeStatus::Unknown => "unknown",
        };
        let detail = r.detail.as_deref().unwrap_or("");
        println!("[ {label} ] {}  {}", r.name, detail);
        if r.status == ProbeStatus::Missing {
            any_missing = true;
        }
    }
    if any_missing { ExitCode::from(2) } else { ExitCode::SUCCESS }
}
```

- [ ] **Update integration tests in `tests/cli.rs`**

Replace `list_shows_fixture_manifest_ids` and `list_output_is_sorted` with tests that provide a `CLASSIC_LAUNCHER_GAMES_DIR`:

```rust
fn launcher_with_games(games_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("classic-launcher").unwrap();
    cmd.env("CLASSIC_LAUNCHER_REPO_ROOT", fixture_root())
       .env("CLASSIC_LAUNCHER_GAMES_DIR", games_dir);
    cmd
}

#[test]
fn list_shows_installed_games() {
    let temp = tempfile::tempdir().unwrap();
    let qfg_dir = temp.path().join("qfg1-ega");
    std::fs::create_dir(&qfg_dir).unwrap();
    std::fs::write(qfg_dir.join("QUEST.EXE"), b"").unwrap();
    let kq_dir = temp.path().join("kq1sci");
    std::fs::create_dir(&kq_dir).unwrap();
    std::fs::write(kq_dir.join("SIERRA.EXE"), b"").unwrap();

    launcher_with_games(temp.path())
        .arg("list")
        .assert()
        .success()
        .stdout(contains("qfg1-ega"))
        .stdout(contains("kq1sci"));
}

#[test]
fn list_empty_when_no_games_installed() {
    let temp = tempfile::tempdir().unwrap();
    launcher_with_games(temp.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

#[test]
fn list_output_is_sorted() {
    let temp = tempfile::tempdir().unwrap();
    for id in ["qfg2", "kq1sci", "qfg1-ega"] {
        let d = temp.path().join(id);
        std::fs::create_dir(&d).unwrap();
        std::fs::write(d.join("game.exe"), b"").unwrap();
    }
    let output = launcher_with_games(temp.path()).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let ids: Vec<&str> = stdout.lines().filter(|l| !l.is_empty())
        .map(|l| l.split_whitespace().next().unwrap_or("")).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}
```

The `run_dry_run_*` tests can stay as-is — they use `launcher()` (no games dir) and test DOS games which don't need a games_base for dry-run. Update the `launcher()` helper to also set a temp games dir so dry-run tests don't accidentally use `~/games`:

```rust
fn launcher() -> Command {
    let mut cmd = Command::cargo_bin("classic-launcher").unwrap();
    cmd.env("CLASSIC_LAUNCHER_REPO_ROOT", fixture_root())
       .env("CLASSIC_LAUNCHER_GAMES_DIR", tempfile::tempdir().unwrap().path());
    cmd
}
```

Use a fixed non-existent path for the base helper (dry-run and doctor tests don't need real game files):

```rust
fn launcher() -> Command {
    let mut cmd = Command::cargo_bin("classic-launcher").unwrap();
    cmd.env("CLASSIC_LAUNCHER_REPO_ROOT", fixture_root())
       .env("CLASSIC_LAUNCHER_GAMES_DIR", "/tmp/classic-launcher-no-games");
    cmd
}
```

All existing `launcher().arg(...)` call sites remain unchanged.

- [ ] **Run the full suite**

```bash
cd launcher && cargo test
```

Expected: all tests pass.

- [ ] **Commit**

```bash
git add launcher/src-tauri/src/cli.rs launcher/src-tauri/tests/cli.rs
git commit -m "feat(cli): list/run/doctor use ~/games scanning via CLASSIC_LAUNCHER_GAMES_DIR"
```

---

### Task 11: Add `CatalogPanel.svelte`

**Files:**
- Create: `launcher/src/components/CatalogPanel.svelte`

- [ ] **Create the component**

```svelte
<script>
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  // catalog entries from list_catalog
  let catalog = [];
  let loading = true;
  let error = null;

  // which collection install is open: "quest-for-glory" | "kings-quest" | null
  let activeCollectionInstall = null;

  // QFG sub-install state
  let qfgDir = "";
  let qfgEntries = [];
  let qfgSelected = {};
  let qfgDiscoverError = null;

  // KQ sub-install state (game_id -> { entry, error })
  let kqState = {};

  // Shared install progress
  let installing = false;
  let log = [];
  let entryStatus = {};
  let installError = null;
  let logEl;

  let unlistenStarted, unlistenOutput, unlistenFinished, unlistenAborted;

  async function loadCatalog() {
    loading = true;
    error = null;
    try {
      catalog = await invoke("list_catalog");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $: uninstalledQfg = catalog.filter(g => !g.installed && g.collection === "quest-for-glory");
  $: uninstalledKq  = catalog.filter(g => !g.installed && g.collection === "kings-quest");
  $: uninstalledAmiga = catalog.filter(g => !g.installed && g.platform === "amiga");
  $: hasUninstalled = uninstalledQfg.length > 0 || uninstalledKq.length > 0 || uninstalledAmiga.length > 0;

  // ─── QFG helpers ─────────────────────────────────────────────────
  async function loadDefaultQfgDir() {
    try {
      const dir = await invoke("default_installers_dir", { collection: "quest-for-glory" });
      if (dir) { qfgDir = dir; await discoverQfg(); }
    } catch (_) {}
  }

  async function pickQfgDir() {
    const picked = await openDialog({ directory: true, defaultPath: qfgDir || undefined });
    if (typeof picked === "string") { qfgDir = picked; await discoverQfg(); }
  }

  async function discoverQfg() {
    qfgDiscoverError = null;
    qfgEntries = [];
    try {
      qfgEntries = await invoke("discover_qfg_installers", { directory: qfgDir });
      const next = {};
      for (const e of qfgEntries) next[e.game_id] = true;
      qfgSelected = next;
    } catch (e) { qfgDiscoverError = String(e); }
  }

  // ─── KQ helpers ──────────────────────────────────────────────────
  async function pickKqSource(gameId) {
    const picked = await openDialog({ directory: true });
    if (typeof picked !== "string") return;
    try {
      const entry = await invoke("build_kq_entry", { gameId, directory: picked });
      kqState = { ...kqState, [gameId]: { entry, error: null } };
    } catch (e) {
      kqState = { ...kqState, [gameId]: { entry: null, error: String(e) } };
    }
  }

  function clearKq(gameId) {
    const next = { ...kqState };
    delete next[gameId];
    kqState = next;
  }

  // ─── Amiga install ───────────────────────────────────────────────
  async function installAmiga(gameId) {
    const picked = await openDialog({
      filters: [{ name: "Amiga files", extensions: ["adf", "hdf", "rp9"] }]
    });
    if (!picked) return;
    const src = typeof picked === "string" ? picked : picked[0];
    installing = true;
    installError = null;
    log = [];
    entryStatus = { [gameId]: "pending" };
    try {
      await invoke("install_amiga_game", { gameId, sourcePath: src });
    } catch (e) {
      installError = String(e);
    } finally {
      installing = false;
      await loadCatalog();
    }
  }

  // ─── DOS collection install ──────────────────────────────────────
  async function installQfg() {
    const entries = qfgEntries.filter(e => qfgSelected[e.game_id]);
    if (entries.length === 0) return;
    installing = true;
    installError = null;
    log = [];
    entryStatus = {};
    for (const e of entries) entryStatus[e.game_id] = "pending";
    entryStatus = { ...entryStatus };
    try {
      await invoke("install_games", { collection: "quest-for-glory", entries });
    } catch (e) { installError = String(e); }
    finally { installing = false; await loadCatalog(); }
  }

  async function installKq() {
    const entries = Object.values(kqState).map(s => s.entry).filter(Boolean);
    if (entries.length === 0) return;
    installing = true;
    installError = null;
    log = [];
    entryStatus = {};
    for (const e of entries) entryStatus[e.game_id] = "pending";
    entryStatus = { ...entryStatus };
    try {
      await invoke("install_games", { collection: "kings-quest", entries });
    } catch (e) { installError = String(e); }
    finally { installing = false; await loadCatalog(); }
  }

  function appendLog(line, stream, gameId) {
    log = [...log, { line, stream, gameId }];
    requestAnimationFrame(() => { if (logEl) logEl.scrollTop = logEl.scrollHeight; });
  }

  function statusBadge(status) {
    switch (status) {
      case "installing": return { label: "installing…", cls: "badge-installing" };
      case "ok":         return { label: "ok",           cls: "badge-ok" };
      case "failed":     return { label: "failed",       cls: "badge-failed" };
      case "pending":    return { label: "pending",      cls: "badge-pending" };
      default:           return null;
    }
  }

  $: qfgSelectedCount = qfgEntries.filter(e => qfgSelected[e.game_id]).length;
  $: kqEntryCount = Object.values(kqState).filter(s => s.entry).length;

  onMount(async () => {
    await loadCatalog();
    await loadDefaultQfgDir();
    unlistenStarted  = await listen("game-install-started",  e => { entryStatus = { ...entryStatus, [e.payload.game_id]: "installing" }; });
    unlistenOutput   = await listen("game-install-output",   e => appendLog(e.payload.line, e.payload.stream, e.payload.game_id));
    unlistenFinished = await listen("game-install-finished", e => {
      const { game_id, exit_code } = e.payload;
      entryStatus = { ...entryStatus, [game_id]: exit_code === 0 ? "ok" : "failed" };
    });
    unlistenAborted  = await listen("game-install-aborted",  e => appendLog(`[aborted] ${e.payload.game_id} exited ${e.payload.exit_code}`, "stderr", e.payload.game_id));
  });

  onDestroy(() => {
    for (const u of [unlistenStarted, unlistenOutput, unlistenFinished, unlistenAborted]) {
      if (u) u();
    }
  });
</script>

<div class="catalog">
  <h2>Add Game</h2>

  {#if loading}
    <p class="status">Loading catalog…</p>
  {:else if error}
    <p class="status error">{error}</p>
  {:else if !hasUninstalled}
    <p class="status">All known games are already installed.</p>
  {:else}

    {#if uninstalledQfg.length > 0}
      <section>
        <div class="collection-header">
          <span>Quest for Glory ({uninstalledQfg.length} uninstalled)</span>
          <button class="secondary" on:click={() => activeCollectionInstall = activeCollectionInstall === "quest-for-glory" ? null : "quest-for-glory"} disabled={installing}>
            {activeCollectionInstall === "quest-for-glory" ? "Hide" : "Install collection…"}
          </button>
        </div>
        {#if activeCollectionInstall === "quest-for-glory"}
          <div class="sub-install">
            <div class="dir-row">
              <span class="dir-label">Installers directory</span>
              <span class="dir-path">{qfgDir || "(not set)"}</span>
              <button class="secondary" on:click={pickQfgDir} disabled={installing}>Change…</button>
            </div>
            {#if qfgDiscoverError}<p class="status error">{qfgDiscoverError}</p>{/if}
            {#if qfgEntries.length === 0 && !qfgDiscoverError}
              <p class="status">No installers found. Drop qfg1.exe–qfg4.exe into the directory then click Change…</p>
            {:else}
              <ul class="entries">
                {#each qfgEntries as entry}
                  {@const badge = statusBadge(entryStatus[entry.game_id])}
                  <li>
                    <label>
                      <input type="checkbox" bind:checked={qfgSelected[entry.game_id]} disabled={installing} />
                      <span class="entry-label">{entry.label}</span>
                    </label>
                    {#if badge}<span class="badge {badge.cls}">{badge.label}</span>{/if}
                  </li>
                {/each}
              </ul>
              <button class="primary" on:click={installQfg} disabled={installing || qfgSelectedCount === 0}>
                {installing ? "Installing…" : `Install ${qfgSelectedCount} selected`}
              </button>
            {/if}
          </div>
        {/if}
      </section>
    {/if}

    {#if uninstalledKq.length > 0}
      <section>
        <div class="collection-header">
          <span>King's Quest ({uninstalledKq.length} uninstalled)</span>
          <button class="secondary" on:click={() => activeCollectionInstall = activeCollectionInstall === "kings-quest" ? null : "kings-quest"} disabled={installing}>
            {activeCollectionInstall === "kings-quest" ? "Hide" : "Install collection…"}
          </button>
        </div>
        {#if activeCollectionInstall === "kings-quest"}
          <div class="sub-install">
            <p class="hint">Pick the Steam subfolder for each game (the folder with .EXE files at the top level).</p>
            <ul class="entries">
              {#each uninstalledKq as g}
                {@const state = kqState[g.id]}
                {@const badge = statusBadge(entryStatus[g.id])}
                <li class="kq-row">
                  <span class="entry-label">{g.title}</span>
                  <span class="kq-path">
                    {#if state?.entry}{state.entry.source}
                    {:else if state?.error}<span class="error-text">{state.error}</span>
                    {:else}<span class="muted">(not picked)</span>{/if}
                  </span>
                  <span class="kq-actions">
                    <button class="secondary" on:click={() => pickKqSource(g.id)} disabled={installing}>
                      {state?.entry ? "Change…" : "Pick folder…"}
                    </button>
                    {#if state?.entry}
                      <button class="link" on:click={() => clearKq(g.id)} disabled={installing}>clear</button>
                    {/if}
                    {#if badge}<span class="badge {badge.cls}">{badge.label}</span>{/if}
                  </span>
                </li>
              {/each}
            </ul>
            <button class="primary" on:click={installKq} disabled={installing || kqEntryCount === 0}>
              {installing ? "Installing…" : `Install ${kqEntryCount} game${kqEntryCount === 1 ? "" : "s"}`}
            </button>
          </div>
        {/if}
      </section>
    {/if}

    {#if uninstalledAmiga.length > 0}
      <section>
        <div class="collection-header"><span>Amiga</span></div>
        <ul class="entries">
          {#each uninstalledAmiga as g}
            {@const badge = statusBadge(entryStatus[g.id])}
            <li>
              <span class="entry-label">{g.title}</span>
              <span class="amiga-actions">
                <button class="secondary" on:click={() => installAmiga(g.id)} disabled={installing}>
                  Install (.adf / .hdf / .rp9)…
                </button>
                {#if badge}<span class="badge {badge.cls}">{badge.label}</span>{/if}
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

  {/if}

  {#if installError}
    <p class="status error">{installError}</p>
  {/if}

  {#if log.length > 0 || installing}
    <pre class="log" bind:this={logEl}>{#each log as entry}<span class="log-line {entry.stream}">[{entry.gameId}] {entry.line}
</span>{/each}{#if installing && log.length === 0}<span class="log-line stdout">starting…
</span>{/if}</pre>
  {/if}
</div>

<style>
  .catalog { flex: 1; overflow-y: auto; padding: 20px; max-width: 900px; }
  h2 { font-size: 1.1rem; font-weight: 600; color: #a0a8ff; margin-bottom: 16px; }
  section { margin-bottom: 24px; }
  .collection-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 8px 0; border-bottom: 1px solid #2a2a40; margin-bottom: 12px;
    color: #cfd2ff; font-size: 0.9rem;
  }
  .sub-install { padding-left: 12px; border-left: 2px solid #2a2a40; margin-top: 10px; }
  .dir-row {
    display: grid; grid-template-columns: auto 1fr auto;
    align-items: center; gap: 12px; padding: 8px 0;
    border-bottom: 1px solid #1e1e30; margin-bottom: 12px;
  }
  .dir-label { font-size: 0.85rem; color: #888; }
  .dir-path { font-family: monospace; font-size: 0.82rem; color: #bbb; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hint { font-size: 0.85rem; color: #888; margin-bottom: 12px; line-height: 1.5; }
  .entries { list-style: none; margin-bottom: 14px; }
  .entries li {
    display: flex; align-items: center; justify-content: space-between;
    gap: 12px; padding: 8px 0; border-bottom: 1px solid #1e1e30; font-size: 0.88rem;
  }
  .entries label { display: flex; align-items: baseline; gap: 10px; cursor: pointer; flex: 1; }
  .entry-label { color: #ccc; }
  .kq-row { display: grid; grid-template-columns: 220px 1fr auto; gap: 12px; align-items: center; }
  .kq-path { font-family: monospace; font-size: 0.78rem; color: #bbb; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .kq-actions, .amiga-actions { display: flex; align-items: center; gap: 8px; }
  .muted { color: #555; font-family: sans-serif; }
  .error-text { color: #ff8080; }
  button.primary { background: #3a3a7a; border: 1px solid #5555aa; color: #c0c8ff; padding: 8px 20px; border-radius: 6px; cursor: pointer; font-size: 0.9rem; }
  button.primary:hover:not(:disabled) { background: #4a4a9a; }
  button.primary:disabled { opacity: 0.5; cursor: default; }
  button.secondary { background: #2a2a45; border: 1px solid #4a4a70; color: #cfd2ff; padding: 4px 12px; border-radius: 4px; cursor: pointer; font-size: 0.8rem; }
  button.secondary:hover:not(:disabled) { background: #34345a; }
  button.secondary:disabled { opacity: 0.5; cursor: default; }
  button.link { background: transparent; border: none; color: #888; cursor: pointer; font-size: 0.78rem; padding: 0; text-decoration: underline; }
  button.link:disabled { opacity: 0.5; cursor: default; }
  .badge { font-size: 0.72rem; padding: 2px 8px; border-radius: 10px; border: 1px solid currentColor; flex-shrink: 0; }
  .badge-pending { color: #888; } .badge-installing { color: #e0c060; }
  .badge-ok { color: #4caf50; } .badge-failed { color: #ff8080; }
  .log { margin-top: 14px; padding: 10px 12px; background: #0d0d18; border: 1px solid #1e1e30; border-radius: 4px; max-height: 280px; overflow-y: auto; font-family: ui-monospace, monospace; font-size: 0.78rem; line-height: 1.4; white-space: pre-wrap; color: #b8b8c8; }
  .log-line.stderr { color: #ff9090; }
  .status { color: #888; font-size: 0.88rem; padding: 8px 0; }
  .status.error { color: #ff8080; }
</style>
```

- [ ] **Commit**

```bash
git add launcher/src/components/CatalogPanel.svelte
git commit -m "feat(ui): add CatalogPanel for uninstalled-game catalog and Amiga install"
```

---

### Task 12: Update `App.svelte` — wire up CatalogPanel, replace Install button

**Files:**
- Modify: `launcher/src/App.svelte`

- [ ] **Update the script block** — replace `installOpen`/`toggleInstall` with `catalogOpen`/`toggleCatalog`, import `CatalogPanel`, remove `InstallPanel` import

```svelte
<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import FilterBar from "./components/FilterBar.svelte";
  import GameGrid from "./components/GameGrid.svelte";
  import GameDetail from "./components/GameDetail.svelte";
  import DoctorPanel from "./components/DoctorPanel.svelte";
  import CatalogPanel from "./components/CatalogPanel.svelte";

  let games = [];
  let filter = "all";
  let selectedGame = null;
  let loading = true;
  let error = null;
  let doctorOpen = false;
  let catalogOpen = false;

  function toggleDoctor() {
    doctorOpen = !doctorOpen;
    if (doctorOpen) catalogOpen = false;
  }

  function toggleCatalog() {
    catalogOpen = !catalogOpen;
    if (catalogOpen) doctorOpen = false;
  }

  async function loadGames() {
    loading = true;
    error = null;
    try {
      games = await invoke("list_games");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadGames();
    window.addEventListener("focus", loadGames);
    return () => window.removeEventListener("focus", loadGames);
  });

  $: filtered = games.filter((g) => filter === "all" || g.platform === filter);
</script>
```

- [ ] **Update the template** — replace `installOpen` / `InstallPanel` references

```svelte
<div class="app">
  <header>
    <h1>Classic Launcher</h1>
    <div class="header-actions">
      <FilterBar bind:filter />
      <button class="doctor-btn" on:click={toggleCatalog}>
        {catalogOpen ? "✕ Add Game" : "+ Add Game"}
      </button>
      <button class="doctor-btn" on:click={toggleDoctor}>
        {doctorOpen ? "✕ Doctor" : "⚕ Doctor"}
      </button>
    </div>
  </header>

  {#if doctorOpen}
    <DoctorPanel />
  {:else if catalogOpen}
    <CatalogPanel />
  {:else if selectedGame}
    <GameDetail game={selectedGame} on:back={() => (selectedGame = null)} />
  {:else if loading}
    <div class="status">Loading games…</div>
  {:else if error}
    <div class="status error">Error loading games: {error}</div>
  {:else if games.length === 0}
    <div class="status">No games installed. Click <strong>+ Add Game</strong> to install one.</div>
  {:else}
    <GameGrid games={filtered} on:select={(e) => (selectedGame = e.detail)} />
  {/if}
</div>
```

(Keep the `<style>` block unchanged.)

- [ ] **Run the dev server and manually verify**

```bash
cd launcher && pnpm tauri dev
```

Check:
1. Grid shows only installed games (empty if `~/games` is empty)
2. "+ Add Game" button opens the catalog panel
3. Catalog panel shows uninstalled games grouped by collection/platform
4. Doctor panel still opens via "⚕ Doctor"

- [ ] **Run cargo tests one final time**

```bash
cd launcher && cargo test
```

Expected: all pass.

- [ ] **Commit**

```bash
git add launcher/src/App.svelte
git commit -m "feat(ui): wire CatalogPanel, show empty-library hint when no games installed"
```

---

## Verification

End-to-end checks after completing all tasks:

- `classic-launcher list` shows nothing when `~/games` is empty; shows installed games when directories exist
- `classic-launcher run qfg1-ega --dry-run` prints `[sidecar] fluidsynth` and `[primary] flatpak` (unchanged behavior)
- `classic-launcher doctor` reports per-game install dir status using `~/games/{id}` paths
- GUI: game grid is empty until games are installed
- GUI: "+ Add Game" opens catalog; QFG and KQ collection install flows work as before
- GUI: Amiga game file picker copies to `~/games/{id}/`, game appears in library on refocus
- `cargo test` — full suite passes
