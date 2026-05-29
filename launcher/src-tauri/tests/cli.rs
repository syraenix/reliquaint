use assert_cmd::Command;
use predicates::str::contains;
use std::path::{Path, PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Build a `reliquaint` invocation pointed at the fixture tap with an
/// isolated installs directory and a deliberately-missing user config
/// (so the launcher uses defaults rather than picking up the developer's
/// real ~/.config/reliquaint/config.toml). The user tap is pointed at
/// the same isolated directory by default; tests that want to exercise
/// the user tap pre-populate `<installs_dir>/user-tap/`. The caller may
/// pre-populate the installs dir to set up "installed" entries.
fn launcher(installs_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("reliquaint").unwrap();
    cmd.env("RELIQUAINT_REPO_ROOT", fixture_root())
        .env("RELIQUAINT_INSTALLS_DIR", installs_dir)
        .env(
            "RELIQUAINT_USER_CONFIG_PATH",
            installs_dir.join("nonexistent-config.toml"),
        )
        .env("RELIQUAINT_USER_TAP_DIR", installs_dir.join("user-tap"));
    cmd
}

/// Seed the user tap directory with metadata + a single DOS catalog
/// entry, mirroring what the wizard will write in M4.
fn seed_user_tap_with_dos_entry(user_tap_root: &Path, id: &str, title: &str) {
    let catalog_dir = user_tap_root.join("catalog/dos");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    std::fs::write(
        user_tap_root.join("tap.toml"),
        r#"schema_version = 1
id = "local"
title = "Local games"
description = "User-created entries."
version = "0.1.0"
maintainer = "local"
url = "https://github.com/syraenix/reliquaint"
license = "CC0-1.0"
"#,
    )
    .unwrap();
    let body = format!(
        r#"schema_version = 1
[game]
id = "{id}"
title = "{title}"
platform = "dos"
[runtime]
emulator = "dosbox-staging"
[runtime.dosbox]
config = "{id}.conf"
entry = "TEST.EXE"
"#
    );
    std::fs::write(catalog_dir.join(format!("{id}.toml")), body).unwrap();
}

fn write_install_record(dir: &Path, catalog_id: &str, install_path: &str) {
    write_install_record_for_tap(dir, catalog_id, "reliquaint-core", install_path);
}

fn write_install_record_for_tap(dir: &Path, catalog_id: &str, tap_id: &str, install_path: &str) {
    let body = format!(
        r#"schema_version = 1

[install]
catalog_id   = "{catalog_id}"
tap          = "{tap_id}"
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

    assert!(
        output.status.success(),
        "list should succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Both fixture entries appear:
    assert!(
        stdout.contains("qfg1-ega"),
        "stdout missing qfg1-ega: {stdout}"
    );
    assert!(stdout.contains("fatman"), "stdout missing fatman: {stdout}");

    // Grouped under collection headers (qfg1-ega has collection
    // "quest-for-glory"; fatman has no collection):
    assert!(
        stdout.contains("quest-for-glory"),
        "missing collection header: {stdout}"
    );
    assert!(
        stdout.contains("(no collection)"),
        "missing no-collection bucket: {stdout}"
    );

    // Status column present:
    assert!(
        stdout.contains("not installed"),
        "missing not-installed status: {stdout}"
    );
}

#[test]
fn list_includes_user_tap_entries_alongside_bundled() {
    let installs = tempfile::tempdir().unwrap();
    let user_tap = installs.path().join("user-tap");
    seed_user_tap_with_dos_entry(&user_tap, "my-custom-game", "My Custom Game");

    let output = launcher(installs.path())
        .args(["list", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "list should succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Bundled fixture entries still present:
    assert!(
        stdout.contains("qfg1-ega"),
        "missing bundled entry: {stdout}"
    );
    assert!(stdout.contains("fatman"), "missing bundled entry: {stdout}");
    // User tap entry surfaced with tap-of-origin "local":
    assert!(
        stdout.contains("my-custom-game"),
        "missing user-tap entry: {stdout}"
    );
    assert!(
        stdout.contains("\"tap_id\":\"local\"") || stdout.contains("\"tap_id\": \"local\""),
        "user-tap entry missing tap_id=local: {stdout}"
    );
}

#[test]
fn list_warns_when_external_tap_claims_reserved_local_id() {
    let installs = tempfile::tempdir().unwrap();
    // Pretend the user tap dir contains a *valid* tap.toml with a wrong id.
    // load_user_tap rejects this, but the bundled catalog still loads.
    let user_tap = installs.path().join("user-tap");
    std::fs::create_dir_all(&user_tap).unwrap();
    std::fs::write(
        user_tap.join("tap.toml"),
        r#"schema_version = 1
id = "third-party"
title = "x"
description = "x"
version = "0.1.0"
maintainer = "x"
url = "https://x.test"
license = "MIT"
"#,
    )
    .unwrap();

    let output = launcher(installs.path()).arg("list").output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("user tap failed to load")
            || stderr.contains("must be \"local\"")
            || stderr.contains("third-party"),
        "expected user-tap warning, got stderr: {stderr}"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("qfg1-ega"),
        "bundled entries should still load: {stdout}"
    );
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
    assert!(
        !stdout.contains("fatman"),
        "amiga entry leaked into --platform dos: {stdout}"
    );
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
    assert!(
        !stdout.contains("qfg1-ega"),
        "dos entry leaked into --platform amiga: {stdout}"
    );
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
    assert!(
        !stdout.contains("fatman"),
        "uninstalled entry shouldn't appear: {stdout}"
    );
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
    assert!(
        stdout.contains("[primary]"),
        "missing primary line: {stdout}"
    );
    assert!(
        stdout.contains("flatpak"),
        "primary should use flatpak: {stdout}"
    );
    assert!(
        stdout.contains("io.github.dosbox-staging"),
        "missing dosbox id: {stdout}"
    );
    assert!(
        stdout.contains("SIERRA.BAT"),
        "missing entry command: {stdout}"
    );
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

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !stdout.contains("[sidecar:"),
        "fatman should have no sidecars: {stdout}"
    );
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
    let games = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();
    // qfg1-ega declares expects_files = ["SIERRA.BAT", "RESOURCE.000"]
    std::fs::write(game.path().join("SIERRA.BAT"), b"").unwrap();
    std::fs::write(game.path().join("RESOURCE.000"), b"").unwrap();

    let output = launcher(installs.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
        .args(["install", "qfg1-ega"])
        .arg(game.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "install should succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Files copied into the managed library; record written.
    assert!(games.path().join("qfg1-ega/SIERRA.BAT").is_file());
    let record = install_record_path(installs.path(), "qfg1-ega");
    assert!(
        record.exists(),
        "record should exist at {}",
        record.display()
    );

    // list now shows it as installed.
    let list_out = launcher(installs.path()).arg("list").output().unwrap();
    let stdout = String::from_utf8(list_out.stdout).unwrap();
    assert!(stdout.contains("installed"), "list output: {stdout}");
}

#[test]
fn install_force_writes_record_with_missing_expects_files() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();
    // empty game dir; expects_files missing

    launcher(installs.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
        .args(["install", "qfg1-ega", "--force"])
        .arg(game.path())
        .assert()
        .success();

    assert!(install_record_path(installs.path(), "qfg1-ega").exists());
}

#[test]
fn install_aborts_when_prompt_declined() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();

    launcher(installs.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
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
    let games = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();

    launcher(installs.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
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

    // A plain file is neither a directory nor a recognized installer/image.
    launcher(installs.path())
        .args(["install", "qfg1-ega", "--force"])
        .arg(&file)
        .assert()
        .failure()
        .stderr(contains("unsupported source"));
}

// --- migrate-installs tests ----------------------------------------------

#[test]
fn migrate_installs_picks_up_legacy_per_id_dirs() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();

    // qfg1-ega and fatman are the catalog fixture entries. Stage two
    // games under the legacy ~/games/<id>/ layout.
    let qfg_dir = games.path().join("qfg1-ega");
    std::fs::create_dir(&qfg_dir).unwrap();
    std::fs::write(qfg_dir.join("SIERRA.BAT"), b"").unwrap();
    std::fs::write(qfg_dir.join("RESOURCE.000"), b"").unwrap();

    let fatman_dir = games.path().join("fatman");
    std::fs::create_dir(&fatman_dir).unwrap();
    std::fs::write(fatman_dir.join("fatman.adf"), b"").unwrap();

    let output = launcher(installs.path())
        .args(["migrate-installs", "--base"])
        .arg(games.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("registered qfg1-ega"));
    assert!(stdout.contains("registered fatman"));
    assert!(stdout.contains("2 migrated"));

    // Both install records were written.
    assert!(installs.path().join("qfg1-ega.toml").exists());
    assert!(installs.path().join("fatman.toml").exists());
}

#[test]
fn migrate_installs_skips_already_installed_entries() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();

    // qfg1-ega already has a record; the migrator should skip it.
    write_install_record(installs.path(), "qfg1-ega", "/existing/path");

    let qfg_dir = games.path().join("qfg1-ega");
    std::fs::create_dir(&qfg_dir).unwrap();

    let output = launcher(installs.path())
        .args(["migrate-installs", "--base"])
        .arg(games.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("1 already installed"));
    assert!(stdout.contains("0 migrated"));
}

#[test]
fn migrate_installs_errors_when_base_missing() {
    let installs = tempfile::tempdir().unwrap();
    launcher(installs.path())
        .args([
            "migrate-installs",
            "--base",
            "/definitely/not/a/real/games/dir",
        ])
        .assert()
        .failure()
        .stderr(contains("does not exist"));
}

// --- doctor tests --------------------------------------------------------

#[test]
fn doctor_always_includes_emulator_probes() {
    let installs = tempfile::tempdir().unwrap();
    let output = launcher(installs.path()).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("dosbox-staging"),
        "missing dosbox probe: {stdout}"
    );
    assert!(stdout.contains("fs-uae"), "missing fs-uae probe: {stdout}");
}

#[test]
fn doctor_omits_soundfont_check_when_no_fluidsynth_user_installed() {
    let installs = tempfile::tempdir().unwrap();
    // No install records → no installed games → no soundfont probe.
    let output = launcher(installs.path()).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains("fluidsynth soundfont"),
        "soundfont probe shouldn't appear when nothing's installed: {stdout}"
    );
}

#[test]
fn doctor_includes_soundfont_check_when_fluidsynth_game_installed() {
    let installs = tempfile::tempdir().unwrap();
    // qfg1-ega declares fluidsynth in runtime.sidecars; an install
    // record for it should trigger the soundfont probe.
    write_install_record(installs.path(), "qfg1-ega", "/nonexistent/qfg1");

    let output = launcher(installs.path()).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("fluidsynth soundfont"),
        "soundfont probe should appear: {stdout}"
    );
}

#[test]
fn doctor_reports_missing_install_path() {
    let installs = tempfile::tempdir().unwrap();
    write_install_record(
        installs.path(),
        "qfg1-ega",
        "/definitely/not/a/real/path/anywhere",
    );

    let output = launcher(installs.path()).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("install path for qfg1-ega"),
        "missing install path not reported: {stdout}"
    );
    assert!(
        stdout.contains("missing"),
        "expected a missing-status line: {stdout}"
    );
    // Deliberately-broken state → exit code 2.
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn doctor_reports_ok_for_valid_install_with_expects_files() {
    let installs = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();
    std::fs::write(game.path().join("SIERRA.BAT"), b"").unwrap();
    std::fs::write(game.path().join("RESOURCE.000"), b"").unwrap();
    write_install_record(installs.path(), "qfg1-ega", game.path().to_str().unwrap());

    let output = launcher(installs.path()).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // The per-install probe should be Ok (no "missing" against it).
    assert!(
        stdout.contains("install for qfg1-ega"),
        "missing install probe line: {stdout}"
    );
    assert!(
        !stdout.contains("expects_files for qfg1-ega"),
        "expects_files probe should not appear when all present: {stdout}"
    );
}

#[test]
fn doctor_reports_missing_expects_files() {
    let installs = tempfile::tempdir().unwrap();
    let game = tempfile::tempdir().unwrap();
    // Game dir exists but has none of the expected files.
    write_install_record(installs.path(), "qfg1-ega", game.path().to_str().unwrap());

    let output = launcher(installs.path()).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("expects_files for qfg1-ega"),
        "expects_files probe missing: {stdout}"
    );
    assert!(
        stdout.contains("SIERRA.BAT"),
        "should name missing file: {stdout}"
    );
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn doctor_reports_orphan_install_records() {
    let installs = tempfile::tempdir().unwrap();
    // Catalog id doesn't exist in the fixture tap → orphan.
    write_install_record(installs.path(), "ghost-game", "/tmp/whatever");

    let output = launcher(installs.path()).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("orphan install record"),
        "orphan not reported: {stdout}"
    );
    assert!(
        stdout.contains("ghost-game"),
        "orphan id missing from output: {stdout}"
    );
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

/// A source directory holding the files qfg1-ega's catalog entry expects.
fn dos_source_with_expected_files() -> tempfile::TempDir {
    let source = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("SIERRA.BAT"), b"@echo off\n").unwrap();
    std::fs::write(source.path().join("RESOURCE.000"), b"data").unwrap();
    source
}

