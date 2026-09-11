//! Release metadata: asset naming, version parsing and checksum manifests.
//!
//! Everything here is pure so the naming contract between the release workflow
//! and the updater is testable without network access.

use serde::Deserialize;

use super::UpdateError;

/// Target triple this binary was built for, baked in by `build.rs`.
pub const BUILD_TARGET: &str = env!("WVR_BUILD_TARGET");

/// Executable name inside a release archive.
pub const BIN_NAME: &str = "wvr";

/// Aggregate checksum manifest published alongside the per-asset files.
pub const CHECKSUM_MANIFEST: &str = "SHA256SUMS";

/// Asset name published by `.github/workflows/release.yml`.
///
/// `wvr-v0.2.0-aarch64-apple-darwin.tar.gz`
pub fn asset_name(tag: &str, target: &str) -> String {
    format!("{BIN_NAME}-{tag}-{target}.tar.gz")
}

/// Suffix shared by every archive for a given target, used as a fallback when
/// the tag in the asset name does not match the release tag exactly.
fn asset_suffix(target: &str) -> String {
    format!("-{target}.tar.gz")
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// Subset of the GitHub release payload the updater relies on.
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

impl Release {
    /// Semantic version of the release, tolerating the `v` tag prefix.
    pub fn version(&self) -> Result<semver::Version, UpdateError> {
        parse_version(&self.tag_name)
    }

    /// Archive for `target`: exact workflow-generated name first, then any
    /// archive with the target's suffix.
    pub fn asset_for(&self, target: &str) -> Result<&ReleaseAsset, UpdateError> {
        let exact = asset_name(&self.tag_name, target);
        let suffix = asset_suffix(target);

        self.assets
            .iter()
            .find(|asset| asset.name == exact)
            .or_else(|| {
                self.assets
                    .iter()
                    .find(|asset| asset.name.ends_with(&suffix))
            })
            .ok_or_else(|| UpdateError::NoAsset {
                target: target.to_string(),
                tag: self.tag_name.clone(),
            })
    }

    /// Checksum asset for `archive`: the per-asset `.sha256` file if present,
    /// otherwise the aggregate manifest.
    pub fn checksum_for(&self, archive: &ReleaseAsset) -> Result<&ReleaseAsset, UpdateError> {
        let per_asset = format!("{}.sha256", archive.name);

        self.assets
            .iter()
            .find(|asset| asset.name == per_asset)
            .or_else(|| {
                self.assets
                    .iter()
                    .find(|asset| asset.name == CHECKSUM_MANIFEST)
            })
            .ok_or_else(|| UpdateError::MissingChecksum {
                asset: archive.name.clone(),
            })
    }
}

/// Parse a release tag (`v1.2.3` or `1.2.3`) into a semantic version.
pub fn parse_version(tag: &str) -> Result<semver::Version, UpdateError> {
    let trimmed = tag.trim();
    let value = trimmed.strip_prefix('v').unwrap_or(trimmed);

    semver::Version::parse(value).map_err(|source| UpdateError::InvalidVersion {
        value: tag.to_string(),
        source,
    })
}

/// Extract the digest for `asset_name` from a checksum file.
///
/// Accepts both `sha256sum` style manifests (`<digest>  <name>`, optionally
/// with a `*` binary marker) and bare single-digest files.
pub fn digest_for(checksums: &str, asset_name: &str) -> Result<String, UpdateError> {
    let missing = || UpdateError::MissingChecksum {
        asset: asset_name.to_string(),
    };

    for line in checksums.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut fields = line.split_whitespace();
        let Some(digest) = fields.next() else {
            continue;
        };

        match fields.next() {
            // `<digest>  <name>` — only accept the line naming our asset.
            Some(name) => {
                let name = name.trim_start_matches('*');
                let basename = name.rsplit('/').next().unwrap_or(name);
                if basename == asset_name {
                    return normalize_digest(digest, asset_name);
                }
            }
            // Bare digest file: it can only describe the asset we asked for.
            None => return normalize_digest(digest, asset_name),
        }
    }

    Err(missing())
}

