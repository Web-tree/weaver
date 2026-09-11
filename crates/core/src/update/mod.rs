//! Self-update: discover GitHub releases, verify them, and replace the running
//! binary.
//!
//! Release assets are produced by `.github/workflows/release.yml`; the naming
//! contract lives in [`release`]. Every download is checked against the
//! published SHA-256 digest and smoke-tested before it can replace a working
//! install.

pub mod check;
pub mod install;
pub mod release;

use std::path::{Path, PathBuf};
use std::time::Duration;

use release::{BUILD_TARGET, Release};

/// Default GitHub API host; overridable for tests and GitHub Enterprise.
pub const DEFAULT_API_BASE: &str = "https://api.github.com";

/// Point the updater at a different repository (`owner/repo`).
pub const REPO_ENV: &str = "WVR_UPDATE_REPO";
/// Point the updater at a different GitHub API base URL.
pub const API_BASE_ENV: &str = "WVR_UPDATE_API_BASE";
/// Token used for release lookups (helps with API rate limits).
pub const TOKEN_ENV: &str = "WVR_UPDATE_TOKEN";
/// Download assets for a different target triple than this build's.
pub const TARGET_ENV: &str = "WVR_UPDATE_TARGET";

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("{0}")]
    Config(String),

    #[error("cannot reach {url}: {message}")]
    Network { url: String, message: String },

    #[error("no release found for {0}")]
    NotFound(String),

    #[error("release {tag} has no prebuilt binary for {target}")]
    NoAsset { target: String, tag: String },

    #[error("release does not publish a checksum for {asset}")]
    MissingChecksum { asset: String },

    #[error("checksum for {asset} is not a sha256 digest: {value:?}")]
    MalformedChecksum { asset: String, value: String },

    #[error("checksum mismatch for {asset}: expected {expected}, downloaded {actual}")]
    ChecksumMismatch {
        asset: String,
        expected: String,
        actual: String,
    },

    #[error("invalid version {value:?}: {source}")]
    InvalidVersion {
        value: String,
        source: semver::Error,
    },

    #[error("{0}")]
    Archive(String),

    #[error("downloaded binary is unusable: {reason}")]
    Unusable { reason: String },

    #[error("downloaded binary reports {actual}, expected {expected}")]
    VersionMismatch { expected: String, actual: String },

    #[error("cannot write to {path}: re-run with elevated permissions or reinstall {binary}")]
    NotWritable { path: String, binary: String },

    #[error("cannot install to {path}: {source}")]
    Install {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    UpToDate {
        current: semver::Version,
    },
    Available {
        current: semver::Version,
        latest: semver::Version,
        url: String,
    },
}

/// Talks to GitHub releases and installs them.
pub struct Updater {
    http: reqwest::Client,
    api_base: String,
    repo: String,
    token: Option<String>,
    target: String,
    current: semver::Version,
    install_path: Option<PathBuf>,
}

impl Updater {
    /// Build an updater for `current_version`, reading repository and endpoint
    /// overrides from the environment.
    pub fn from_env(current_version: semver::Version) -> Result<Self, UpdateError> {
        let repo = match env_non_empty(REPO_ENV) {
            Some(repo) => normalize_repo(&repo)?,
            None => normalize_repo(env!("CARGO_PKG_REPOSITORY"))?,
        };

        let api_base = env_non_empty(API_BASE_ENV).unwrap_or_else(|| DEFAULT_API_BASE.to_string());

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent(format!("wvr/{current_version}"))
            .build()
            .map_err(|e| UpdateError::Config(format!("cannot build HTTP client: {e}")))?;

        Ok(Self {
            http,
            api_base: api_base.trim_end_matches('/').to_string(),
            repo,
            token: env_non_empty(TOKEN_ENV).or_else(|| env_non_empty("GITHUB_TOKEN")),
            target: env_non_empty(TARGET_ENV).unwrap_or_else(|| BUILD_TARGET.to_string()),
            current: current_version,
            install_path: None,
        })
    }