#[test]
fn install_dest_flag_overrides_library_dir() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    let source = dos_source_with_expected_files();

    launcher(installs.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
        .args([
            "install",
            "qfg1-ega",
            source.path().to_str().unwrap(),
            "--dest",
            dest.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    // Installed under the explicit --dest, not the default library dir.
    assert!(dest.path().join("qfg1-ega/SIERRA.BAT").is_file());
    assert!(!games.path().join("qfg1-ega").exists());
}

#[test]
fn install_rejects_unsupported_source_for_platform() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let adf = tmp.path().join("disk.adf");
    std::fs::write(&adf, b"x").unwrap();

    // A .adf is not a valid source for a DOS game.
    launcher(installs.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
        .args(["install", "qfg1-ega", adf.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("unsupported source"));
}

#[test]
fn declined_install_leaves_nothing_and_can_be_retried() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();

    // First attempt: source is missing the expected files; decline the prompt.
    let bad = tempfile::tempdir().unwrap();
    std::fs::write(bad.path().join("README.txt"), b"x").unwrap();
    launcher(installs.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
        .args(["install", "qfg1-ega", bad.path().to_str().unwrap()])
        .write_stdin("n\n")
        .assert()
        .failure();

    // Declining must not strand <games>/qfg1-ega (stage-then-commit).
    assert!(
        !games.path().join("qfg1-ega").exists(),
        "a declined install should leave no managed directory behind"
    );

    // Retry with a good source — must succeed, not hit DestinationOccupied.
    let good = dos_source_with_expected_files();
    launcher(installs.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
        .args(["install", "qfg1-ega", good.path().to_str().unwrap()])
        .assert()
        .success();
    assert!(games.path().join("qfg1-ega/SIERRA.BAT").is_file());
    assert!(installs.path().join("qfg1-ega.toml").is_file());
}

/// Build a throwaway repo whose tap has one DOS entry with `subdir = "INNER"`,
/// so we can exercise the subdir commit path without touching the shared
/// fixture tap. Returns the repo root (set as `RELIQUAINT_REPO_ROOT`).
fn temp_repo_with_subdir_entry() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let tap = root.path().join("tap");
    let dos = tap.join("catalog/dos");
    std::fs::create_dir_all(&dos).unwrap();
    std::fs::write(
        tap.join("tap.toml"),
        r#"schema_version = 1
id          = "test-tap"
title       = "Test Tap"
description = "test"
version     = "0.1.0"
maintainer  = "test"
url         = "https://example.test"
license     = "MIT"
"#,
    )
    .unwrap();
    std::fs::write(
        dos.join("subgame.toml"),
        r#"schema_version = 1
[game]
id = "subgame"
title = "Sub Game"
platform = "dos"
[install]
expects_files = ["GAME.EXE"]
subdir = "INNER"
[runtime]
emulator = "dosbox-staging"
[runtime.dosbox]
config = "subgame.conf"
entry = "GAME.EXE"
"#,
    )
    .unwrap();
    root
}

#[test]
fn forced_install_missing_subdir_fails_and_can_be_retried() {
    let root = temp_repo_with_subdir_entry();
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();

    // Source has GAME.EXE at the top level but NOT under the required INNER/.
    let bad = tempfile::tempdir().unwrap();
    std::fs::write(bad.path().join("GAME.EXE"), b"x").unwrap();

    // --force bypasses the expects_files prompt, but the missing subdir must
    // still fail the commit — and leave the final destination untouched.
    launcher(installs.path())
        .env("RELIQUAINT_REPO_ROOT", root.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
        .args([
            "install",
            "subgame",
            bad.path().to_str().unwrap(),
            "--force",
        ])
        .assert()
        .failure();
    assert!(
        !games.path().join("subgame").exists(),
        "a failed subdir install must not leave a committed directory"
    );
    assert!(!installs.path().join("subgame.toml").exists());

    // Retry with a good source (GAME.EXE under INNER/) — must succeed.
    let good = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(good.path().join("INNER")).unwrap();
    std::fs::write(good.path().join("INNER/GAME.EXE"), b"x").unwrap();
    launcher(installs.path())
        .env("RELIQUAINT_REPO_ROOT", root.path())
        .env("RELIQUAINT_GAMES_DIR", games.path())
        .args(["install", "subgame", good.path().to_str().unwrap()])
        .assert()
        .success();
    assert!(games.path().join("subgame/INNER/GAME.EXE").is_file());
    assert!(installs.path().join("subgame.toml").is_file());
}

// --- M4: manifest creation wizard ---

fn fixture_dos_source(dir: &Path, name: &str) -> PathBuf {
    let source = dir.join(name);
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("SIERRA.BAT"), "@echo off\nSIERRA.EXE\n").unwrap();
    std::fs::write(source.join("SIERRA.EXE"), [0u8; 4096]).unwrap();
    std::fs::write(source.join("RESOURCE.000"), [0u8; 4096]).unwrap();
    source
}

#[test]
fn add_writes_manifest_conf_and_install_record_for_dos_source() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let source = fixture_dos_source(games.path(), "my-custom-game");
    let user_tap = installs.path().join("user-tap");

    launcher(installs.path())
        .args(["add", source.to_str().unwrap(), "--yes"])
        .assert()
        .success();

    let manifest = user_tap.join("catalog/dos/my-custom-game.toml");
    let conf = user_tap.join("catalog/dos/my-custom-game.conf");
    let record = installs.path().join("my-custom-game.toml");
    assert!(manifest.is_file(), "manifest not written");
    assert!(conf.is_file(), "DOSBox conf not written");
    assert!(record.is_file(), "install record not written");

    // Generated conf has no [autoexec] block.
    let conf_text = std::fs::read_to_string(&conf).unwrap();
    assert!(!conf_text.to_ascii_lowercase().contains("[autoexec]"));

    // The new entry shows up in `list`, tagged with tap_id=local.
    let output = launcher(installs.path())
        .args(["list", "--format", "json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("my-custom-game"));
    assert!(
        stdout.contains("\"tap_id\":\"local\"") || stdout.contains("\"tap_id\": \"local\""),
        "user-tap entry missing tap_id=local: {stdout}"
    );
}

#[test]
fn run_dry_run_succeeds_after_add_for_dos_source() {
    // Round-trip: add → list shows it → run --dry-run composes the
    // launch command using the user-tap manifest + install record.
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let source = fixture_dos_source(games.path(), "my-custom-game");

    launcher(installs.path())
        .args(["add", source.to_str().unwrap(), "--yes"])
        .assert()
        .success();

    launcher(installs.path())
        .args(["run", "my-custom-game", "--dry-run"])
        .assert()
        .success();
}

#[test]
fn add_rejects_collision_with_bundled_id() {
    // qfg1-ega is a bundled fixture id. The wizard must refuse to
    // shadow it.
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let source = fixture_dos_source(games.path(), "qfg1-ega");

    launcher(installs.path())
        .args(["add", source.to_str().unwrap(), "--yes"])
        .assert()
        .failure()
        .stderr(contains("already used by the bundled catalog"));
}

#[test]
fn add_fails_when_platform_is_indeterminate() {
    // Empty source dir → unknown platform; the wizard should refuse
    // and point at --platform.
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let source = games.path().join("mystery-game");
    std::fs::create_dir_all(&source).unwrap();

    launcher(installs.path())
        .args(["add", source.to_str().unwrap(), "--yes"])
        .assert()
        .failure()
        .stderr(contains("platform could not be determined"));
}

#[test]
fn where_prints_paths_for_user_tap_entry() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let source = fixture_dos_source(games.path(), "my-custom-game");
    launcher(installs.path())
        .args(["add", source.to_str().unwrap(), "--yes"])
        .assert()
        .success();

    launcher(installs.path())
        .args(["where", "my-custom-game"])
        .assert()
        .success()
        .stdout(contains("user-tap entry my-custom-game"))
        .stdout(contains("catalog/dos/my-custom-game.toml"))
        .stdout(contains("catalog/dos/my-custom-game.conf"))
        .stdout(contains("my-custom-game.toml"));
}

#[test]
fn where_refuses_to_treat_bundled_entry_as_user_owned() {
    let installs = tempfile::tempdir().unwrap();
    launcher(installs.path())
        .args(["where", "qfg1-ega"])
        .assert()
        .success()
        .stdout(contains("bundled entry qfg1-ega"))
        .stdout(contains("not user-owned"));
}

#[test]
fn remove_cascades_user_entry_files() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let source = fixture_dos_source(games.path(), "my-custom-game");
    launcher(installs.path())
        .args(["add", source.to_str().unwrap(), "--yes"])
        .assert()
        .success();

    let manifest = installs
        .path()
        .join("user-tap/catalog/dos/my-custom-game.toml");
    let conf = installs
        .path()
        .join("user-tap/catalog/dos/my-custom-game.conf");
    let record = installs.path().join("my-custom-game.toml");
    assert!(manifest.is_file());

    launcher(installs.path())
        .args(["remove", "my-custom-game", "--force"])
        .assert()
        .success()
        .stdout(contains("removed my-custom-game"));

    assert!(!manifest.exists(), "manifest should be gone");
    assert!(!conf.exists(), "conf should be gone");
    assert!(!record.exists(), "install record should be gone");
}

#[test]
fn remove_refuses_bundled_entries() {
    let installs = tempfile::tempdir().unwrap();
    launcher(installs.path())
        .args(["remove", "qfg1-ega", "--force"])
        .assert()
        .failure()
        .stderr(contains("belongs to the bundled tap"));
}

// --- M6: upstream submission ---

#[test]
fn submit_emits_clean_manifest_and_next_steps_for_user_entry() {
    let installs = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let source = fixture_dos_source(games.path(), "my-custom-game");
    launcher(installs.path())
        .args(["add", source.to_str().unwrap(), "--yes"])
        .assert()
        .success();

    let output = launcher(installs.path())
        .args(["submit", "my-custom-game"])
        .output()
        .unwrap();
    assert!(output.status.success(), "submit failed");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    // Canonical manifest on stdout.
    assert!(stdout.contains("schema_version"));
    assert!(stdout.contains("my-custom-game"));
    assert!(stdout.contains("dosbox-staging"));

    // Submission instructions on stderr.
    assert!(stderr.contains("tap/catalog/dos/my-custom-game.toml"));
    assert!(stderr.contains("github.com/syraenix/reliquaint/new/develop"));
    // Bare-bones meta triggers completeness warnings.
    assert!(stderr.contains("[meta].year"));
    assert!(stderr.contains("[acquisition]"));
}

#[test]
fn submit_refuses_bundled_entries() {
    let installs = tempfile::tempdir().unwrap();
    launcher(installs.path())
        .args(["submit", "qfg1-ega"])
        .assert()
        .failure()
        .stderr(contains("belongs to the bundled tap"));
}

// ── Tap CLI subcommand integration tests ──────────────────────────────────────

/// Build a real local git repo with a valid tap structure.
/// Returns (bare_dir, work_dir, file_url) — keep bare_dir alive (TempDir).
fn make_local_tap_repo_with_entry(
    tap_id: &str,
    game_id: &str,
) -> (tempfile::TempDir, tempfile::TempDir, String) {
    use std::process::Command as Cmd;
    let bare = tempfile::tempdir().unwrap();
    Cmd::new("git")
        .args(["init", "--bare"])
        .arg(bare.path())
        .output()
        .unwrap();
    let work = tempfile::tempdir().unwrap();
    Cmd::new("git")
        .args(["clone"])
        .arg(bare.path())
        .arg(work.path())
        .output()
        .unwrap();
    for cfg in [("user.email", "test@test.com"), ("user.name", "Test")] {
        Cmd::new("git")
            .args(["-C"])
            .arg(work.path())
            .args(["config", cfg.0, cfg.1])
            .output()
            .unwrap();
    }
    let tap_toml = format!(
        "schema_version = 1\nid = \"{tap_id}\"\ntitle = \"Test Tap\"\ndescription = \"test\"\nversion = \"0.1.0\"\nmaintainer = \"test\"\nurl = \"https://example.com\"\nlicense = \"MIT\"\n"
    );
    std::fs::write(work.path().join("tap.toml"), tap_toml).unwrap();
    let catalog_dir = work.path().join("catalog/dos");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    let entry_toml = format!(
        "schema_version = 1\n[game]\nid = \"{game_id}\"\ntitle = \"Test Game\"\nplatform = \"dos\"\n[runtime]\nemulator = \"dosbox-staging\"\n[runtime.dosbox]\nconfig = \"{game_id}.conf\"\nentry = \"TEST.EXE\"\n"
    );
    std::fs::write(catalog_dir.join(format!("{game_id}.toml")), entry_toml).unwrap();
    Cmd::new("git")
        .args(["-C"])
        .arg(work.path())
        .args(["add", "."])
        .output()
        .unwrap();
    Cmd::new("git")
        .args(["-C"])
        .arg(work.path())
        .args(["commit", "-m", "init"])
        .output()
        .unwrap();
    Cmd::new("git")
        .args(["-C"])
        .arg(work.path())
        .args(["push"])
        .output()
        .unwrap();
    let url = format!("file://{}", bare.path().display());
    (bare, work, url)
}

/// Build a `launcher` command with extra tap env vars isolated to `dir`.
fn launcher_with_tap_env(installs_dir: &Path, subs_path: &Path, taps_cache: &Path) -> Command {
    let mut cmd = launcher(installs_dir);
    cmd.env("RELIQUAINT_SUBSCRIPTIONS_PATH", subs_path)
        .env("RELIQUAINT_TAPS_CACHE_DIR", taps_cache);
    cmd
}

#[test]
fn tap_list_shows_local_row_when_no_subscriptions() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "list"])
        .assert()
        .success()
        .stdout(contains("local"));
}

