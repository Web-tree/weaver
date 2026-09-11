//! `wvr self-update` and the background release check, against a local
//! stand-in for the GitHub releases API.

use std::path::{Path, PathBuf};
use std::process::Command;

use weaver_test_support::{ReleaseFixture, ReleaseServer};

const REPO: &str = "acme/weaver";
const TARGET: &str = "test-target";

/// Version the binary under test reports.
fn own_version() -> String {
    format!("wvr {}", env!("CARGO_PKG_VERSION"))
}

#[test]
fn version_flag_reports_the_crate_version() {
    let output = Command::new(wvr_bin())
        .arg("--version")
        .env("WVR_NO_UPDATE_CHECK", "1")
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        own_version()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_reports_an_available_release_without_installing() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v99.0.0", TARGET)]).await;
    let home = tempfile::tempdir().unwrap();
    let installed = install_copy(&home.path().join("bin"));

    let output = self_update(&installed, &server, home.path())
        .arg("--check")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        stdout.contains(&format!("wvr {} -> 99.0.0", env!("CARGO_PKG_VERSION"))),
        "{stdout}"
    );
    assert!(
        stdout.contains("https://example.test/releases/v99.0.0"),
        "{stdout}"
    );
    assert_eq!(reported_version(&installed), own_version());
}

/// `self-update` owns the release lifecycle: the background auto-updater must
/// not install anything behind it, least of all during `--check`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_installs_nothing_even_with_auto_update_enabled() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v99.0.0", TARGET)]).await;
    let home = tempfile::tempdir().unwrap();
    let installed = install_copy(&home.path().join("bin"));

    let output = self_update(&installed, &server, home.path())
        .arg("--check")
        .env("WVR_AUTO_UPDATE", "1")
        .env("WVR_UPDATE_INTERVAL_HOURS", "0")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(reported_version(&installed), own_version());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_reports_up_to_date_for_an_older_release() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v0.0.1", TARGET)]).await;
    let home = tempfile::tempdir().unwrap();
    let installed = install_copy(&home.path().join("bin"));

    let output = self_update(&installed, &server, home.path())
        .arg("--check")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("is up to date"), "{stdout}");
}

/// The real CLI downloading a release and replacing its own executable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_update_replaces_the_running_executable() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v99.0.0", TARGET)]).await;
    let home = tempfile::tempdir().unwrap();
    let installed = install_copy(&home.path().join("bin"));

    let output = self_update(&installed, &server, home.path())
        .arg("--yes")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(reported_version(&installed), "wvr 99.0.0");

    // The installed version is recorded so the notice stops firing.
    let state = std::fs::read_to_string(home.path().join("update-check.json")).unwrap();
    assert!(state.contains("99.0.0"), "{state}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn installing_a_specific_tag_is_possible() {
    let server = ReleaseServer::start(&[
        ReleaseFixture::new(REPO, "v99.0.0", TARGET),
        ReleaseFixture::new(REPO, "v50.0.0", TARGET).not_latest(),
    ])
    .await;
    let home = tempfile::tempdir().unwrap();
    let installed = install_copy(&home.path().join("bin"));

    let output = self_update(&installed, &server, home.path())
        .args(["--tag", "v50.0.0", "--yes"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(reported_version(&installed), "wvr 50.0.0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tampered_release_fails_without_touching_the_binary() {
    let server = ReleaseServer::start(&[
        ReleaseFixture::new(REPO, "v99.0.0", TARGET).with_tampered_archive()
    ])
    .await;
    let home = tempfile::tempdir().unwrap();
    let installed = install_copy(&home.path().join("bin"));

    let output = self_update(&installed, &server, home.path())
        .arg("--yes")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected failure: {stderr}");
    assert!(stderr.contains("checksum mismatch"), "{stderr}");
    assert_eq!(reported_version(&installed), own_version());
}

/// An ordinary command refreshes the cached check in the background.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn background_check_caches_the_latest_release() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v99.0.0", TARGET)]).await;
    let home = tempfile::tempdir().unwrap();
    let project = project_dir();

    let output = wvr_list(&wvr_bin(), &server, home.path(), project.path())
        // Scripted output would skip the check; ask for it explicitly.
        .env("WVR_UPDATE_CHECK", "1")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state = std::fs::read_to_string(home.path().join("update-check.json")).unwrap();
    assert!(state.contains("\"latest_version\": \"99.0.0\""), "{state}");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Update available"), "{stderr}");
    assert!(stderr.contains("self-update"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn opting_out_skips_the_background_check() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v99.0.0", TARGET)]).await;
    let home = tempfile::tempdir().unwrap();
    let project = project_dir();

    let status = wvr_list(&wvr_bin(), &server, home.path(), project.path())
        .env("WVR_UPDATE_CHECK", "1")
        .env("WVR_NO_UPDATE_CHECK", "1")
        .status()
        .unwrap();

    assert!(status.success());
    assert!(
        !home.path().join("update-check.json").exists(),
        "check ran despite WVR_NO_UPDATE_CHECK"
    );
}

/// Machine-readable output must stay parseable: no notice, no check.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_output_never_gets_an_update_notice() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v99.0.0", TARGET)]).await;
    let home = tempfile::tempdir().unwrap();
    let project = project_dir();

    let output = wvr_list(&wvr_bin(), &server, home.path(), project.path())
        .arg("--json")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(!stderr.contains("Update available"), "{stderr}");
    assert!(!home.path().join("update-check.json").exists(), "{stderr}");
}

/// `WVR_AUTO_UPDATE=1` installs the release instead of only reporting it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_update_installs_without_being_asked() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v99.0.0", TARGET)]).await;
    let home = tempfile::tempdir().unwrap();
    let project = project_dir();
    let installed = install_copy(&home.path().join("bin"));

    let output = wvr_list(&installed, &server, home.path(), project.path())
        .env("WVR_AUTO_UPDATE", "1")
        .env("WVR_UPDATE_INTERVAL_HOURS", "0")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("Auto-updated"), "{stderr}");
    assert_eq!(reported_version(&installed), "wvr 99.0.0");
}

fn self_update(binary: &Path, server: &ReleaseServer, home: &Path) -> Command {
    let mut command = Command::new(binary);
    command.arg("self-update");
    release_env(&mut command, server, home);
    command
}

fn wvr_list(binary: &Path, server: &ReleaseServer, home: &Path, project: &Path) -> Command {
    let mut command = Command::new(binary);
    command.arg("list").current_dir(project);
    release_env(&mut command, server, home);
    command
}

fn release_env(command: &mut Command, server: &ReleaseServer, home: &Path) {
    command
        .env("WVR_HOME", home)
        .env("WVR_UPDATE_API_BASE", server.base_url())
        .env("WVR_UPDATE_REPO", REPO)
        .env("WVR_UPDATE_TARGET", TARGET)
        .env_remove("WVR_NO_UPDATE_CHECK")
        .env_remove("WVR_AUTO_UPDATE")
        .env_remove("CI");
}

/// Path of the `wvr` binary under test.
fn wvr_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wvr"))
}

/// Copy the binary under test into `dir` so a self-update can replace it.
fn install_copy(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("wvr");
    std::fs::copy(wvr_bin(), &path).unwrap();
    path
}

/// A project `wvr list` can run in.
fn project_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("weaver.yaml"),
        "version: \"1\"\nmodules: []\napps: []\n",
    )
    .unwrap();
    dir
}

fn reported_version(binary: &Path) -> String {
    let output = Command::new(binary)
        .arg("--version")
        .env("WVR_NO_UPDATE_CHECK", "1")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
