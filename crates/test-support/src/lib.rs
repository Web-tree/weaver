//! In-process stand-in for the GitHub releases API, used by the self-update
//! tests.
//!
//! It serves exactly what the updater consumes — a release payload, a
//! `.tar.gz` archive and a `SHA256SUMS` manifest — over real HTTP on
//! loopback, so self-update can be exercised end to end without network
//! access.
//!
//! The release layout is spelled out here rather than imported from
//! `weaver-core`, so these fixtures independently pin the contract that
//! `.github/workflows/release.yml` publishes.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Executable name inside a release archive.
pub const BIN_NAME: &str = "wvr";

/// Aggregate checksum manifest published with every release.
pub const CHECKSUM_MANIFEST: &str = "SHA256SUMS";

/// Release archive name, as published by the release workflow.
pub fn asset_name(tag: &str, target: &str) -> String {
    format!("{BIN_NAME}-{tag}-{target}.tar.gz")
}

/// A release as `.github/workflows/release.yml` publishes it.
pub struct ReleaseFixture {
    tag: String,
    archive_name: String,
    archive: Vec<u8>,
    checksums: Option<String>,
    is_latest: bool,
    repo: String,
}

impl ReleaseFixture {
    /// A release for `tag` carrying one archive for `target`, its checksum
    /// manifest, and a binary that reports `tag`'s version.
    pub fn new(repo: &str, tag: &str, target: &str) -> Self {
        let version = tag.trim_start_matches('v');
        let archive = release_tarball(&fake_wvr_binary(version));
        let archive_name = asset_name(tag, target);
        let checksums = format!("{}  {archive_name}\n", sha256_hex(&archive));

        Self {
            tag: tag.to_string(),
            archive_name,
            archive,
            checksums: Some(checksums),
            is_latest: true,
            repo: repo.to_string(),
        }
    }

    /// Serve archive bytes that do not match the published digest.
    pub fn with_tampered_archive(mut self) -> Self {
        self.archive = release_tarball(&fake_wvr_binary("6.6.6"));
        self
    }

    /// Publish the release without any checksum asset.
    pub fn without_checksums(mut self) -> Self {
        self.checksums = None;
        self
    }

    /// Serve this release only under its tag, not as `releases/latest`.
    pub fn not_latest(mut self) -> Self {
        self.is_latest = false;
        self
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Release page URL the fixture reports.
    pub fn html_url(&self) -> String {
        format!("https://example.test/releases/{}", self.tag)
    }

    /// Routes this release contributes to a [`ReleaseServer`].
    pub fn routes(&self) -> HashMap<String, Route> {
        let mut routes = HashMap::new();
        let download = |name: &str| format!("/download/{}/{name}", self.tag);
        let url = |name: &str| {
            format!(
                "http://{}{}",
                ReleaseServer::HOST_PLACEHOLDER,
                download(name)
            )
        };

        let mut assets = vec![serde_json::json!({
            "name": self.archive_name,
            "browser_download_url": url(&self.archive_name),
        })];
        routes.insert(
            download(&self.archive_name),
            Route::new("application/gzip", self.archive.clone()),
        );

        if let Some(checksums) = &self.checksums {
            assets.push(serde_json::json!({
                "name": CHECKSUM_MANIFEST,
                "browser_download_url": url(CHECKSUM_MANIFEST),
            }));
            routes.insert(
                download(CHECKSUM_MANIFEST),
                Route::new("text/plain", checksums.clone().into_bytes()),
            );
        }

        let payload = serde_json::json!({
            "tag_name": self.tag,
            "html_url": self.html_url(),
            "prerelease": false,
            "assets": assets,
        })
        .to_string();

        let release = Route::new("application/json", payload.into_bytes());
        routes.insert(
            format!("/repos/{}/releases/tags/{}", self.repo, self.tag),
            release.clone(),
        );
        if self.is_latest {
            routes.insert(format!("/repos/{}/releases/latest", self.repo), release);
        }

        routes
    }
}

/// A canned HTTP response.
#[derive(Clone)]
pub struct Route {
    content_type: String,
    body: Vec<u8>,
}

impl Route {
    pub fn new(content_type: &str, body: Vec<u8>) -> Self {
        Self {
            content_type: content_type.to_string(),
            body,
        }
    }
}

/// Minimal HTTP/1.1 server answering release lookups and asset downloads.
pub struct ReleaseServer {
    addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl ReleaseServer {
    /// Placeholder in fixture payloads, replaced with the live address so
    /// asset URLs point back at this server.
    const HOST_PLACEHOLDER: &'static str = "{host}";

    /// Serve one or more releases.
    pub async fn start(fixtures: &[ReleaseFixture]) -> Self {
        let mut routes = HashMap::new();
        for fixture in fixtures {
            routes.extend(fixture.routes());
        }
        Self::with_routes(routes).await
    }

    pub async fn with_routes(routes: HashMap<String, Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let host = addr.to_string();

        let routes: HashMap<String, Route> = routes
            .into_iter()
            .map(|(path, mut route)| {
                if route.content_type == "application/json" {
                    let body =
                        String::from_utf8_lossy(&route.body).replace(Self::HOST_PLACEHOLDER, &host);
                    route.body = body.into_bytes();
                }
                (path, route)
            })
            .collect();
        let routes = Arc::new(routes);

        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let routes = Arc::clone(&routes);
                tokio::spawn(async move {
                    let _ = serve(stream, routes).await;
                });
            }
        });