#[test]
fn tap_list_shows_subscribed_tap_with_ok_status() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    write_minimal_subscribed_tap(&taps_cache, "my-test-tap", "unique-game-9z", "Unique Game");
    write_subscriptions_toml(&sub_path, "my-test-tap", 0);
    let output = launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "list"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("my-test-tap"),
        "subscribed tap id missing: {stdout}"
    );
    assert!(stdout.contains("ok"), "expected ok status: {stdout}");
}

#[test]
fn tap_list_json_format_emits_valid_array() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    write_minimal_subscribed_tap(&taps_cache, "json-tap", "json-game", "JSON Game");
    write_subscriptions_toml(&sub_path, "json-tap", 0);
    let output = launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "list", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("output isn't valid JSON ({e}): {stdout}"));
    assert!(value.is_array(), "expected array: {stdout}");
    let arr = value.as_array().unwrap();
    // includes "local" plus one subscribed tap
    assert!(arr.len() >= 2, "expected at least 2 rows: {stdout}");
}

#[test]
fn tap_add_subscribes_and_clones_local_repo() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    let (_bare, _work, url) = make_local_tap_repo_with_entry("my-new-tap", "tap-game-1");
    let output = launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "add", &url])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        output.status.success(),
        "tap add failed; stderr={stderr}\nstdout={stdout}"
    );
    assert!(
        stdout.contains("subscribed"),
        "expected subscribed message: {stdout}"
    );
    // Cache should be present
    assert!(
        taps_cache.join("my-new-tap/tap.toml").exists(),
        "tap.toml not cloned"
    );
    // Subscription record written
    assert!(sub_path.exists(), "subscriptions.toml not written");
}

