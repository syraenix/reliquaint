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
        results.push(ProbeResult {
            name: format!(
                "orphan install record: {}",
                orphan.record.install.catalog_id
            ),
            status: ProbeStatus::Missing,
            detail: Some(format!(
                "tap={:?} catalog_id={:?} — no matching entry in any loaded tap",
                orphan.record.install.tap, orphan.record.install.catalog_id
            )),
            kind: ProbeKind::GameInstallDir(orphan.record.install.catalog_id.clone()),
        });
    }

    results
}

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

