//! End-to-end self-update against a local stand-in for the GitHub releases
//! API: real HTTP, real tar.gz, real binary swap. No network, no mocks of the
//! updater itself.

use std::path::PathBuf;

use weaver_core::update::{Status, UpdateError, Updater};
use weaver_test_support::{InstalledBinary, ReleaseFixture, ReleaseServer};

const TARGET: &str = "aarch64-apple-darwin";
const REPO: &str = "acme/weaver";

#[tokio::test]
async fn reports_a_newer_release() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v9.9.9", TARGET)]).await;
    let updater = updater("0.1.0", &server, None);

    let status = updater.check().await.unwrap();

    assert_eq!(
        status,
        Status::Available {
            current: semver::Version::new(0, 1, 0),
            latest: semver::Version::new(9, 9, 9),
            url: "https://example.test/releases/v9.9.9".to_string(),
        }
    );
}

#[tokio::test]
async fn reports_up_to_date_when_running_the_latest_release() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v9.9.9", TARGET)]).await;
    let updater = updater("9.9.9", &server, None);

    assert_eq!(
        updater.check().await.unwrap(),
        Status::UpToDate {
            current: semver::Version::new(9, 9, 9)
        }
    );
}

#[tokio::test]
async fn installs_the_release_binary_over_the_running_one() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v9.9.9", TARGET)]).await;
    let install = InstalledBinary::new("0.1.0");
    let updater = updater("0.1.0", &server, Some(install.path()));

    let release = updater.latest_release().await.unwrap();
    let installed = updater.install(&release).await.unwrap();

    assert_eq!(installed, semver::Version::new(9, 9, 9));
    assert_eq!(install.reported_version(), "wvr 9.9.9");
    assert!(
        install.staging_leftovers().is_empty(),
        "staged files left behind: {:?}",
        install.staging_leftovers()
    );
}

#[tokio::test]
async fn a_tampered_archive_never_replaces_the_binary() {
    let server =
        ReleaseServer::start(
            &[ReleaseFixture::new(REPO, "v9.9.9", TARGET).with_tampered_archive()],
        )
        .await;
    let install = InstalledBinary::new("0.1.0");
    let updater = updater("0.1.0", &server, Some(install.path()));

    let release = updater.latest_release().await.unwrap();
    let err = updater.install(&release).await.unwrap_err();

    assert!(
        matches!(err, UpdateError::ChecksumMismatch { .. }),
        "unexpected error: {err}"
    );
    assert_eq!(install.reported_version(), "wvr 0.1.0");
}

#[tokio::test]
async fn a_release_without_a_checksum_is_refused() {
    let server =
        ReleaseServer::start(&[ReleaseFixture::new(REPO, "v9.9.9", TARGET).without_checksums()])
            .await;
    let install = InstalledBinary::new("0.1.0");
    let updater = updater("0.1.0", &server, Some(install.path()));

    let release = updater.latest_release().await.unwrap();
    let err = updater.install(&release).await.unwrap_err();

    assert!(
        matches!(err, UpdateError::MissingChecksum { .. }),
        "unexpected error: {err}"
    );
    assert_eq!(install.reported_version(), "wvr 0.1.0");
}

#[tokio::test]
async fn a_release_without_our_target_is_reported() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(
        REPO,
        "v9.9.9",
        "s390x-unknown-linux-gnu",
    )])
    .await;
    let install = InstalledBinary::new("0.1.0");
    let updater = updater("0.1.0", &server, Some(install.path()));

    let release = updater.latest_release().await.unwrap();
    let err = updater.install(&release).await.unwrap_err();

    assert!(
        matches!(&err, UpdateError::NoAsset { target, .. } if target == TARGET),
        "unexpected error: {err}"
    );
    assert_eq!(install.reported_version(), "wvr 0.1.0");
}

#[tokio::test]
async fn installing_a_specific_tag_bypasses_latest() {
    let server = ReleaseServer::start(&[
        ReleaseFixture::new(REPO, "v9.9.9", TARGET),
        ReleaseFixture::new(REPO, "v2.0.0", TARGET).not_latest(),
    ])
    .await;
    let install = InstalledBinary::new("0.1.0");
    let updater = updater("0.1.0", &server, Some(install.path()));

    let release = updater.release_by_tag("v2.0.0").await.unwrap();
    updater.install(&release).await.unwrap();

    assert_eq!(install.reported_version(), "wvr 2.0.0");
}

#[tokio::test]
async fn an_unknown_tag_is_not_found() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v9.9.9", TARGET)]).await;
    let updater = updater("0.1.0", &server, None);

    let err = updater.release_by_tag("v3.1.4").await.unwrap_err();

    assert!(
        matches!(&err, UpdateError::NotFound(what) if what == "v3.1.4"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn refreshing_the_check_state_persists_the_latest_version() {
    let server = ReleaseServer::start(&[ReleaseFixture::new(REPO, "v9.9.9", TARGET)]).await;
    let updater = updater("0.1.0", &server, None);
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("update-check.json");

    updater.refresh_check_state(&state_path).await.unwrap();

    let cached = weaver_core::update::check::UpdateCheck::load(&state_path).unwrap();
    assert_eq!(cached.latest_version, "9.9.9");
    assert_eq!(cached.html_url, "https://example.test/releases/v9.9.9");
    assert_eq!(
        cached.newer_than(&semver::Version::new(0, 1, 0)),
        Some(semver::Version::new(9, 9, 9))
    );
}

#[tokio::test]
async fn an_unreachable_release_api_is_an_error_not_a_panic() {
    // Port 1 on loopback: nothing listens there.
    let updater = Updater::from_env(semver::Version::new(0, 1, 0))
        .unwrap()
        .with_api_base("http://127.0.0.1:1")
        .with_repo(REPO)
        .unwrap()
        .with_target(TARGET);

    let err = updater.check().await.unwrap_err();

    assert!(
        matches!(err, UpdateError::Network { .. }),
        "unexpected error: {err}"
    );
}

fn updater(current: &str, server: &ReleaseServer, install_path: Option<PathBuf>) -> Updater {
    let mut updater = Updater::from_env(semver::Version::parse(current).unwrap())
        .unwrap()
        .with_api_base(server.base_url())
        .with_repo(REPO)
        .unwrap()
        .with_target(TARGET);

    if let Some(path) = install_path {
        updater = updater.with_install_path(path);
    }

    updater
}
