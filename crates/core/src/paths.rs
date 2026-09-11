//! User-level directory layout for `wvr`.
//!
//! Everything the CLI keeps outside a project lives under a single root so it
//! can be relocated (tests, sandboxes, CI) with one environment variable.

use std::ffi::OsString;
use std::path::PathBuf;

/// Environment variable that relocates the user-level state root.
pub const HOME_ENV: &str = "WVR_HOME";

/// Root directory for user-level state (`$WVR_HOME`, default `~/.rw`).
pub fn home() -> PathBuf {
    resolve_home(std::env::var_os(HOME_ENV), user_home())
}

/// Global plugin cache (`<home>/plugins`).
pub fn plugins_dir() -> PathBuf {
    home().join("plugins")
}

/// Module store where fetched module sources are materialised (`<home>/store`).
pub fn store_dir() -> PathBuf {
    home().join("store")
}

/// Cached result of the last release check (`<home>/update-check.json`).
pub fn update_check_file() -> PathBuf {
    home().join("update-check.json")
}

fn user_home() -> Option<PathBuf> {
    home::home_dir().or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

/// Pure resolution so the precedence rules are testable without mutating the
/// process environment.
fn resolve_home(override_var: Option<OsString>, user_home: Option<PathBuf>) -> PathBuf {
    match override_var {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => user_home.unwrap_or_else(|| PathBuf::from(".")).join(".rw"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `$WVR_HOME` wins over the user's home directory so callers can be
    /// sandboxed without touching real state.
    #[test]
    fn override_wins_over_user_home() {
        let resolved = resolve_home(
            Some(OsString::from("/tmp/wvr-home")),
            Some(PathBuf::from("/Users/example")),
        );
        assert_eq!(resolved, PathBuf::from("/tmp/wvr-home"));
    }

    /// An empty override behaves as "unset" instead of resolving to a relative
    /// path rooted at the current directory.
    #[test]
    fn empty_override_falls_back_to_user_home() {
        let resolved = resolve_home(
            Some(OsString::from("")),
            Some(PathBuf::from("/Users/example")),
        );
        assert_eq!(resolved, PathBuf::from("/Users/example/.rw"));
    }

    /// A user without a resolvable home still gets a usable relative root
    /// rather than a panic.
    #[test]
    fn missing_user_home_falls_back_to_cwd() {
        assert_eq!(resolve_home(None, None), PathBuf::from("./.rw"));
    }

    #[test]
    fn subdirectories_hang_off_the_root() {
        let root = home();
        assert_eq!(plugins_dir(), root.join("plugins"));
        assert_eq!(store_dir(), root.join("store"));
        assert_eq!(update_check_file(), root.join("update-check.json"));
    }
}