        Self { addr, task }
    }

    /// Base URL to pass as the GitHub API endpoint.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for ReleaseServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(mut stream: TcpStream, routes: Arc<HashMap<String, Route>>) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buf = [0_u8; 1024];

    // Requests are header-only GETs; read until the end of the header block.
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        let read = stream.read(&mut buf).await?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buf[..read]);
    }

    let head = String::from_utf8_lossy(&request);
    let path = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string();

    let response = match routes.get(&path) {
        Some(route) => {
            let mut response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                route.content_type,
                route.body.len()
            )
            .into_bytes();
            response.extend_from_slice(&route.body);
            response
        }
        None => {
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
        }
    };

    stream.write_all(&response).await?;
    stream.flush().await
}

/// Stand-in for a released `wvr`: an executable that answers `--version` the
/// way the real CLI does.
pub fn fake_wvr_binary(version: &str) -> Vec<u8> {
    format!("#!/bin/sh\necho \"{BIN_NAME} {version}\"\n").into_bytes()
}

/// Pack `binary` the way the release workflow does.
pub fn release_tarball(binary: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    {
        let mut builder = tar::Builder::new(&mut encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(binary.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder
            .append_data(&mut header, BIN_NAME, binary)
            .expect("append binary");
        builder.finish().expect("finish tar");
    }
    encoder.finish().expect("finish gzip")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// An installed `wvr` in a scratch directory, for exercising the swap.
pub struct InstalledBinary {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl InstalledBinary {
    pub fn new(version: &str) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(BIN_NAME);

        let mut file = fs::File::create(&path).expect("create binary");
        file.write_all(&fake_wvr_binary(version))
            .expect("write binary");
        drop(file);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        }

        Self { _dir: dir, path }
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// What the installed binary reports for `--version`.
    pub fn reported_version(&self) -> String {
        let output = std::process::Command::new(&self.path)
            .arg("--version")
            .output()
            .expect("run installed binary");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Files in the install directory other than the binary itself.
    pub fn staging_leftovers(&self) -> Vec<String> {
        let dir: &Path = self.path.parent().expect("install dir");
        fs::read_dir(dir)
            .expect("read install dir")
            .map(|entry| {
                entry
                    .expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name != BIN_NAME)
            .collect()
    }
}
