use assert_cmd::Command;
use predicates::str::contains;
use std::path::{Path, PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Build a `reliquaint` invocation pointed at the fixture tap with an
/// isolated installs directory. The caller may pre-populate the
/// installs dir to set up "installed" entries.
fn launcher(installs_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("reliquaint").unwrap();
    cmd.env("RELIQUAINT_REPO_ROOT", fixture_root())
        .env("RELIQUAINT_INSTALLS_DIR", installs_dir);
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
