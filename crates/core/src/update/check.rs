//! Periodic release check: when to look, and what was last seen.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::UpdateError;
use super::release::parse_version;

/// How often the background check talks to GitHub.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Disable the background check entirely.
pub const NO_CHECK_ENV: &str = "WVR_NO_UPDATE_CHECK";
/// Check even when the run is non-interactive or on CI.
pub const CHECK_ENV: &str = "WVR_UPDATE_CHECK";
/// Install new releases automatically instead of only printing a notice.
pub const AUTO_UPDATE_ENV: &str = "WVR_AUTO_UPDATE";
/// Override the check interval, in hours (`0` checks on every run).
pub const INTERVAL_ENV: &str = "WVR_UPDATE_INTERVAL_HOURS";

/// What the CLI is allowed to do about updates on a normal run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePreferences {
    /// Consult (and refresh) the latest release.
    pub check: bool,
    /// Print a notice when a newer release exists.
    pub announce: bool,
    /// Install the new release instead of just printing a notice.
    pub auto_apply: bool,
    /// Minimum age of the cached result before checking again.
    pub interval: Duration,
}

impl UpdatePreferences {
    /// Read preferences for a run whose output a human is watching
    /// (`interactive`), typically "stderr is a terminal and output is not
    /// machine-readable".
    pub fn from_env(interactive: bool) -> Self {
        Self::resolve(Inputs {
            no_check: std::env::var(NO_CHECK_ENV).ok(),
            force_check: std::env::var(CHECK_ENV).ok(),
            auto_update: std::env::var(AUTO_UPDATE_ENV).ok(),
            interval_hours: std::env::var(INTERVAL_ENV).ok(),
            ci: std::env::var_os("CI").is_some(),
            interactive,
        })
    }

    /// Pure precedence rules: opting out beats everything; scripted and CI
    /// runs stay off the network unless updates were explicitly requested.
    fn resolve(inputs: Inputs) -> Self {
        let truthy = |value: &Option<String>| value.as_deref().is_some_and(is_truthy);

        let opted_out = truthy(&inputs.no_check);
        let forced = truthy(&inputs.force_check) && !opted_out;
        let auto_apply = truthy(&inputs.auto_update) && !opted_out;
        let check = !opted_out && (forced || auto_apply || (inputs.interactive && !inputs.ci));
        let announce = check && (inputs.interactive || forced);

        let interval = inputs
            .interval_hours
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .map(|hours| Duration::from_secs(hours * 60 * 60))
            .unwrap_or(DEFAULT_INTERVAL);

        Self {
            check,
            announce,
            auto_apply,
            interval,
        }
    }
}

/// Raw environment inputs behind [`UpdatePreferences`].
struct Inputs {
    no_check: Option<String>,
    force_check: Option<String>,
    auto_update: Option<String>,
    interval_hours: Option<String>,
    ci: bool,
    interactive: bool,
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "always"
    )
}

/// Result of the last release check, cached so normal runs never block on the
/// network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCheck {
    /// Unix timestamp (seconds) of the check.
    pub checked_at: i64,
    /// Latest release version seen, without the `v` prefix.
    pub latest_version: String,
    #[serde(default)]
    pub html_url: String,
}

impl UpdateCheck {
    pub fn new(latest_version: &semver::Version, html_url: &str, now: i64) -> Self {
        Self {
            checked_at: now,
            latest_version: latest_version.to_string(),
            html_url: html_url.to_string(),
        }
    }

    /// Read the cached check, treating missing or unreadable state as "never
    /// checked" — a stale cache must never break a command.
    pub fn load(path: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Persist atomically so a concurrent `wvr` never reads a half-written file.
    pub fn save(&self, path: &Path) -> Result<(), UpdateError> {
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir).map_err(|source| UpdateError::Install {
            path: dir.display().to_string(),
            source,
        })?;

        let staged = path.with_extension(format!("tmp-{}", std::process::id()));
        let body = serde_json::to_string_pretty(self).map_err(|e| {
            UpdateError::Config(format!("cannot serialize update check state: {e}"))
        })?;

        std::fs::write(&staged, body).map_err(|source| UpdateError::Install {
            path: staged.display().to_string(),
            source,
        })?;

        std::fs::rename(&staged, path).map_err(|source| {
            let _ = std::fs::remove_file(&staged);
            UpdateError::Install {
                path: path.display().to_string(),
                source,
            }
        })
    }

    /// Whether the cached result is old enough to refresh.
    pub fn is_stale(&self, now: i64, interval: Duration) -> bool {
        let age = now.saturating_sub(self.checked_at);
        // A timestamp in the future means a clock change; refresh rather than
        // trusting it forever.
        age < 0 || age as u64 >= interval.as_secs()
    }

    /// The cached version when it is newer than `current`.
    pub fn newer_than(&self, current: &semver::Version) -> Option<semver::Version> {
        let latest = parse_version(&self.latest_version).ok()?;
        (latest > *current).then_some(latest)
    }
}

