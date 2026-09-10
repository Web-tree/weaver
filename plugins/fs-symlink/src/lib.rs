//! `fs.symlink` ensure — converges `src` into being a symlink pointing at
//! `dst`, relocating any real data from `src` to `dst` first if needed.
//!
//! Used to move large cache/toolchain directories (e.g. `~/.rustup`) off a
//! small internal disk onto a bigger external volume without breaking any
//! tool that hardcodes the original path: tools keep opening `src`, the OS
//! transparently follows the symlink to `dst`.
//!
//! Detection (`test -L`, `test -e`, `readlink`) and mutation (`mkdir -p`,
//! `mv`, `ln -s`, `rm`) both go through the host `process.exec` import,
//! because `src`/`dst` are arbitrary absolute paths outside any "app"
//! directory — not files inside the project being converged.
//!
//! The convergence decision itself is a small pure function (`decide`) over
//! booleans/strings describing detected filesystem state, so it is
//! unit-testable without shelling out. `plan`/`execute` only gather that
//! state (via exec) and hand it to `decide`.

use serde::Deserialize;
use wit_bindgen::generate;

generate!({
    world: "ensure-provider",
    path: "../../wit",
});

use exports::weaver::plugin::ensures::{EnsureError, EnsurePlan, EnsureRequest, Guest};
use weaver::plugin::process::{exec, ExecRequest};

struct Component;

#[derive(Deserialize)]
struct SymlinkConfig {
    src: String,
    dst: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_true")]
    precreate: bool,
}

fn default_true() -> bool {
    true
}

/// The convergence decision for a given detected filesystem state. Pure data
/// — no I/O — so `decide()` below can be unit tested directly.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Decision {
    /// `enabled: false` — no-op regardless of anything else.
    Disabled,
    /// `src` is already a symlink pointing at `dst`.
    AlreadyLinked,
    /// `src` is a symlink pointing somewhere else (drift): remove and
    /// recreate pointing at `dst`.
    Repoint { old_target: String },
    /// `src` holds real data and `dst` is empty: move `src` -> `dst`, then
    /// link `src` -> `dst`.
    Move,
    /// Real data exists at *both* `src` and `dst`. Never deleted or merged
    /// automatically — surfaced for manual resolution.
    Conflict,
    /// `src` is absent but `dst` already holds the data (e.g. relocated by
    /// another machine sharing the same volume, or a previous run): just
    /// link `src` -> `dst`.
    LinkOnly,
    /// Neither `src` nor `dst` exists and `precreate: true`: create `dst`
    /// and link `src` -> `dst` so a freshly installed tool writes straight
    /// into the relocated storage.
    Precreate,
    /// Neither `src` nor `dst` exists and `precreate: false`: nothing to do
    /// yet.
    NothingYet,
}

/// Decide the convergence action from detected filesystem state. Pure and
/// side-effect free: `plan()`/`execute()` gather these inputs via
/// `process.exec` and hand them here.
fn decide(
    enabled: bool,
    precreate: bool,
    src_is_symlink: bool,
    src_symlink_target: Option<&str>,
    src_exists: bool,
    dst_exists: bool,
    dst: &str,
) -> Decision {
    if !enabled {
        return Decision::Disabled;
    }

    if src_is_symlink {
        return match src_symlink_target {
            Some(target) if target == dst => Decision::AlreadyLinked,
            Some(target) => Decision::Repoint {
                old_target: target.to_string(),
            },
            // `test -L` said it's a symlink but `readlink` produced nothing
            // (shouldn't normally happen) — treat defensively as drift so we
            // still converge to a correct link at `dst`.
            None => Decision::Repoint {
                old_target: String::new(),
            },
        };
    }

    if src_exists {
        return if dst_exists {
            Decision::Conflict
        } else {
            Decision::Move
        };
    }

    if dst_exists {
        return Decision::LinkOnly;
    }

    if precreate {
        Decision::Precreate
    } else {
        Decision::NothingYet
    }
}

/// Validate `src`/`dst`: both must be non-empty absolute paths, and must
/// differ. No `~` expansion — that is the caller's job.
fn validate(cfg: &SymlinkConfig) -> Result<(), EnsureError> {
    if cfg.src.is_empty() || !cfg.src.starts_with('/') {
        return Err(EnsureError::ConfigError(format!(
            "fs.symlink: 'src' must be a non-empty absolute path, got {:?}",
            cfg.src
        )));
    }
    if cfg.dst.is_empty() || !cfg.dst.starts_with('/') {
        return Err(EnsureError::ConfigError(format!(
            "fs.symlink: 'dst' must be a non-empty absolute path, got {:?}",
            cfg.dst
        )));
    }
    if cfg.src == cfg.dst {
        return Err(EnsureError::ConfigError(format!(
            "fs.symlink: 'src' and 'dst' must differ (both are {:?})",
            cfg.src
        )));
    }
    Ok(())
}

