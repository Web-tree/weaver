//! Unpacking a release archive and swapping the running binary in place.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use super::UpdateError;
use super::release::BIN_NAME;

/// Extract the `wvr` executable from a `.tar.gz` release archive.
pub fn extract_binary(archive: &[u8]) -> Result<Vec<u8>, UpdateError> {
    let decoder = flate2::read::GzDecoder::new(archive);
    let mut tar = tar::Archive::new(decoder);

    let entries = tar
        .entries()
        .map_err(|e| UpdateError::Archive(format!("cannot read release archive: {e}")))?;

    for entry in entries {
        let mut entry =
            entry.map_err(|e| UpdateError::Archive(format!("corrupt release archive: {e}")))?;

        let path = entry
            .path()
            .map_err(|e| UpdateError::Archive(format!("invalid path in release archive: {e}")))?
            .into_owned();

        if path.file_name().and_then(|n| n.to_str()) != Some(BIN_NAME) {
            continue;
        }

        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| UpdateError::Archive(format!("cannot unpack {BIN_NAME}: {e}")))?;

        return Ok(bytes);
    }

    Err(UpdateError::Archive(format!(
        "release archive does not contain a `{BIN_NAME}` executable"
    )))
}

/// Path of the binary to replace: the running executable with symlinks
/// resolved, so installs behind `~/.local/bin/wvr -> …` update the real file.
pub fn current_binary_path() -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe().map_err(|source| UpdateError::Install {
        path: "<current exe>".to_string(),
        source,
    })?;

    Ok(fs::canonicalize(&exe).unwrap_or(exe))
}

/// Atomically replace `path` with `bytes`.
///
/// The replacement is written next to the target (same filesystem, so the
/// final `rename` is atomic), made executable, and smoke-tested by running
/// `--version` before it is allowed to take over. A failure at any step leaves
/// the installed binary untouched.
pub fn replace_binary(
    path: &Path,
    bytes: &[u8],
    expected_version: &semver::Version,
) -> Result<(), UpdateError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let staged = Staged::create(dir, path, bytes)?;

    verify_version(staged.path(), expected_version)?;

    fs::rename(staged.path(), path).map_err(|source| UpdateError::Install {
        path: path.display().to_string(),
        source,
    })?;

    staged.keep();
    Ok(())
}

/// Temporary file next to the install target, removed on drop unless kept.
struct Staged {
    path: PathBuf,
    keep: bool,
}

impl Staged {
    fn create(dir: &Path, target: &Path, bytes: &[u8]) -> Result<Self, UpdateError> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path = dir.join(format!(".{BIN_NAME}-update-{}-{nanos}", std::process::id()));

        let staged = Self { path, keep: false };

        fs::write(&staged.path, bytes).map_err(|source| install_error(dir, target, source))?;
        set_executable(&staged.path, target)?;

        Ok(staged)
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Writing into the install directory is the step most likely to fail for
/// system-wide installs, so point at the real fix instead of the raw errno.
fn install_error(dir: &Path, target: &Path, source: io::Error) -> UpdateError {
    if source.kind() == io::ErrorKind::PermissionDenied {
        return UpdateError::NotWritable {
            path: dir.display().to_string(),
            binary: target.display().to_string(),
        };
    }

    UpdateError::Install {
        path: dir.display().to_string(),
        source,
    }
}

/// Keep the mode of the binary being replaced (so `0755` root installs and
/// `0700` private installs both survive), defaulting to `0755`.
#[cfg(unix)]
fn set_executable(staged: &Path, target: &Path) -> Result<(), UpdateError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(target)
        .map(|meta| meta.permissions().mode() & 0o7777)
        .unwrap_or(0o755);