#[test]
fn tap_add_fails_gracefully_on_invalid_git_url() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    let output = launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "add", "file:///definitely/not/a/real/repo/xyz"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "expected failure for bad URL");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("clone") || stderr.contains("error") || stderr.contains("failed"),
        "expected error message: {stderr}"
    );
}

#[test]
fn tap_add_refuses_duplicate_subscription() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    let (_bare, _work, url) = make_local_tap_repo_with_entry("dup-tap", "dup-game");
    // First add succeeds
    launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "add", &url])
        .assert()
        .success();
    // Second add for same tap id must fail
    let (_bare2, _work2, url2) = make_local_tap_repo_with_entry("dup-tap", "dup-game");
    let output2 = launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "add", &url2])
        .output()
        .unwrap();
    assert!(!output2.status.success(), "second add should fail");
    let stderr2 = String::from_utf8(output2.stderr).unwrap();
    assert!(
        stderr2.contains("already subscribed")
            || stderr2.contains("duplicate")
            || stderr2.contains("dup-tap"),
        "expected duplicate error: {stderr2}"
    );
}

#[test]
fn tap_remove_removes_subscription_and_cache() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    let (_bare, _work, url) = make_local_tap_repo_with_entry("rm-tap", "rm-game");
    // Add first
    launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "add", &url])
        .assert()
        .success();
    assert!(taps_cache.join("rm-tap/tap.toml").exists());
    // Remove
    launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "remove", "rm-tap", "--force"])
        .assert()
        .success()
        .stdout(contains("unsubscribed"));
    // Cache should be gone
    assert!(!taps_cache.join("rm-tap").exists(), "cache not removed");
}