/// Parent directory of `path` as a string, for `mkdir -p`. Pure string
/// manipulation — no filesystem access.
fn parent_of(path: &str) -> String {
    match std::path::Path::new(path).parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_string_lossy().to_string(),
        _ => "/".to_string(),
    }
}

fn run(program: &str, args: &[String], cwd: &str) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let req = ExecRequest {
        program: program.to_string(),
        args: args.to_vec(),
        cwd: Some(cwd.to_string()),
        env: vec![],
        inherit_env: true,
        stdin: None,
    };
    let r = exec(&req)?;
    Ok((r.status as i32, r.stdout, r.stderr))
}

fn run_ok(cwd: &str, program: &str, args: &[String]) -> Result<(), EnsureError> {
    let (status, _, stderr) = run(program, args, cwd).map_err(EnsureError::ExecutionError)?;
    if status != 0 {
        return Err(EnsureError::ExecutionError(format!(
            "{} {} failed: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&stderr)
        )));
    }
    Ok(())
}

fn test_flag(cwd: &str, flag: &str, path: &str) -> Result<bool, String> {
    let (status, _, _) = run("test", &[flag.to_string(), path.to_string()], cwd)?;
    Ok(status == 0)
}

fn readlink_target(cwd: &str, path: &str) -> Result<Option<String>, String> {
    let (status, stdout, _) = run("readlink", &[path.to_string()], cwd)?;
    if status != 0 {
        return Ok(None);
    }
    let target = String::from_utf8_lossy(&stdout).trim().to_string();
    Ok(if target.is_empty() { None } else { Some(target) })
}

struct DetectedState {
    src_is_symlink: bool,
    src_symlink_target: Option<String>,
    src_exists: bool,
    dst_exists: bool,
}

/// Read-only detection: `test -L`, `test -e` (both paths), and `readlink`
/// when `src` is a symlink. Never mutates anything.
fn detect(app_path: &str, src: &str, dst: &str) -> Result<DetectedState, String> {
    let src_is_symlink = test_flag(app_path, "-L", src)?;
    let src_exists = test_flag(app_path, "-e", src)?;
    let dst_exists = test_flag(app_path, "-e", dst)?;
    let src_symlink_target = if src_is_symlink {
        readlink_target(app_path, src)?
    } else {
        None
    };
    Ok(DetectedState {
        src_is_symlink,
        src_symlink_target,
        src_exists,
        dst_exists,
    })
}

fn decide_for(cfg: &SymlinkConfig, state: &DetectedState) -> Decision {
    decide(
        cfg.enabled,
        cfg.precreate,
        state.src_is_symlink,
        state.src_symlink_target.as_deref(),
        state.src_exists,
        state.dst_exists,
        &cfg.dst,
    )
}

fn describe(decision: &Decision, src: &str, dst: &str) -> EnsurePlan {
    match decision {
        Decision::Disabled => EnsurePlan {
            description: format!("skip {src} (disabled)"),
            actions: vec![],
        },
        Decision::AlreadyLinked => EnsurePlan {
            description: format!("{src} is already linked to {dst}"),
            actions: vec![],
        },
        Decision::Repoint { old_target } => EnsurePlan {
            description: format!("repoint {src} from {old_target} to {dst}"),
            actions: vec![format!("rm {src}"), format!("ln -s {dst} {src}")],
        },
        Decision::Move => EnsurePlan {
            description: format!("relocate {src} to {dst} and symlink {src} back to it"),
            actions: vec![
                format!("mkdir -p {}", parent_of(dst)),
                format!("mv {src} {dst}"),
                format!("ln -s {dst} {src}"),
            ],
        },
        Decision::Conflict => EnsurePlan {
            description: format!(
                "CONFLICT: both {src} and {dst} contain data; resolve manually before running apply again"
            ),
            actions: vec![format!(
                "manual resolution required: both {src} and {dst} have real data"
            )],
        },
        Decision::LinkOnly => EnsurePlan {
            description: format!("{src} missing but {dst} already has the data; link {src} -> {dst}"),
            actions: vec![
                format!("mkdir -p {}", parent_of(src)),
                format!("ln -s {dst} {src}"),
            ],
        },
        Decision::Precreate => EnsurePlan {
            description: format!("precreate {dst} and link {src} -> {dst}"),
            actions: vec![
                format!("mkdir -p {dst}"),
                format!("mkdir -p {}", parent_of(src)),
                format!("ln -s {dst} {src}"),
            ],
        },
        Decision::NothingYet => EnsurePlan {
            description: format!("nothing to relocate yet ({src} and {dst} both absent)"),
            actions: vec![],
        },
    }
}