    /// Override the GitHub API endpoint (GitHub Enterprise, tests).
    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into().trim_end_matches('/').to_string();
        self
    }

    /// Override the repository releases are read from.
    pub fn with_repo(mut self, repo: &str) -> Result<Self, UpdateError> {
        self.repo = normalize_repo(repo)?;
        Ok(self)
    }

    /// Override the target triple used to pick a release asset.
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = target.into();
        self
    }

    /// Override the binary that gets replaced (defaults to the running one).
    pub fn with_install_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.install_path = Some(path.into());
        self
    }

    pub fn current_version(&self) -> &semver::Version {
        &self.current
    }

    pub fn repo(&self) -> &str {
        &self.repo
    }

    /// Binary that an install would replace.
    pub fn install_path(&self) -> Result<PathBuf, UpdateError> {
        match &self.install_path {
            Some(path) => Ok(path.clone()),
            None => install::current_binary_path(),
        }
    }

    /// Latest non-prerelease release.
    pub async fn latest_release(&self) -> Result<Release, UpdateError> {
        let url = format!("{}/repos/{}/releases/latest", self.api_base, self.repo);
        self.get_release(&url, "latest").await
    }

    /// A specific release by tag (`v0.2.0`).
    pub async fn release_by_tag(&self, tag: &str) -> Result<Release, UpdateError> {
        let url = format!(
            "{}/repos/{}/releases/tags/{}",
            self.api_base,
            self.repo,
            urlencoding::encode(tag)
        );
        self.get_release(&url, tag).await
    }

    /// Compare the latest release against the running version.
    pub async fn check(&self) -> Result<Status, UpdateError> {
        let release = self.latest_release().await?;
        self.status_for(&release)
    }

    /// Classify a release against the running version.
    pub fn status_for(&self, release: &Release) -> Result<Status, UpdateError> {
        let latest = release.version()?;

        if latest <= self.current {
            return Ok(Status::UpToDate {
                current: self.current.clone(),
            });
        }

        Ok(Status::Available {
            current: self.current.clone(),
            latest,
            url: release.html_url.clone(),
        })
    }

    /// Download, verify and install `release`, returning the installed version.
    ///
    /// Fails without touching the existing binary if the download does not
    /// match the published checksum or does not run.
    pub async fn install(&self, release: &Release) -> Result<semver::Version, UpdateError> {
        let version = release.version()?;
        let asset = release.asset_for(&self.target)?;
        let checksum_asset = release.checksum_for(asset)?;

        let archive = self.download(&asset.browser_download_url).await?;
        let checksums = self.download(&checksum_asset.browser_download_url).await?;
        let checksums = String::from_utf8_lossy(&checksums);

        let expected = release::digest_for(&checksums, &asset.name)?;
        let actual = crate::state::calculate_checksum_from_bytes(&archive);
        if actual != expected {
            return Err(UpdateError::ChecksumMismatch {
                asset: asset.name.clone(),
                expected,
                actual,
            });
        }

        let binary = install::extract_binary(&archive)?;
        let path = self.install_path()?;
        install::replace_binary(&path, &binary, &version)?;

        // The CLI prints the user-facing summary; keep the log channel clean.
        tracing::debug!(
            version = %version,
            path = %path.display(),
            "installed new wvr release"
        );

        Ok(version)
    }

    /// Refresh the cached check state used by the background notice.
    pub async fn refresh_check_state(
        &self,
        path: &Path,
    ) -> Result<check::UpdateCheck, UpdateError> {
        let release = self.latest_release().await?;
        let state =
            check::UpdateCheck::new(&release.version()?, &release.html_url, check::now_unix());
        state.save(path)?;
        Ok(state)
    }

    async fn get_release(&self, url: &str, what: &str) -> Result<Release, UpdateError> {
        let response = self
            .request(url)
            .send()
            .await
            .map_err(|e| network_error(url, e))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(UpdateError::NotFound(what.to_string()));
        }
        let response = self.ensure_success(url, response)?;

        response
            .json::<Release>()
            .await
            .map_err(|e| UpdateError::Config(format!("unexpected release payload from {url}: {e}")))
    }

    /// GET with a small retry budget: release downloads are large enough that a
    /// single transient failure should not abort an update.
    async fn download(&self, url: &str) -> Result<Vec<u8>, UpdateError> {
        const ATTEMPTS: u32 = 3;
        let mut last_error = None;

        for attempt in 0..ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(200 * 2_u64.pow(attempt))).await;
            }

            match self.try_download(url).await {
                Ok(bytes) => return Ok(bytes),
                Err(e) => {
                    tracing::debug!(url, attempt, error = %e, "release download failed");
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| UpdateError::Network {
            url: url.to_string(),
            message: "download failed".to_string(),
        }))
    }

    async fn try_download(&self, url: &str) -> Result<Vec<u8>, UpdateError> {
        let response = self
            .request(url)
            .send()
            .await
            .map_err(|e| network_error(url, e))?;
        let response = self.ensure_success(url, response)?;

        let bytes = response.bytes().await.map_err(|e| network_error(url, e))?;

        Ok(bytes.to_vec())
    }

    fn request(&self, url: &str) -> reqwest::RequestBuilder {
        let request = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");

        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    fn ensure_success(
        &self,
        url: &str,
        response: reqwest::Response,
    ) -> Result<reqwest::Response, UpdateError> {
        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let message = if status == reqwest::StatusCode::FORBIDDEN
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            format!("HTTP {status} (GitHub API rate limit? set {TOKEN_ENV})")
        } else {
            format!("HTTP {status}")
        };

        Err(UpdateError::Network {
            url: url.to_string(),
            message,
        })
    }
}

