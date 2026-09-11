//! Background release check.
//!
//! A normal `wvr` run never blocks on the network: the notice is printed from
//! a cached result while a refresh runs concurrently with the command itself.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

use console::style;
use weaver_core::paths;
use weaver_core::update::check::{UpdateCheck, UpdatePreferences, now_unix};
use weaver_core::update::{Updater, release::BIN_NAME};

/// How long the refresh started at launch is given to finish once the command
/// is done. It normally completed while the command was running.
const REFRESH_GRACE: Duration = Duration::from_millis(1_500);

/// A release check running alongside the current command.
pub struct UpdateWatch {
    current: semver::Version,
    state_path: PathBuf,
    /// Update already known from the cached check.
    cached: Option<UpdateCheck>,
    refresh: Option<tokio::task::JoinHandle<Option<UpdateCheck>>>,
    announce: bool,
    auto_apply: bool,
}

/// Start a check unless the user, the environment, or the output mode opts out.
///
/// `output_is_human` is false for machine-readable or silenced output, where a
/// notice would be noise.
pub fn start(output_is_human: bool) -> UpdateWatch {
    let interactive = output_is_human && std::io::stderr().is_terminal();
    let prefs = UpdatePreferences::from_env(interactive);

    let mut watch = UpdateWatch {
        current: current_version(),
        state_path: paths::update_check_file(),
        cached: None,
        refresh: None,
        announce: prefs.announce && output_is_human,
        auto_apply: prefs.auto_apply,
    };

    if !prefs.check {
        return watch;
    }

    watch.cached = UpdateCheck::load(&watch.state_path);
    let stale = watch
        .cached
        .as_ref()
        .is_none_or(|state| state.is_stale(now_unix(), prefs.interval));

    if stale {
        let state_path = watch.state_path.clone();
        let current = watch.current.clone();
        watch.refresh = Some(tokio::spawn(async move {
            let updater = Updater::from_env(current).ok()?;
            match updater.refresh_check_state(&state_path).await {
                Ok(state) => Some(state),
                Err(e) => {
                    tracing::debug!(error = %e, "release check failed");
                    None
                }
            }
        }));
    }

    watch
}

fn current_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(semver::Version::new(0, 0, 0))
}

impl UpdateWatch {
    /// Print the notice, or install the update when auto-update is enabled.
    ///
    /// Never fails the command it is attached to: an unreachable GitHub or an
    /// unwritable install directory only produces a warning.
    pub async fn finish(mut self) {
        let Some((version, url)) = self.resolve_latest().await else {
            return;
        };

        if self.auto_apply {
            let message = self.apply(&version).await;
            let _ = std::io::stderr().write_all(message.as_bytes());
            return;
        }

        if self.announce {
            let _ = std::io::stderr().write_all(notice(&self.current, &version, &url).as_bytes());
        }
    }

    /// The newer version to report: from the refresh if it finished in time,
    /// otherwise from the cached check.
    async fn resolve_latest(&mut self) -> Option<(semver::Version, String)> {
        if let Some(handle) = self.refresh.take() {
            // Timeout, panic or failure: fall back to the cache and check
            // again on the next run.
            if let Ok(Ok(Some(state))) = tokio::time::timeout(REFRESH_GRACE, handle).await {
                self.cached = Some(state);
            }
        }

        let state = self.cached.as_ref()?;
        let version = state.newer_than(&self.current)?;
        Some((version, state.html_url.clone()))
    }

    async fn apply(&self, version: &semver::Version) -> String {
        let outcome = async {
            let updater = Updater::from_env(self.current.clone())?;
            let release = updater.latest_release().await?;
            updater.install(&release).await
        }
        .await;

        match outcome {
            Ok(installed) => format!(
                "\n{} {BIN_NAME} {} -> {installed}\n",
                style("Auto-updated:").green().bold(),
                self.current
            ),
            Err(e) => format!(
                "\n{} could not auto-update to {version}: {e}\n",
                style("warning:").yellow().bold()
            ),
        }
    }
}

/// The notice shown at the end of a command when a newer release exists.
fn notice(current: &semver::Version, latest: &semver::Version, url: &str) -> String {
    let mut out = format!(
        "\n{} {BIN_NAME} {current} -> {}\n",
        style("Update available:").yellow().bold(),
        style(latest).bold()
    );

    if !url.is_empty() {
        out.push_str(&format!("  {}\n", style(url).dim()));
    }
    out.push_str(&format!("  Run `{BIN_NAME} self-update` to install it\n"));

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> String {
        console::strip_ansi_codes(text).to_string()
    }

    #[test]
    fn notice_names_both_versions_and_the_upgrade_command() {
        let text = plain(&notice(
            &semver::Version::new(0, 1, 0),
            &semver::Version::new(0, 2, 0),
            "https://example.test/releases/v0.2.0",
        ));

        assert!(text.contains("wvr 0.1.0 -> 0.2.0"), "{text}");
        assert!(
            text.contains("https://example.test/releases/v0.2.0"),
            "{text}"
        );
        assert!(text.contains("wvr self-update"), "{text}");
    }

    #[test]
    fn notice_without_a_release_url_still_renders() {
        let text = plain(&notice(
            &semver::Version::new(0, 1, 0),
            &semver::Version::new(0, 2, 0),
            "",
        ));

        assert!(text.contains("wvr 0.1.0 -> 0.2.0"), "{text}");
        assert!(!text.contains("  \n"), "empty url line rendered: {text:?}");
    }

    /// Only a cached version newer than the running one produces a notice.
    #[tokio::test]
    async fn cached_state_decides_whether_there_is_anything_to_report() {
        let newer = watch_with_cache("0.1.0", "0.2.0");
        assert_eq!(
            newer.await,
            Some((semver::Version::new(0, 2, 0), String::new()))
        );

        let same = watch_with_cache("0.2.0", "0.2.0");
        assert_eq!(same.await, None);

        let older = watch_with_cache("0.3.0", "0.2.0");
        assert_eq!(older.await, None);
    }

    async fn watch_with_cache(current: &str, cached: &str) -> Option<(semver::Version, String)> {
        let mut watch = UpdateWatch {
            current: semver::Version::parse(current).unwrap(),
            state_path: PathBuf::from("/nonexistent"),
            cached: Some(UpdateCheck {
                checked_at: now_unix(),
                latest_version: cached.to_string(),
                html_url: String::new(),
            }),
            refresh: None,
            announce: true,
            auto_apply: false,
        };

        watch.resolve_latest().await
    }
}