fn dry_run_message(decision: &Decision, src: &str, dst: &str) -> String {
    match decision {
        Decision::Disabled => format!("Skipped {src} (disabled by config)"),
        Decision::AlreadyLinked => format!("{src} is already linked to {dst}; no changes needed"),
        Decision::Repoint { old_target } => {
            format!("Would repoint {src} from {old_target} to {dst}")
        }
        Decision::Move => format!("Would move {src} to {dst} and symlink {src} -> {dst}"),
        Decision::Conflict => format!(
            "Would report conflict: both {src} and {dst} contain data; resolve manually before running apply again"
        ),
        Decision::LinkOnly => format!("Would link {src} -> {dst} (already relocated)"),
        Decision::Precreate => format!("Would create {dst} and link {src} -> it"),
        Decision::NothingYet => {
            format!("Nothing to relocate yet ({src} and {dst} both absent)")
        }
    }
}

fn perform(app_path: &str, decision: &Decision, src: &str, dst: &str) -> Result<String, EnsureError> {
    match decision {
        Decision::Disabled => Ok(format!("Skipped {src} (disabled by config)")),
        Decision::AlreadyLinked => Ok(format!("{src} already linked to {dst}")),
        Decision::NothingYet => Ok(format!(
            "Nothing to relocate yet ({src} and {dst} both absent)"
        )),
        Decision::Conflict => Err(EnsureError::ExecutionError(format!(
            "both {src} and {dst} contain data; resolve manually before running apply again"
        ))),
        Decision::Repoint { .. } => {
            run_ok(app_path, "rm", &[src.to_string()])?;
            run_ok(
                app_path,
                "ln",
                &["-s".to_string(), dst.to_string(), src.to_string()],
            )?;
            Ok(format!("Repointed {src} to {dst}"))
        }
        Decision::Move => {
            run_ok(app_path, "mkdir", &["-p".to_string(), parent_of(dst)])?;
            run_ok(app_path, "mv", &[src.to_string(), dst.to_string()])?;
            run_ok(
                app_path,
                "ln",
                &["-s".to_string(), dst.to_string(), src.to_string()],
            )?;
            Ok(format!("Relocated {src} to {dst} and linked it back"))
        }
        Decision::LinkOnly => {
            run_ok(app_path, "mkdir", &["-p".to_string(), parent_of(src)])?;
            run_ok(
                app_path,
                "ln",
                &["-s".to_string(), dst.to_string(), src.to_string()],
            )?;
            Ok(format!("Linked {src} -> {dst} (already relocated)"))
        }
        Decision::Precreate => {
            run_ok(app_path, "mkdir", &["-p".to_string(), dst.to_string()])?;
            run_ok(app_path, "mkdir", &["-p".to_string(), parent_of(src)])?;
            run_ok(
                app_path,
                "ln",
                &["-s".to_string(), dst.to_string(), src.to_string()],
            )?;
            Ok(format!("Precreated {dst} and linked {src} -> it"))
        }
    }
}

impl Guest for Component {
    fn plan(req: EnsureRequest) -> Result<EnsurePlan, EnsureError> {
        let cfg: SymlinkConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {e}")))?;
        validate(&cfg)?;

        if !cfg.enabled {
            return Ok(describe(&Decision::Disabled, &cfg.src, &cfg.dst));
        }

        let state = detect(&req.app_path, &cfg.src, &cfg.dst).map_err(EnsureError::ExecutionError)?;
        let decision = decide_for(&cfg, &state);
        Ok(describe(&decision, &cfg.src, &cfg.dst))
    }

    fn execute(req: EnsureRequest) -> Result<String, EnsureError> {
        let cfg: SymlinkConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {e}")))?;
        validate(&cfg)?;

        if !cfg.enabled {
            return Ok(format!("Skipped {} (disabled by config)", cfg.src));
        }

        let state = detect(&req.app_path, &cfg.src, &cfg.dst).map_err(EnsureError::ExecutionError)?;
        let decision = decide_for(&cfg, &state);

        if req.dry_run {
            return Ok(dry_run_message(&decision, &cfg.src, &cfg.dst));
        }

        perform(&req.app_path, &decision, &cfg.src, &cfg.dst)
    }
}

export!(Component);

#[cfg(test)]
mod decision_tests {
    use super::*;