fn network_error(url: &str, error: reqwest::Error) -> UpdateError {
    UpdateError::Network {
        url: url.to_string(),
        message: error.to_string(),
    }
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Accept `owner/repo`, `https://github.com/owner/repo(.git)` and
/// `git@github.com:owner/repo(.git)`.
fn normalize_repo(value: &str) -> Result<String, UpdateError> {
    let value = value.trim().trim_end_matches('/');
    let value = value.strip_suffix(".git").unwrap_or(value);

    let path = value
        .split_once("github.com")
        .map(|(_, rest)| rest.trim_start_matches([':', '/']))
        .unwrap_or(value);

    let mut parts = path.split('/').filter(|part| !part.is_empty());
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) => Ok(format!("{owner}/{repo}")),
        _ => Err(UpdateError::Config(format!(
            "cannot derive a GitHub repository from {value:?}; set {REPO_ENV}=owner/repo"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_urls_normalize_to_owner_repo() {
        for input in [
            "Web-tree/weaver",
            "https://github.com/Web-tree/weaver",
            "https://github.com/Web-tree/weaver.git",
            "https://github.com/Web-tree/weaver/",
            "git@github.com:Web-tree/weaver.git",
        ] {
            assert_eq!(normalize_repo(input).unwrap(), "Web-tree/weaver", "{input}");
        }
    }

    #[test]
    fn unusable_repository_values_are_rejected() {
        for input in ["", "weaver", "https://example.com/a/b/c"] {
            assert!(
                normalize_repo(input).is_err(),
                "{input:?} should be invalid"
            );
        }
    }

    /// The packaged crate metadata must be enough to find releases, so a stock
    /// binary can self-update with no configuration.
    #[test]
    fn crate_metadata_resolves_to_the_release_repository() {
        assert_eq!(
            normalize_repo(env!("CARGO_PKG_REPOSITORY")).unwrap(),
            "Web-tree/weaver"
        );
    }

    #[test]
    fn status_ignores_older_and_equal_releases() {
        let updater = Updater::from_env(semver::Version::new(0, 2, 0)).unwrap();

        let older = Release {
            tag_name: "v0.1.0".to_string(),
            html_url: String::new(),
            prerelease: false,
            assets: Vec::new(),
        };
        let same = Release {
            tag_name: "v0.2.0".to_string(),
            ..older.clone()
        };
        let newer = Release {
            tag_name: "v0.3.0".to_string(),
            html_url: "https://example.test/v0.3.0".to_string(),
            ..older.clone()
        };

        assert_eq!(
            updater.status_for(&older).unwrap(),
            Status::UpToDate {
                current: semver::Version::new(0, 2, 0)
            }
        );
        assert_eq!(
            updater.status_for(&same).unwrap(),
            Status::UpToDate {
                current: semver::Version::new(0, 2, 0)
            }
        );
        assert_eq!(
            updater.status_for(&newer).unwrap(),
            Status::Available {
                current: semver::Version::new(0, 2, 0),
                latest: semver::Version::new(0, 3, 0),
                url: "https://example.test/v0.3.0".to_string(),
            }
        );
    }
}