#[test]
fn tap_remove_refuses_local_id() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "remove", "local", "--force"])
        .assert()
        .failure()
        .stderr(contains("local"));
}

#[test]
fn tap_remove_warns_about_orphaned_installs_and_cancels_on_n() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    let (_bare, _work, url) = make_local_tap_repo_with_entry("orphan-tap", "orphan-game");
    launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "add", &url])
        .assert()
        .success();
    // Write a fake install record referencing this tap
    let install_body = format!(
        "schema_version = 1\n[install]\ncatalog_id = \"orphan-game\"\ntap = \"orphan-tap\"\ninstall_path = \"/fake/path\"\ninstalled_at = 2026-01-01T00:00:00Z\n"
    );
    std::fs::write(dir.path().join("orphan-game.toml"), install_body).unwrap();
    // Remove without --force, answer "n"
    launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "remove", "orphan-tap"])
        .write_stdin("n\n")
        .assert()
        .success(); // cancelled → success (not failure)
                    // Subscription still present (cancelled)
    assert!(
        sub_path.exists(),
        "subscription should still exist after cancel"
    );
    let content = std::fs::read_to_string(&sub_path).unwrap();
    assert!(
        content.contains("orphan-tap"),
        "subscription should still contain orphan-tap"
    );
}

#[test]
fn tap_sync_reports_up_to_date() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    let (_bare, _work, url) = make_local_tap_repo_with_entry("sync-tap", "sync-game");
    // Add first
    launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "add", &url])
        .assert()
        .success();
    // Sync — should succeed and say "up to date"
    let output = launcher_with_tap_env(dir.path(), &sub_path, &taps_cache)
        .args(["tap", "sync"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "tap sync failed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("up to date") || stdout.contains("updated") || stdout.contains("sync-tap"),
        "expected sync output: {stdout}"
    );
}

