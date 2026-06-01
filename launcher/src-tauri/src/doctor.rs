use crate::catalog::Platform;
use crate::catalog_view::CatalogView;
use crate::user_config::UserConfig;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
pub enum ProbeStatus {
    Ok,
    Missing,
    Unknown,
    /// Advisory issue — surfaced to the user but not a host failure, so it does
    /// not flip `reliquaint doctor`'s exit code. Used for companion-content
    /// quality problems (broken refs, stray dirs, stripped raw HTML).
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeKind {
    DosboxFlatpak,
    Fluidsynth,
    Soundfont,
    Innoextract,
    FsUae,
    Unzip,
    GameInstallDir(String),
    /// A companion-content health issue, tagged with `"<tap>/<game>"`.
    Companion(String),
}

pub struct ProbeResult {
    pub name: String,
    pub status: ProbeStatus,
    pub detail: Option<String>,
    pub kind: ProbeKind,
}

// --- New install-aware doctor (Milestone 4, Task 4.4) -------------------
//
// run_all (above) stays the source of truth for the legacy code path that
// commands.rs / the GUI still call until Milestone 5. check_install is
// what the new CLI invokes.

/// Run host-dependency and install-record diagnostics against the new
/// model. Checks emulator binaries, fluidsynth soundfont (only if any
/// installed game declares it), fs-uae kickstart_path (only if any
/// Amiga game is installed and the path is set), and per-install
/// directory + expects_files. Orphan install records are surfaced as
/// missing probes too.
pub fn check_install(view: &CatalogView, user_config: &UserConfig) -> Vec<ProbeResult> {
    let mut results = Vec::new();

    results.push(probe_emulator_command(
        "dosbox-staging",
        &user_config.emulators.dosbox_staging.command,
        ProbeKind::DosboxFlatpak,
    ));
    results.push(probe_emulator_command(
        "fs-uae",
        &user_config.emulators.fs_uae.command,
        ProbeKind::FsUae,
    ));

    let installed: Vec<_> = view.installed_only().collect();

    let any_uses_fluidsynth = installed
        .iter()
        .any(|e| e.catalog.runtime.sidecars.iter().any(|s| s == "fluidsynth"));
    if any_uses_fluidsynth {
        results.push(probe_path_exists(
            "fluidsynth soundfont",
            &user_config.sidecars.fluidsynth.soundfont,
            ProbeKind::Soundfont,
        ));
    }

    let any_amiga_installed = installed
        .iter()
        .any(|e| e.catalog.game.platform == Platform::Amiga);
    if any_amiga_installed {
        if let Some(kdir) = &user_config.emulators.fs_uae.kickstart_path {
            results.push(probe_path_exists(
                "fs-uae kickstart_path",
                kdir,
                ProbeKind::FsUae,
            ));
        }
    }

    for entry in &installed {
        let install = entry
            .install
            .as_ref()
            .expect("installed_only() yields entries with Some install");
        let install_path = &install.install.install_path;
        let id = &entry.catalog.game.id;

        if !install_path.is_dir() {
            results.push(ProbeResult {
                name: format!("install path for {id}"),
                status: ProbeStatus::Missing,
                detail: Some(install_path.display().to_string()),
                kind: ProbeKind::GameInstallDir(id.clone()),
            });
            continue;
        }

        let missing = missing_expects_files(install_path, &entry.catalog.install.expects_files);
        if missing.is_empty() {
            results.push(ProbeResult {
                name: format!("install for {id}"),
                status: ProbeStatus::Ok,
                detail: Some(install_path.display().to_string()),
                kind: ProbeKind::GameInstallDir(id.clone()),
            });
        } else {
            results.push(ProbeResult {
                name: format!("expects_files for {id}"),
                status: ProbeStatus::Missing,
                detail: Some(format!(
                    "missing in {}: {}",
                    install_path.display(),
                    missing.join(", ")
                )),
                kind: ProbeKind::GameInstallDir(id.clone()),
            });
        }
    }

    for orphan in view.orphans() {
        let tap_id = &orphan.record.install.tap;
        let suggestion = if crate::known_taps::KNOWN_TAPS
            .iter()
            .any(|(name, _)| name == tap_id)
        {
            format!(" — run: reliquaint tap add {tap_id}")
        } else {
            String::new()
        };
        results.push(ProbeResult {
            name: format!(
                "orphan install record: {}",
                orphan.record.install.catalog_id
            ),
            status: ProbeStatus::Missing,
            detail: Some(format!(
                "tap={tap_id:?} catalog_id={:?} — no matching entry in any loaded tap{suggestion}",
                orphan.record.install.catalog_id
            )),
            kind: ProbeKind::GameInstallDir(orphan.record.install.catalog_id.clone()),
        });
    }

    results.extend(check_companion(view));

    results
}

/// Companion-content health checks (Milestone 4, Task 4.2). All findings are
/// advisory [`ProbeStatus::Warn`] — tap-content quality issues, not host
/// failures. Three checks:
///
/// 1. **Broken image references** — relative images in installed games'
///    companion Markdown that point at files that don't exist on disk.
/// 2. **Stray companion directories** — `companion/<game-id>/` content for a
///    game with no catalog entry in that tap (likely a maintainer typo).
/// 3. **Stripped raw HTML** — Markdown carrying raw HTML the sanitizer drops,
///    so the author's intended output won't render.
pub fn check_companion(view: &CatalogView) -> Vec<ProbeResult> {
    let mut results = Vec::new();

    // 1 & 3: per installed game, inspect its companion Markdown.
    for entry in view.installed_only() {
        let game_id = &entry.catalog.game.id;
        for item in view.companion_for(game_id) {
            if item.kind != crate::companion::CompanionKind::Markdown {
                continue;
            }
            let md = match crate::companion_protocol::read_markdown(
                &item.tap_id,
                &item.game_id,
                &item.rel_path,
            ) {
                Ok(md) => md,
                // Unreadable Markdown is reported by the image/render paths; the
                // doctor focuses on content quality, so skip rather than error.
                Err(_) => continue,
            };
            let where_ = format!("{}/{}/{}", item.tap_id, item.game_id, item.rel_path);

            // Check 1: image references that won't render. Validate through the
            // same resolver the viewer uses, so doctor's verdict matches what
            // actually happens — a plain `exists()` would accept traversal-like
            // refs, symlink escapes, and wrong-format files that the protocol
            // rejects at serve time.
            let companion_dir = crate::paths::companion_dir(&item.tap_id, &item.game_id);
            for img in crate::companion_render::relative_image_refs(&md) {
                if let Err(e) = crate::companion_protocol::resolve_image_in(&companion_dir, &img) {
                    use crate::companion_protocol::ImageError;
                    let reason = match e {
                        ImageError::NotFound => format!("references missing file: {img}"),
                        ImageError::OutsideBoundary => {
                            format!("reference escapes the companion directory: {img}")
                        }
                        ImageError::BadFormat => {
                            format!("not a renderable image (bad extension or contents): {img}")
                        }
                        ImageError::Io => format!("could not read image: {img}"),
                    };
                    results.push(ProbeResult {
                        name: format!("broken image in {where_}"),
                        status: ProbeStatus::Warn,
                        detail: Some(reason),
                        kind: ProbeKind::Companion(format!("{}/{}", item.tap_id, item.game_id)),
                    });
                }
            }

            // Check 3: raw HTML the sanitizer will strip.
            let raw = crate::companion_render::raw_html_byte_count(&md);
            if raw > RAW_HTML_WARN_THRESHOLD {
                results.push(ProbeResult {
                    name: format!("raw HTML stripped in {where_}"),
                    status: ProbeStatus::Warn,
                    detail: Some(format!(
                        "{raw} bytes of raw HTML were neutralized — they won't render"
                    )),
                    kind: ProbeKind::Companion(format!("{}/{}", item.tap_id, item.game_id)),
                });
            }
        }
    }

    // 2: companion directories for games not in the owning tap's catalog.
    let mut seen: Vec<(String, String)> = Vec::new();
    for item in view.all_companion() {
        let key = (item.tap_id.clone(), item.game_id.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        if view.by_tap_and_id(&item.tap_id, &item.game_id).is_none() {
            results.push(ProbeResult {
                name: format!("companion content for {}/{}", item.tap_id, item.game_id),
                status: ProbeStatus::Warn,
                detail: Some(
                    "no catalog entry for this game in that tap — maintainer typo?".to_string(),
                ),
                kind: ProbeKind::Companion(format!("{}/{}", item.tap_id, item.game_id)),
            });
        }
    }

    results
}

/// Raw-HTML byte count above which the doctor warns. A handful of bytes can
/// arise from incidental angle brackets; this catches deliberate HTML blocks.
const RAW_HTML_WARN_THRESHOLD: usize = 32;

fn probe_emulator_command(name: &str, command: &str, kind: ProbeKind) -> ProbeResult {
    let mut tokens = command.split_whitespace();
    let Some(program) = tokens.next() else {
        return ProbeResult {
            name: name.to_string(),
            status: ProbeStatus::Missing,
            detail: Some("user config command is empty".into()),
            kind,
        };
    };

    // flatpak run <app_id> is the Debian-default DOSBox setup; probe the
    // app itself rather than just `which flatpak`.
    if program == "flatpak" {
        let rest: Vec<&str> = tokens.collect();
        if rest.len() >= 2 && rest[0] == "run" {
            return probe_flatpak_app(name, rest[1], kind);
        }
    }

    probe_which_named(name, program, kind)
}

fn probe_which_named(name: &str, program: &str, kind: ProbeKind) -> ProbeResult {
    let status = Command::new("which")
        .arg(program)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    ProbeResult {
        name: name.to_string(),
        status: match status {
            Ok(s) if s.success() => ProbeStatus::Ok,
            Ok(_) => ProbeStatus::Missing,
            Err(_) => ProbeStatus::Unknown,
        },
        detail: Some(program.to_string()),
        kind,
    }
}

fn probe_flatpak_app(name: &str, app_id: &str, kind: ProbeKind) -> ProbeResult {
    let status = Command::new("flatpak")
        .args(["info", app_id])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    ProbeResult {
        name: name.to_string(),
        status: match status {
            Ok(s) if s.success() => ProbeStatus::Ok,
            Ok(_) => ProbeStatus::Missing,
            Err(_) => ProbeStatus::Unknown,
        },
        detail: Some(format!("flatpak run {app_id}")),
        kind,
    }
}

fn probe_path_exists(name: &str, path: &Path, kind: ProbeKind) -> ProbeResult {
    ProbeResult {
        name: name.to_string(),
        status: if path.exists() {
            ProbeStatus::Ok
        } else {
            ProbeStatus::Missing
        },
        detail: Some(path.display().to_string()),
        kind,
    }
}

/// Case-insensitive check; same logic as cli::missing_expects_files but
/// duplicated to keep the modules from depending on each other.
fn missing_expects_files(install_dir: &Path, expected: &[String]) -> Vec<String> {
    let entries: Vec<String> = match std::fs::read_dir(install_dir) {
        Ok(read) => read
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect(),
        Err(_) => Vec::new(),
    };
    expected
        .iter()
        .filter(|exp| !entries.iter().any(|e| e.eq_ignore_ascii_case(exp)))
        .cloned()
        .collect()
}
