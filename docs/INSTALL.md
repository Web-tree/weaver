# Installing and updating `wvr`

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/Web-tree/weaver/main/scripts/install.sh | sh
```

The script picks the archive for your platform, verifies it against the
release's `SHA256SUMS`, refuses anything that does not run, and installs to
`~/.local/bin` (or `/usr/local/bin` when run as root).

| Variable | Purpose | Default |
| --- | --- | --- |
| `WVR_VERSION` | Release tag to install | latest release |
| `WVR_INSTALL_DIR` | Install directory | `~/.local/bin` |
| `WVR_REPO` | Source repository | `Web-tree/weaver` |
| `GITHUB_TOKEN` | Avoids API rate limits | unset |

```sh
# Pin a version, install system-wide.
# `sudo env` is required: sudo's env_reset would drop these variables.
curl -fsSL .../install.sh | sudo env WVR_VERSION=v0.1.0 WVR_INSTALL_DIR=/usr/local/bin sh
```

### From source

Requires Rust 1.92+:

```sh
cargo install --git https://github.com/Web-tree/weaver --locked weaver
# or, in a clone
cargo install --path crates/cli --locked
```

## Supported targets

| Platform | Asset suffix |
| --- | --- |
| macOS, Apple silicon | `aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `x86_64-apple-darwin.tar.gz` |
| Linux x86_64, glibc 2.35+ | `x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64, glibc 2.35+ | `aarch64-unknown-linux-gnu.tar.gz` |

Anything else: build from source.

## Verify a download

Every release publishes per-asset `.sha256` files, an aggregate `SHA256SUMS`,
and a GitHub build provenance attestation:

```sh
sha256sum -c SHA256SUMS --ignore-missing
gh attestation verify wvr-v0.1.0-aarch64-apple-darwin.tar.gz --repo Web-tree/weaver
```

## Updating

```sh
wvr self-update            # install the latest release
wvr self-update --check    # report only
wvr self-update --tag v0.1.0
wvr self-update --force    # reinstall the current version
```

`self-update` verifies the published SHA-256 digest, runs the downloaded
binary's `--version` and only then swaps it into place with an atomic rename.
A failed download never replaces a working install. Symlinked installs are
resolved first, so the real file is replaced.

If `wvr` lives in a root-owned directory, run `sudo wvr self-update`.

### Background update notice

Interactive runs check for a new release at most once every 24 hours (the
result is cached in `$WVR_HOME/update-check.json`) and print a notice after the
command finishes:

```
Update available: wvr 0.1.0 -> 0.2.0
  https://github.com/Web-tree/weaver/releases/tag/v0.2.0
  Run `wvr self-update` to install it
```

The check runs concurrently with your command, never blocks it, and is skipped
for `--json`, `--quiet`, non-terminal output, and CI.

| Variable | Effect |
| --- | --- |
| `WVR_NO_UPDATE_CHECK=1` | Disable checks entirely (wins over everything else) |
| `WVR_UPDATE_CHECK=1` | Check even when output is piped or `CI` is set |
| `WVR_AUTO_UPDATE=1` | Install new releases automatically instead of printing a notice |
| `WVR_UPDATE_INTERVAL_HOURS=n` | Check interval; `0` checks on every run |
| `WVR_UPDATE_TOKEN` | Token for release lookups (falls back to `GITHUB_TOKEN`) |
| `WVR_UPDATE_REPO` | Release repository, `owner/repo` |
| `WVR_UPDATE_API_BASE` | GitHub API base URL |
| `WVR_UPDATE_TARGET` | Target triple to download assets for |
| `WVR_HOME` | State root for caches and update state (default `~/.rw`) |

## Cutting a release

Releases are driven by [Release Please](https://github.com/googleapis/release-please)
from [conventional commits](https://www.conventionalcommits.org/); nobody bumps
a version or pushes a tag by hand.

1. Land commits on `main` with conventional messages. `feat:` bumps the minor
   version (pre-1.0), `fix:`/`perf:`/`refactor:` the patch, `feat!:` or a
   `BREAKING CHANGE:` footer the major.
2. `.github/workflows/release-please.yml` keeps a **release PR** open titled
   `chore(main): release wvr X.Y.Z`. It updates `CHANGELOG.md`,
   `[workspace.package].version` and — via a follow-up `cargo update
   --workspace` commit — `Cargo.lock`, so the PR builds under `--locked`.
3. Merging that PR tags `vX.Y.Z`, creates the GitHub release from the changelog
   entry, and calls `release.yml` to build and attach the binaries.

To ship a specific version instead of the computed one, edit the version in the
release PR, or push a commit with a `Release-As: 1.0.0` footer.

### What `release.yml` does

1. verifies the tag matches `[workspace.package].version` and runs `cargo test --locked`;
2. builds `wvr` with the `dist` profile for all four targets (macOS binaries are
   re-signed ad-hoc after stripping) and smoke-tests every native build;
3. packages `wvr-<tag>-<target>.tar.gz` plus per-asset checksums;
4. attaches the archives, `SHA256SUMS`, `install.sh` and a build provenance
   attestation to the release, then appends install instructions below the
   changelog. Tags containing `-` are marked as prereleases and are not served
   as `latest`, so `wvr self-update` and `install.sh` ignore them.

Binaries land a few minutes after the release appears — the release object is
created first, then the four builds attach their assets.

Two escape hatches: pushing a `vX.Y.Z` tag by hand runs the same workflow (it
creates the release itself if Release Please hasn't), and `workflow_dispatch`
builds everything while publishing nothing, for rehearsing.