fn normalize_digest(digest: &str, asset_name: &str) -> Result<String, UpdateError> {
    let digest = digest.trim().to_ascii_lowercase();

    let valid = digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err(UpdateError::MalformedChecksum {
            asset: asset_name.to_string(),
            value: digest,
        });
    }

    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.test/{name}"),
        }
    }

    fn release(tag: &str, assets: &[&str]) -> Release {
        Release {
            tag_name: tag.to_string(),
            html_url: format!("https://example.test/releases/{tag}"),
            prerelease: false,
            assets: assets.iter().map(|name| asset(name)).collect(),
        }
    }

    #[test]
    fn asset_name_matches_release_workflow_contract() {
        assert_eq!(
            asset_name("v0.2.0", "aarch64-apple-darwin"),
            "wvr-v0.2.0-aarch64-apple-darwin.tar.gz"
        );
    }

    #[test]
    fn tag_prefix_is_optional_when_parsing_versions() {
        assert_eq!(
            parse_version("v1.2.3").unwrap(),
            semver::Version::new(1, 2, 3)
        );
        assert_eq!(
            parse_version(" 1.2.3 ").unwrap(),
            parse_version("v1.2.3").unwrap()
        );
        assert!(parse_version("nightly").is_err());
    }

    #[test]
    fn prerelease_tags_order_below_their_release() {
        let rc = parse_version("v1.0.0-rc.1").unwrap();
        let final_release = parse_version("v1.0.0").unwrap();
        assert!(rc < final_release);
    }

    #[test]
    fn picks_the_archive_for_the_running_target() {
        let release = release(
            "v0.2.0",
            &[
                "wvr-v0.2.0-x86_64-unknown-linux-gnu.tar.gz",
                "wvr-v0.2.0-aarch64-apple-darwin.tar.gz",
                "SHA256SUMS",
            ],
        );

        let picked = release.asset_for("aarch64-apple-darwin").unwrap();
        assert_eq!(picked.name, "wvr-v0.2.0-aarch64-apple-darwin.tar.gz");
    }

    #[test]
    fn falls_back_to_target_suffix_when_tag_differs() {
        // Re-tagged/renamed release: asset keeps the target suffix contract.
        let release = release("v0.2.0", &["wvr-0.2.0-aarch64-apple-darwin.tar.gz"]);

        let picked = release.asset_for("aarch64-apple-darwin").unwrap();
        assert_eq!(picked.name, "wvr-0.2.0-aarch64-apple-darwin.tar.gz");
    }

    #[test]
    fn unsupported_target_is_reported_not_guessed() {
        let release = release("v0.2.0", &["wvr-v0.2.0-aarch64-apple-darwin.tar.gz"]);

        let err = release
            .asset_for("riscv64gc-unknown-linux-gnu")
            .unwrap_err();
        assert!(
            matches!(err, UpdateError::NoAsset { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn per_asset_checksum_wins_over_aggregate_manifest() {
        let release = release(
            "v0.2.0",
            &[
                "wvr-v0.2.0-aarch64-apple-darwin.tar.gz",
                "wvr-v0.2.0-aarch64-apple-darwin.tar.gz.sha256",
                "SHA256SUMS",
            ],
        );

        let archive = release.asset_for("aarch64-apple-darwin").unwrap().clone();
        let checksum = release.checksum_for(&archive).unwrap();
        assert_eq!(
            checksum.name,
            "wvr-v0.2.0-aarch64-apple-darwin.tar.gz.sha256"
        );
    }

    #[test]
    fn aggregate_manifest_is_used_when_no_per_asset_file_exists() {
        let release = release(
            "v0.2.0",
            &["wvr-v0.2.0-aarch64-apple-darwin.tar.gz", "SHA256SUMS"],
        );

        let archive = release.asset_for("aarch64-apple-darwin").unwrap().clone();
        assert_eq!(release.checksum_for(&archive).unwrap().name, "SHA256SUMS");
    }

    #[test]
    fn release_without_checksums_is_rejected() {
        let release = release("v0.2.0", &["wvr-v0.2.0-aarch64-apple-darwin.tar.gz"]);

        let archive = release.asset_for("aarch64-apple-darwin").unwrap().clone();
        let err = release.checksum_for(&archive).unwrap_err();
        assert!(
            matches!(err, UpdateError::MissingChecksum { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn digest_is_selected_by_asset_name_from_a_manifest() {
        let manifest = concat!(
            "1111111111111111111111111111111111111111111111111111111111111111  wvr-v0.2.0-x86_64-unknown-linux-gnu.tar.gz\n",
            "2222222222222222222222222222222222222222222222222222222222222222  wvr-v0.2.0-aarch64-apple-darwin.tar.gz\n",
        );

        let digest = digest_for(manifest, "wvr-v0.2.0-aarch64-apple-darwin.tar.gz").unwrap();
        assert_eq!(digest, "2".repeat(64));
    }

    #[test]
    fn digest_accepts_bare_and_binary_marked_formats() {
        let bare = format!("{}\n", "a".repeat(64));
        assert_eq!(digest_for(&bare, "wvr.tar.gz").unwrap(), "a".repeat(64));

        let marked = format!("{}  *dist/wvr.tar.gz\n", "B".repeat(64));
        assert_eq!(digest_for(&marked, "wvr.tar.gz").unwrap(), "b".repeat(64));
    }

    #[test]
    fn asset_missing_from_manifest_is_an_error() {
        let manifest = format!("{}  some-other-file.tar.gz\n", "c".repeat(64));

        let err = digest_for(&manifest, "wvr-v0.2.0-aarch64-apple-darwin.tar.gz").unwrap_err();
        assert!(
            matches!(err, UpdateError::MissingChecksum { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn truncated_digest_is_rejected_rather_than_trusted() {
        let manifest = "deadbeef  wvr.tar.gz\n";

        let err = digest_for(manifest, "wvr.tar.gz").unwrap_err();
        assert!(
            matches!(err, UpdateError::MalformedChecksum { .. }),
            "unexpected error: {err}"
        );
    }
}