#[test]
fn tap_validate_succeeds_on_fixture_tap() {
    let installs = tempfile::tempdir().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tap");
    let output = launcher(installs.path())
        .args(["tap", "validate", fixture.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "tap validate failed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ok"), "expected ok: {stdout}");
}

#[test]
fn tap_validate_fails_on_dir_without_tap_toml() {
    let installs = tempfile::tempdir().unwrap();
    let empty = tempfile::tempdir().unwrap();
    let output = launcher(installs.path())
        .args(["tap", "validate", empty.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "validate should fail on empty dir"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("error") || stderr.contains("tap.toml") || stderr.contains("failed"),
        "expected error message: {stderr}"
    );
}

#[test]
fn doctor_shows_git_status() {
    let installs = tempfile::tempdir().unwrap();
    let output = launcher(installs.path()).arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("git"),
        "doctor should show git status: {stdout}"
    );
}

// ── Tap subscription integration tests ────────────────────────────────────────

fn write_minimal_subscribed_tap(taps_cache: &Path, tap_id: &str, game_id: &str, game_title: &str) {
    let tap_dir = taps_cache.join(tap_id);
    let catalog_dir = tap_dir.join("catalog/dos");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    std::fs::write(
        tap_dir.join("tap.toml"),
        format!(
            "schema_version = 1\nid = \"{tap_id}\"\ntitle = \"Test Tap\"\ndescription = \"test\"\nversion = \"0.1.0\"\nmaintainer = \"test\"\nurl = \"https://example.com\"\nlicense = \"MIT\"\n"
        ),
    )
    .unwrap();
    std::fs::write(
        catalog_dir.join(format!("{game_id}.toml")),
        format!(
            "schema_version = 1\n[game]\nid = \"{game_id}\"\ntitle = \"{game_title}\"\nplatform = \"dos\"\n[runtime]\nemulator = \"dosbox-staging\"\n[runtime.dosbox]\nconfig = \"{game_id}.conf\"\nentry = \"TEST.EXE\"\n"
        ),
    )
    .unwrap();
}

