use assert_cmd::Command;
use predicates::str::contains;
use std::path::{Path, PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Build a `reliquaint` invocation pointed at the fixture tap with an
/// isolated installs directory and a deliberately-missing user config
/// (so the launcher uses defaults rather than picking up the developer's
/// real ~/.config/reliquaint/config.toml). The caller may pre-populate
/// the installs dir to set up "installed" entries.
fn launcher(installs_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("reliquaint").unwrap();
    cmd.env("RELIQUAINT_REPO_ROOT", fixture_root())
        .env("RELIQUAINT_INSTALLS_DIR", installs_dir)
        .env(
            "RELIQUAINT_USER_CONFIG_PATH",
            installs_dir.join("nonexistent-config.toml"),
        );
    cmd
}

fn write_install_record(dir: &Path, catalog_id: &str, install_path: &str) {
    let body = format!(
        r#"schema_version = 1

[install]
catalog_id   = "{catalog_id}"
tap          = "reliquaint-core"
install_path = "{install_path}"
installed_at = 2026-05-23T14:32:00Z
"#
    );
    std::fs::write(dir.join(format!("{catalog_id}.toml")), body).unwrap();
}

#[test]
fn list_shows_fixture_entries_grouped_by_collection() {
    let installs = tempfile::tempdir().unwrap();
    let output = launcher(installs.path()).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success(), "list should succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr));

    // Both fixture entries appear:
    assert!(stdout.contains("qfg1-ega"), "stdout missing qfg1-ega: {stdout}");
    assert!(stdout.contains("fatman"), "stdout missing fatman: {stdout}");

    // Grouped under collection headers (qfg1-ega has collection
    // "quest-for-glory"; fatman has no collection):
    assert!(stdout.contains("quest-for-glory"), "missing collection header: {stdout}");
    assert!(stdout.contains("(no collection)"), "missing no-collection bucket: {stdout}");

    // Status column present:
    assert!(stdout.contains("not installed"), "missing not-installed status: {stdout}");
}

#[test]
fn list_platform_dos_excludes_amiga() {
    let installs = tempfile::tempdir().unwrap();
    let output = launcher(installs.path())
        .args(["list", "--platform", "dos"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("qfg1-ega"));
    assert!(!stdout.contains("fatman"), "amiga entry leaked into --platform dos: {stdout}");
}

#[test]
fn list_platform_amiga_excludes_dos() {
    let installs = tempfile::tempdir().unwrap();
    let output = launcher(installs.path())
        .args(["list", "--platform", "amiga"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("fatman"));
    assert!(!stdout.contains("qfg1-ega"), "dos entry leaked into --platform amiga: {stdout}");
}

#[test]
fn list_installed_filter_only_shows_entries_with_records() {
    let installs = tempfile::tempdir().unwrap();
    // Mark qfg1-ega as installed; leave fatman uninstalled.
    write_install_record(installs.path(), "qfg1-ega", "/tmp/fake-install-qfg1-ega");

    let output = launcher(installs.path())
        .args(["list", "--installed"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("qfg1-ega"));
    assert!(!stdout.contains("fatman"), "uninstalled entry shouldn't appear: {stdout}");
    assert!(stdout.contains("installed"));
}

#[test]
fn list_not_installed_filter_excludes_records() {
    let installs = tempfile::tempdir().unwrap();
    write_install_record(installs.path(), "qfg1-ega", "/tmp/fake-install-qfg1-ega");

    let output = launcher(installs.path())
        .args(["list", "--not-installed"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("fatman"));
    assert!(
        !stdout.contains("qfg1-ega"),
        "installed entry shouldn't appear: {stdout}"
    );
}

#[test]
fn list_installed_and_not_installed_are_mutually_exclusive() {
    let installs = tempfile::tempdir().unwrap();
    launcher(installs.path())
        .args(["list", "--installed", "--not-installed"])
        .assert()
        .failure()
        .stderr(contains("cannot be used with"));
}

// --- run tests -----------------------------------------------------------

#[test]
fn run_unknown_id_fails_with_hint() {
    let installs = tempfile::tempdir().unwrap();
    launcher(installs.path())
        .args(["run", "nonexistent-game"])
        .assert()
        .failure()
        .stderr(contains("no catalog entry"))
        .stderr(contains("reliquaint list"));
}

#[test]
fn run_without_install_record_fails_with_install_hint() {
    let installs = tempfile::tempdir().unwrap();
    launcher(installs.path())
        .args(["run", "qfg1-ega"])
        .assert()
        .failure()
        .stderr(contains("no installation record"))
        .stderr(contains("reliquaint install qfg1-ega"));
}

#[test]
fn run_dos_dry_run_prints_primary_and_fluidsynth_sidecar() {
    let installs = tempfile::tempdir().unwrap();
    write_install_record(installs.path(), "qfg1-ega", "/home/test/games/qfg1-ega");

    let output = launcher(installs.path())
        .args(["run", "qfg1-ega", "--dry-run"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("[sidecar:fluidsynth]"),
        "missing sidecar line: {stdout}"
    );
    assert!(stdout.contains("[primary]"), "missing primary line: {stdout}");
    assert!(stdout.contains("flatpak"), "primary should use flatpak: {stdout}");
    assert!(stdout.contains("io.github.dosbox-staging"), "missing dosbox id: {stdout}");
    assert!(stdout.contains("SIERRA.BAT"), "missing entry command: {stdout}");
    assert!(
        stdout.contains("/home/test/games/qfg1-ega"),
        "missing install path: {stdout}"
    );
}

#[test]
fn run_amiga_dry_run_prints_primary_with_no_sidecars() {
    let installs = tempfile::tempdir().unwrap();
    write_install_record(installs.path(), "fatman", "/home/test/games/fatman");

    let output = launcher(installs.path())
        .args(["run", "fatman", "--dry-run"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    assert!(!stdout.contains("[sidecar:"), "fatman should have no sidecars: {stdout}");
    assert!(stdout.contains("[primary]"));
    assert!(stdout.contains("fs-uae"));
    assert!(stdout.contains("--amiga_model=A500"));
    assert!(stdout.contains("--floppy_drive_0=/home/test/games/fatman/fatman.adf"));
}

// --- install tests -------------------------------------------------------

fn install_record_path(installs_dir: &Path, id: &str) -> PathBuf {
    installs_dir.join(format!("{id}.toml"))
}

#[test]
fn install_writes_record_when_expects_files_present() {
    let installs = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();
    // qfg1-ega declares expects_files = ["SIERRA.BAT", "RESOURCE.000"]
    std::fs::write(game.path().join("SIERRA.BAT"), b"").unwrap();
    std::fs::write(game.path().join("RESOURCE.000"), b"").unwrap();

    let output = launcher(installs.path())
        .args(["install", "qfg1-ega"])
        .arg(game.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "install should succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let record = install_record_path(installs.path(), "qfg1-ega");
    assert!(record.exists(), "record should exist at {}", record.display());

    // list now shows it as installed.
    let list_out = launcher(installs.path()).arg("list").output().unwrap();
    let stdout = String::from_utf8(list_out.stdout).unwrap();
    assert!(stdout.contains("installed"), "list output: {stdout}");
}

#[test]
fn install_force_writes_record_with_missing_expects_files() {
    let installs = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();
    // empty game dir; expects_files missing

    launcher(installs.path())
        .args(["install", "qfg1-ega", "--force"])
        .arg(game.path())
        .assert()
        .success();

    assert!(install_record_path(installs.path(), "qfg1-ega").exists());
}

#[test]
fn install_aborts_when_prompt_declined() {
    let installs = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();

    launcher(installs.path())
        .args(["install", "qfg1-ega"])
        .arg(game.path())
        .write_stdin("n\n")
        .assert()
        .failure()
        .stderr(contains("aborted"));

    assert!(!install_record_path(installs.path(), "qfg1-ega").exists());
}

#[test]
fn install_writes_record_when_prompt_accepted() {
    let installs = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();

    launcher(installs.path())
        .args(["install", "qfg1-ega"])
        .arg(game.path())
        .write_stdin("y\n")
        .assert()
        .success();

    assert!(install_record_path(installs.path(), "qfg1-ega").exists());
}

#[test]
fn install_unknown_id_fails() {
    let installs = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();

    launcher(installs.path())
        .args(["install", "nonexistent"])
        .arg(game.path())
        .assert()
        .failure()
        .stderr(contains("no catalog entry"));
}

#[test]
fn install_rejects_path_that_is_a_file() {
    let installs = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a-file.txt");
    std::fs::write(&file, b"").unwrap();

    launcher(installs.path())
        .args(["install", "qfg1-ega", "--force"])
        .arg(&file)
        .assert()
        .failure()
        .stderr(contains("is not a directory"));
}

// --- list tests (continued) ----------------------------------------------

#[test]
fn list_format_json_emits_valid_array() {
    let installs = tempfile::tempdir().unwrap();
    write_install_record(installs.path(), "qfg1-ega", "/games/qfg1-ega");

    let output = launcher(installs.path())
        .args(["list", "--format", "json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("output isn't valid JSON ({e}): {stdout}"));
    let arr = value.as_array().expect("expected top-level array");
    assert_eq!(arr.len(), 2, "expected 2 entries, got: {stdout}");

    // qfg1-ega should be the installed one:
    let qfg = arr
        .iter()
        .find(|e| e["id"] == "qfg1-ega")
        .expect("qfg1-ega missing from json");
    assert_eq!(qfg["installed"], true);
    assert_eq!(qfg["platform"], "dos");
    assert_eq!(qfg["tap_id"], "reliquaint-core");
    assert_eq!(qfg["install_path"], "/games/qfg1-ega");

    // fatman should be present and not installed:
    let fatman = arr
        .iter()
        .find(|e| e["id"] == "fatman")
        .expect("fatman missing from json");
    assert_eq!(fatman["installed"], false);
    assert_eq!(fatman["platform"], "amiga");
}