    fs::set_permissions(staged, fs::Permissions::from_mode(mode)).map_err(|source| {
        UpdateError::Install {
            path: staged.display().to_string(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_executable(_staged: &Path, _target: &Path) -> Result<(), UpdateError> {
    Ok(())
}

/// Run the staged binary and require it to report the version we expected to
/// install. Catches truncated downloads, wrong-architecture assets and
/// mislabelled release archives before they replace a working binary.
fn verify_version(staged: &Path, expected: &semver::Version) -> Result<(), UpdateError> {
    let output = Command::new(staged)
        .arg("--version")
        .output()
        .map_err(|source| UpdateError::Unusable {
            reason: format!("cannot execute downloaded binary: {source}"),
        })?;

    if !output.status.success() {
        return Err(UpdateError::Unusable {
            reason: format!(
                "downloaded binary exited with {} on `--version`",
                output.status
            ),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let reported = parse_version_output(&stdout).ok_or_else(|| UpdateError::Unusable {
        reason: format!(
            "downloaded binary reported an unparseable version: {:?}",
            stdout.trim()
        ),
    })?;

    if &reported != expected {
        return Err(UpdateError::VersionMismatch {
            expected: expected.to_string(),
            actual: reported.to_string(),
        });
    }

    Ok(())
}

/// `wvr 0.2.0` -> `0.2.0`
fn parse_version_output(stdout: &str) -> Option<semver::Version> {
    stdout
        .lines()
        .next()?
        .split_whitespace()
        .filter_map(|token| semver::Version::parse(token.trim_start_matches('v')).ok())
        .next_back()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a `.tar.gz` with the given `(path, contents)` entries.
    fn tarball(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            for (path, contents) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(contents.len() as u64);
                header.set_mode(0o755);
                header.set_cksum();
                builder.append_data(&mut header, path, *contents).unwrap();
            }
            builder.finish().unwrap();
        }
        encoder.finish().unwrap()
    }

    /// A stand-in for a released binary: an executable that answers
    /// `--version` the way the real CLI does.
    fn fake_binary(version: &str) -> Vec<u8> {
        format!("#!/bin/sh\necho \"wvr {version}\"\n").into_bytes()
    }

    fn write_executable(path: &Path, bytes: &[u8]) {
        let mut file = fs::File::create(path).unwrap();
        file.write_all(bytes).unwrap();
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn extracts_the_executable_ignoring_other_archive_entries() {
        let archive = tarball(&[
            ("README.md", b"docs" as &[u8]),
            ("wvr", b"binary-bytes"),
            ("completions/wvr.bash", b"completions"),
        ]);

        assert_eq!(extract_binary(&archive).unwrap(), b"binary-bytes");
    }

    #[test]
    fn extracts_from_archives_with_a_top_level_directory() {
        let archive = tarball(&[("wvr-v1.0.0-aarch64-apple-darwin/wvr", b"binary-bytes")]);

        assert_eq!(extract_binary(&archive).unwrap(), b"binary-bytes");
    }

    #[test]
    fn archive_without_the_executable_is_rejected() {
        let archive = tarball(&[("README.md", b"docs" as &[u8])]);

        let err = extract_binary(&archive).unwrap_err();
        assert!(
            matches!(err, UpdateError::Archive(_)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn garbage_bytes_are_rejected_as_a_corrupt_archive() {
        let err = extract_binary(b"not a gzip stream at all").unwrap_err();
        assert!(
            matches!(err, UpdateError::Archive(_)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn replaces_the_binary_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("wvr");
        write_executable(&installed, &fake_binary("0.1.0"));

        replace_binary(
            &installed,
            &fake_binary("0.2.0"),
            &semver::Version::new(0, 2, 0),
        )
        .unwrap();

        let output = Command::new(&installed).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "wvr 0.2.0");
    }

    #[test]
    fn replacement_keeps_the_original_mode() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let dir = tempfile::tempdir().unwrap();
            let installed = dir.path().join("wvr");
            write_executable(&installed, &fake_binary("0.1.0"));
            fs::set_permissions(&installed, fs::Permissions::from_mode(0o700)).unwrap();

            replace_binary(
                &installed,
                &fake_binary("0.2.0"),
                &semver::Version::new(0, 2, 0),
            )
            .unwrap();

            let mode = fs::metadata(&installed).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "mode was {mode:o}");
        }
    }

    #[test]
    fn binary_reporting_the_wrong_version_never_takes_over() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("wvr");
        write_executable(&installed, &fake_binary("0.1.0"));

        let err = replace_binary(
            &installed,
            &fake_binary("0.1.5"),
            &semver::Version::new(0, 2, 0),
        )
        .unwrap_err();

        assert!(
            matches!(err, UpdateError::VersionMismatch { .. }),
            "unexpected error: {err}"
        );
        let output = Command::new(&installed).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "wvr 0.1.0");
    }

    #[test]
    fn unrunnable_download_leaves_no_staged_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("wvr");
        write_executable(&installed, &fake_binary("0.1.0"));

        let err = replace_binary(
            &installed,
            b"\x7fELF-for-another-cpu",
            &semver::Version::new(0, 2, 0),
        )
        .unwrap_err();
        assert!(
            matches!(err, UpdateError::Unusable { .. }),
            "unexpected error: {err}"
        );

        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name != "wvr")
            .collect();
        assert!(
            leftovers.is_empty(),
            "staged files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn version_output_parsing_handles_clap_formats() {
        assert_eq!(
            parse_version_output("wvr 0.2.0\n"),
            Some(semver::Version::new(0, 2, 0))
        );
        assert_eq!(
            parse_version_output("wvr v1.0.0-rc.1\n"),
            Some(semver::Version::parse("1.0.0-rc.1").unwrap())
        );
        assert_eq!(parse_version_output("garbage\n"), None);
    }
}