fn write_subscriptions_toml(path: &Path, tap_id: &str, priority: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!(
            "schema_version = 1\n\n[[tap]]\nid = \"{tap_id}\"\nsource = \"file:///fake\"\nadded_at = 2026-01-01T00:00:00Z\npriority = {priority}\n"
        ),
    )
    .unwrap();
}

#[test]
fn list_shows_games_from_subscribed_tap() {
    let dir = tempfile::tempdir().unwrap();
    let taps_cache = dir.path().join("taps");
    let sub_path = dir.path().join("subscriptions.toml");

    write_minimal_subscribed_tap(
        &taps_cache,
        "my-test-tap",
        "unique-subscribed-game",
        "Unique Subscribed Game Title",
    );
    write_subscriptions_toml(&sub_path, "my-test-tap", 0);

    launcher(dir.path())
        .env("RELIQUAINT_SUBSCRIPTIONS_PATH", &sub_path)
        .env("RELIQUAINT_TAPS_CACHE_DIR", &taps_cache)
        .arg("list")
        .assert()
        .success()
        .stdout(contains("unique-subscribed-game"));
}

#[test]
fn list_with_no_subscriptions_file_still_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    // No subscriptions.toml written — should load gracefully
    launcher(dir.path())
        .env(
            "RELIQUAINT_SUBSCRIPTIONS_PATH",
            dir.path().join("nonexistent-subscriptions.toml"),
        )
        .env("RELIQUAINT_TAPS_CACHE_DIR", dir.path().join("empty-taps"))
        .arg("list")
        .assert()
        .success();
}

