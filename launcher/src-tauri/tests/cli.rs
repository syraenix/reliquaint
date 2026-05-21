use assert_cmd::Command;
use predicates::str::contains;
use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn launcher() -> Command {
    let mut cmd = Command::cargo_bin("classic-launcher").unwrap();
    cmd.env("CLASSIC_LAUNCHER_REPO_ROOT", fixture_root())
       .env("CLASSIC_LAUNCHER_GAMES_DIR", "/tmp/classic-launcher-no-games");
    cmd
}

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
    for id in ["kq1sci", "qfg1-ega"] {
        let d = temp.path().join(id);
        std::fs::create_dir(&d).unwrap();
        std::fs::write(d.join("game.exe"), b"").unwrap();
    }
    let output = launcher_with_games(temp.path()).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let ids: Vec<&str> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.split_whitespace().next().unwrap_or(""))
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "list output should be sorted by id");
    assert_eq!(ids.len(), 2);
}

#[test]
fn run_dry_run_prints_sidecar_and_primary() {
    launcher()
        .args(["run", "qfg1-ega", "--dry-run"])
        .assert()
        .success()
        .stdout(contains("[sidecar] fluidsynth"))
        .stdout(contains("[primary] flatpak"));
}

#[test]
fn run_dry_run_primary_contains_conf_path() {
    launcher()
        .args(["run", "qfg1-ega", "--dry-run"])
        .assert()
        .success()
        .stdout(contains("qfg1-ega.conf"));
}

#[test]
fn run_unknown_id_fails_with_message() {
    launcher()
        .args(["run", "nonexistent-game"])
        .assert()
        .failure()
        .stderr(contains("no manifest found"));
}

#[test]
fn run_kq1sci_dry_run_no_sidecar() {
    // kq1sci has no sidecars, so no [sidecar] line
    let output = launcher()
        .args(["run", "kq1sci", "--dry-run"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("[sidecar]"), "kq1sci has no sidecars");
    assert!(stdout.contains("[primary]"));
}

#[test]
fn doctor_runs_without_panic() {
    // Doctor exit code is host-dependent (0 if all installed, 2 if anything missing)
    // Just verify it exits cleanly and prints some output
    let output = launcher().arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("dosbox-staging"),
        "doctor should report dosbox-staging probe"
    );
    assert!(
        stdout.contains("fluidsynth"),
        "doctor should report fluidsynth probe"
    );
    let code = output.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 2, "doctor exits 0 or 2, got {code}");
}