    // symlink-correct: src already points at dst.
    #[test]
    fn symlink_pointing_at_dst_is_already_linked() {
        assert_eq!(
            decide(true, true, true, Some("/vault/dst"), true, true, "/vault/dst"),
            Decision::AlreadyLinked
        );
    }

    // symlink-drift: src is a symlink pointing somewhere else.
    #[test]
    fn symlink_pointing_elsewhere_is_repointed() {
        assert_eq!(
            decide(
                true,
                true,
                true,
                Some("/somewhere/else"),
                true,
                true,
                "/vault/dst"
            ),
            Decision::Repoint {
                old_target: "/somewhere/else".to_string()
            }
        );
    }

    // move-needed: real data at src, dst empty.
    #[test]
    fn real_data_at_src_with_no_dst_is_move_needed() {
        assert_eq!(
            decide(true, true, false, None, true, false, "/vault/dst"),
            Decision::Move
        );
    }

    // conflict: real data on both sides.
    #[test]
    fn real_data_on_both_sides_is_conflict() {
        assert_eq!(
            decide(true, true, false, None, true, true, "/vault/dst"),
            Decision::Conflict
        );
    }

    // relink-only: src absent, dst already has the data.
    #[test]
    fn missing_src_with_existing_dst_is_link_only() {
        assert_eq!(
            decide(true, true, false, None, false, true, "/vault/dst"),
            Decision::LinkOnly
        );
    }

    // precreate-true: neither exists, precreate enabled.
    #[test]
    fn neither_exists_with_precreate_true_precreates() {
        assert_eq!(
            decide(true, true, false, None, false, false, "/vault/dst"),
            Decision::Precreate
        );
    }

    // precreate-false: neither exists, precreate disabled.
    #[test]
    fn neither_exists_with_precreate_false_is_noop() {
        assert_eq!(
            decide(true, false, false, None, false, false, "/vault/dst"),
            Decision::NothingYet
        );
    }

    // disabled: short-circuits every other branch, including conflict.
    #[test]
    fn disabled_short_circuits_everything() {
        assert_eq!(
            decide(false, true, true, Some("/vault/dst"), true, true, "/vault/dst"),
            Decision::Disabled
        );
        assert_eq!(
            decide(false, true, false, None, true, true, "/vault/dst"),
            Decision::Disabled
        );
        assert_eq!(
            decide(false, false, false, None, false, false, "/vault/dst"),
            Decision::Disabled
        );
    }

    // Broken/unreadable symlink (test -L true but readlink yields nothing)
    // still converges defensively via Repoint rather than panicking.
    #[test]
    fn symlink_with_unreadable_target_is_repointed_defensively() {
        assert_eq!(
            decide(true, true, true, None, true, true, "/vault/dst"),
            Decision::Repoint {
                old_target: String::new()
            }
        );
    }
}

#[cfg(test)]
mod validate_tests {
    use super::*;

    fn cfg(src: &str, dst: &str) -> SymlinkConfig {
        SymlinkConfig {
            src: src.to_string(),
            dst: dst.to_string(),
            enabled: true,
            precreate: true,
        }
    }

    #[test]
    fn accepts_two_distinct_absolute_paths() {
        assert!(validate(&cfg("/a/b", "/c/d")).is_ok());
    }

    #[test]
    fn rejects_relative_src() {
        assert!(matches!(
            validate(&cfg("a/b", "/c/d")),
            Err(EnsureError::ConfigError(_))
        ));
    }

    #[test]
    fn rejects_relative_dst() {
        assert!(matches!(
            validate(&cfg("/a/b", "c/d")),
            Err(EnsureError::ConfigError(_))
        ));
    }

    #[test]
    fn rejects_empty_src() {
        assert!(matches!(
            validate(&cfg("", "/c/d")),
            Err(EnsureError::ConfigError(_))
        ));
    }

    #[test]
    fn rejects_empty_dst() {
        assert!(matches!(
            validate(&cfg("/a/b", "")),
            Err(EnsureError::ConfigError(_))
        ));
    }

    #[test]
    fn rejects_equal_src_and_dst() {
        assert!(matches!(
            validate(&cfg("/same/path", "/same/path")),
            Err(EnsureError::ConfigError(_))
        ));
    }
}

#[cfg(test)]
mod parent_of_tests {
    use super::*;

    #[test]
    fn returns_parent_directory() {
        assert_eq!(parent_of("/vault/caches/rustup"), "/vault/caches");
    }

    #[test]
    fn root_level_path_falls_back_to_root() {
        assert_eq!(parent_of("/rustup"), "/");
    }
}