/// Current unix timestamp in seconds.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build inputs for an interactive, non-CI run with nothing configured.
    fn interactive() -> Inputs {
        Inputs {
            no_check: None,
            force_check: None,
            auto_update: None,
            interval_hours: None,
            ci: false,
            interactive: true,
        }
    }

    #[test]
    fn interactive_runs_check_and_announce_by_default() {
        let prefs = UpdatePreferences::resolve(interactive());

        assert!(prefs.check);
        assert!(prefs.announce);
        assert!(!prefs.auto_apply);
        assert_eq!(prefs.interval, DEFAULT_INTERVAL);
    }

    #[test]
    fn scripted_output_stays_off_the_network() {
        let prefs = UpdatePreferences::resolve(Inputs {
            interactive: false,
            ..interactive()
        });

        assert!(!prefs.check);
        assert!(!prefs.announce);
    }

    #[test]
    fn opting_out_beats_every_other_setting() {
        let prefs = UpdatePreferences::resolve(Inputs {
            no_check: Some("1".to_string()),
            force_check: Some("1".to_string()),
            auto_update: Some("1".to_string()),
            ..interactive()
        });

        assert!(!prefs.check);
        assert!(!prefs.announce);
        assert!(!prefs.auto_apply);
    }

    #[test]
    fn ci_runs_do_not_check_unless_updates_are_requested() {
        let ci = Inputs {
            ci: true,
            ..interactive()
        };
        assert!(!UpdatePreferences::resolve(Inputs { ..ci }).check);

        let auto = UpdatePreferences::resolve(Inputs {
            auto_update: Some("true".to_string()),
            ci: true,
            ..interactive()
        });
        assert!(auto.check);
        assert!(auto.auto_apply);
    }

    #[test]
    fn forcing_a_check_works_for_scripted_runs() {
        let prefs = UpdatePreferences::resolve(Inputs {
            force_check: Some("1".to_string()),
            interactive: false,
            ci: true,
            ..interactive()
        });

        assert!(prefs.check);
        assert!(prefs.announce);
        assert!(!prefs.auto_apply);
    }

    #[test]
    fn auto_update_without_a_terminal_installs_without_announcing() {
        let prefs = UpdatePreferences::resolve(Inputs {
            auto_update: Some("1".to_string()),
            interactive: false,
            ..interactive()
        });

        assert!(prefs.check);
        assert!(prefs.auto_apply);
        assert!(!prefs.announce);
    }

    #[test]
    fn interval_override_is_respected_including_always() {
        let with_hours = |hours: &str| {
            UpdatePreferences::resolve(Inputs {
                interval_hours: Some(hours.to_string()),
                ..interactive()
            })
            .interval
        };

        assert_eq!(with_hours("6"), Duration::from_secs(6 * 60 * 60));
        assert_eq!(with_hours("0"), Duration::ZERO);
        assert_eq!(with_hours("soon"), DEFAULT_INTERVAL);
    }

    #[test]
    fn only_truthy_values_enable_flags() {
        // `WVR_NO_UPDATE_CHECK=0` is not an opt-out.
        for value in ["0", "false", "off", ""] {
            let prefs = UpdatePreferences::resolve(Inputs {
                no_check: Some(value.to_string()),
                ..interactive()
            });
            assert!(prefs.check, "{value:?} should not disable checks");
        }

        for value in ["1", "true", "yes", "on", "always"] {
            let prefs = UpdatePreferences::resolve(Inputs {
                auto_update: Some(value.to_string()),
                ..interactive()
            });
            assert!(prefs.auto_apply, "{value:?} should enable auto-update");
        }

        for value in ["0", "no", "maybe"] {
            let prefs = UpdatePreferences::resolve(Inputs {
                auto_update: Some(value.to_string()),
                ..interactive()
            });
            assert!(!prefs.auto_apply, "{value:?} should not enable auto-update");
        }
    }

    #[test]
    fn cached_check_becomes_stale_after_the_interval() {
        let state = UpdateCheck {
            checked_at: 1_000,
            latest_version: "0.2.0".to_string(),
            html_url: String::new(),
        };

        assert!(!state.is_stale(1_000 + 3_599, Duration::from_secs(3_600)));
        assert!(state.is_stale(1_000 + 3_600, Duration::from_secs(3_600)));
    }

    #[test]
    fn clock_moving_backwards_forces_a_refresh() {
        let state = UpdateCheck {
            checked_at: 10_000,
            latest_version: "0.2.0".to_string(),
            html_url: String::new(),
        };

        assert!(state.is_stale(5_000, DEFAULT_INTERVAL));
    }

    #[test]
    fn newer_than_compares_semantically() {
        let state = UpdateCheck {
            checked_at: 0,
            latest_version: "0.10.0".to_string(),
            html_url: String::new(),
        };

        assert_eq!(
            state.newer_than(&semver::Version::new(0, 9, 0)),
            Some(semver::Version::new(0, 10, 0))
        );
        assert_eq!(state.newer_than(&semver::Version::new(0, 10, 0)), None);
        assert_eq!(state.newer_than(&semver::Version::new(1, 0, 0)), None);
    }

    #[test]
    fn unparseable_cached_version_is_ignored() {
        let state = UpdateCheck {
            checked_at: 0,
            latest_version: "nightly".to_string(),
            html_url: String::new(),
        };

        assert_eq!(state.newer_than(&semver::Version::new(0, 1, 0)), None);
    }

    #[test]
    fn state_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("update-check.json");
        let state = UpdateCheck::new(
            &semver::Version::new(1, 2, 3),
            "https://example.test/releases/v1.2.3",
            42,
        );

        state.save(&path).unwrap();

        assert_eq!(UpdateCheck::load(&path), Some(state));
    }

    #[test]
    fn corrupt_state_reads_as_never_checked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        std::fs::write(&path, "{not json").unwrap();

        assert_eq!(UpdateCheck::load(&path), None);
        assert_eq!(UpdateCheck::load(&dir.path().join("missing.json")), None);
    }
}