#[test]
fn list_subscribed_tap_games_appear_alongside_bundled() {
    let dir = tempfile::tempdir().unwrap();
    let taps_cache = dir.path().join("taps");
    let sub_path = dir.path().join("subscriptions.toml");

    write_minimal_subscribed_tap(
        &taps_cache,
        "extra-tap",
        "extra-unique-game-9z",
        "Extra Game From Subscribed Tap",
    );
    write_subscriptions_toml(&sub_path, "extra-tap", 0);

    let output = launcher(dir.path())
        .env("RELIQUAINT_SUBSCRIPTIONS_PATH", &sub_path)
        .env("RELIQUAINT_TAPS_CACHE_DIR", &taps_cache)
        .args(["list", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Fixture games from bundled tap
    assert!(
        stdout.contains("qfg1-ega"),
        "bundled tap game missing: {stdout}"
    );
    // Game from subscribed tap
    assert!(
        stdout.contains("extra-unique-game-9z"),
        "subscribed tap game missing: {stdout}"
    );
}

#[test]
fn upgrade_suggests_tap_add_for_orphaned_installs_from_known_tap() {
    let dir = tempfile::tempdir().unwrap();
    let installs_dir = dir.path().join("installs");
    std::fs::create_dir_all(&installs_dir).unwrap();
    write_install_record_for_tap(
        &installs_dir,
        "qfg1-ega",
        "reliquaint-core",
        dir.path().join("games/qfg1-ega").to_str().unwrap(),
    );
    let sub_path = dir.path().join("subscriptions.toml");
    // No subscription to reliquaint-core
    std::fs::write(&sub_path, "schema_version = 1\n").unwrap();
    let taps_cache = dir.path().join("taps");

    launcher_with_tap_env(&installs_dir, &sub_path, &taps_cache)
        .arg("upgrade")
        .assert()
        .success()
        .stdout(contains("reliquaint-core"))
        .stdout(contains("tap add reliquaint-core"));
}

#[test]
fn upgrade_reports_nothing_when_all_taps_subscribed() {
    let dir = tempfile::tempdir().unwrap();
    let installs_dir = dir.path().join("installs");
    std::fs::create_dir_all(&installs_dir).unwrap();
    write_install_record_for_tap(
        &installs_dir,
        "qfg1-ega",
        "reliquaint-core",
        dir.path().join("games/qfg1-ega").to_str().unwrap(),
    );
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    // Already subscribed to reliquaint-core
    write_subscriptions_toml(&sub_path, "reliquaint-core", 0);

    launcher_with_tap_env(&installs_dir, &sub_path, &taps_cache)
        .arg("upgrade")
        .assert()
        .success()
        .stdout(contains("Nothing to upgrade"));
}

#[test]
fn upgrade_ignores_installs_from_unknown_taps() {
    let dir = tempfile::tempdir().unwrap();
    let installs_dir = dir.path().join("installs");
    std::fs::create_dir_all(&installs_dir).unwrap();
    // "some-random-tap" is not in the KNOWN_TAPS table
    write_install_record_for_tap(
        &installs_dir,
        "some-game",
        "some-random-tap",
        dir.path().join("games/some-game").to_str().unwrap(),
    );
    let sub_path = dir.path().join("subscriptions.toml");
    let taps_cache = dir.path().join("taps");
    std::fs::write(&sub_path, "schema_version = 1\n").unwrap();

    // Should report "nothing to upgrade" — unknown taps are not suggested
    launcher_with_tap_env(&installs_dir, &sub_path, &taps_cache)
        .arg("upgrade")
        .assert()
        .success()
        .stdout(contains("Nothing to upgrade"));
}
