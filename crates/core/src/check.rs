//! Check runner (R20, R23; `docs/dev-standards-module-handoff.md`;
//! `specs/004-generic-standardization-engine/spec.md` FR-024).
//!
//! Evaluates a single [`CheckDef`] against a repo root and produces a
//! [`CheckResult`]. This lives in core (not the CLI) because it is the piece
//! two later features build on directly: module-inherited checks run every
//! check through this same runner, and `--json` output serializes
//! [`CheckResult`] as-is. The CLI's `check` command is presentation only.
//!
//! ## No false green (§6)
//!
//! A check that cannot actually be evaluated -- the command fails to spawn,
//! the timeout expires, or `stdout_matches` is not a valid regex -- reports
//! [`Status::Error`], never [`Status::Pass`]. A linter that silently passes
//! when it couldn't run is worse than no linter.

use crate::config::{CheckDef, Severity};
use serde::Serialize;
use std::path::Path;
use std::time::Duration;

/// The outcome of evaluating a single check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Exit code matched `expect`, and `stdout_contains`/`stdout_matches`
    /// (when set) were both satisfied.
    Pass,
    /// The check ran to completion but did not satisfy its assertions.
    Fail,
    /// The check could not be evaluated at all -- see the module docs' "no
    /// false green" section. Never conflated with `Pass`.
    Error,
}

/// Where a check's rule definition came from (R2).
///
/// Structured, not a pre-formatted string, because `--json` output (a later
/// feature) needs the module name / resolved commit / consuming app as
/// separate fields rather than something it has to re-parse out of a
/// human-readable label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CheckSource {
    /// Declared directly on the workspace `weaver.yaml`'s top-level
    /// `checks:` list.
    Workspace,
    /// Declared on a specific app's `checks:` list.
    App {
        app: String,
    },
    /// Inherited from a module's manifest `checks:` (R1), evaluated once per
    /// consuming app so a module check can assert about the app it
    /// configures.
    Module {
        module: String,
        /// The real pinned commit SHA the module ref resolved to (from the
        /// lockfile/resolver) -- never the ref name (R2).
        resolved_commit: String,
        app: String,
    },
}

impl CheckSource {
    /// The report's "Context" column: `"Global"` for a workspace check, the
    /// app name for an app-level check, and `module@<short-sha> (app)` for a
    /// module-inherited check -- R2's accept criterion is that a failing
    /// rule's report names `module@<sha>`.
    pub fn context_label(&self) -> String {
        match self {
            CheckSource::Workspace => "Global".to_string(),
            CheckSource::App { app } => app.clone(),
            CheckSource::Module { module, resolved_commit, app } => {
                let short = &resolved_commit[..resolved_commit.len().min(8)];
                format!("{module}@{short} ({app})")
            }
        }
    }

    /// Qualify a rule's authored `id` for cross-module uniqueness (R3): two
    /// modules can both define an id like `no-todo`, so a module check's
    /// qualified id is `<module>:<id>`. Workspace/app checks are already
    /// unique within a run and stay unqualified. Waivers (R31) reference
    /// this qualified form.
    pub fn qualify(&self, id: &str) -> String {
        match self {
            CheckSource::Module { module, .. } => format!("{module}:{id}"),
            CheckSource::Workspace | CheckSource::App { .. } => id.to_string(),
        }
    }
}

/// The evaluated outcome of a single [`CheckDef`], carrying everything a
/// reporter needs without re-running anything: the rule id, name, severity,
/// status, what was observed, what was expected, provenance, and how to fix
/// it.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Stable rule id (R3): `CheckDef::id`, falling back to `name` when unset
    /// so legacy checks still get a usable identifier.
    pub id: String,
    /// `id` qualified by [`CheckSource::qualify`] -- unique across every
    /// check in a single `wvr check` run, even when two modules (or a module
    /// and the local repo) both author the same bare id.
    pub qualified_id: String,
    pub name: String,
    pub severity: Severity,
    pub status: Status,
    /// What was actually observed (exit code, and/or why evaluation failed).
    pub observed: String,
    /// What was expected, in human-readable form (exit code / substring /
    /// pattern), for the report.
    pub expected: String,
    pub remediate: Option<String>,
    /// Where this rule came from (R2): workspace, app, or a specific module
    /// at its resolved commit, consumed by a specific app.
    pub source: CheckSource,
}

impl CheckResult {
    fn new(check: &CheckDef, status: Status, observed: impl Into<String>, source: CheckSource) -> Self {
        let id = check.id.clone().unwrap_or_else(|| check.name.clone());
        let qualified_id = source.qualify(&id);
        Self {
            id,
            qualified_id,
            name: check.name.clone(),
            severity: check.severity,
            status,
            observed: observed.into(),
            expected: expected_description(check),
            remediate: check.remediate.clone(),
            source,
        }
    }
}

fn expected_description(check: &CheckDef) -> String {
    let mut parts = vec![format!("exit code {}", check.expect)];
    if let Some(substr) = &check.stdout_contains {
        parts.push(format!("stdout contains {substr:?}"));
    }
    if let Some(pattern) = &check.stdout_matches {
        parts.push(format!("stdout matches /{pattern}/"));
    }
    parts.join(", ")
}

/// Evaluate one check: spawn `check.command` in a shell, in `check.cwd`
/// (resolved relative to `repo_root`) when set, else in `repo_root` itself.
///
/// `source` records where this check's rule definition came from (R2) --
/// workspace, a specific app, or a module at its resolved commit consumed by
/// a specific app -- and is carried onto the returned [`CheckResult`]
/// unchanged.
///
/// Never panics and never returns `Err` -- an unevaluatable check is
/// reported as [`Status::Error`] in the returned [`CheckResult`], not as a
/// `Result::Err`, so callers always get a reportable result per check.
pub async fn run_check(check: &CheckDef, repo_root: &Path, source: CheckSource) -> CheckResult {
    // Validate the regex before spawning anything: we should never run a
    // command we can't actually judge.
    let matcher = match &check.stdout_matches {
        Some(pattern) => match regex::Regex::new(pattern) {
            Ok(re) => Some(re),
            Err(err) => {
                return CheckResult::new(
                    check,
                    Status::Error,
                    format!("invalid stdout_matches regex /{pattern}/: {err}"),
                    source,
                );
            }
        },
        None => None,
    };

    let cwd = match &check.cwd {
        Some(rel) => repo_root.join(rel),
        None => repo_root.to_path_buf(),
    };

    let mut command = tokio::process::Command::new("sh");
    command.arg("-c").arg(&check.command).current_dir(&cwd);

    let run = command.output();
    let output = match check.timeout {
        Some(secs) => match tokio::time::timeout(Duration::from_secs(secs), run).await {
            Ok(result) => result,
            Err(_elapsed) => {
                return CheckResult::new(check, Status::Error, format!("timed out after {secs}s"), source);
            }
        },
        None => run.await,
    };

    let output = match output {
        Ok(output) => output,
        Err(err) => {
            return CheckResult::new(check, Status::Error, format!("failed to run command: {err}"), source);
        }
    };

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout_raw = String::from_utf8_lossy(&output.stdout);
    // Normalize stdout the same way shell command substitution (`$(...)`)
    // does: strip only *trailing* newlines. Leading and interior bytes
    // (including interior newlines) are left exactly alone. This is what
    // `stdout_contains` and `stdout_matches` are evaluated against, and it's
    // also exactly what gets reported back in `observed` on a failure -- the
    // comparison and the diagnostic must never be able to disagree.
    //
    // Without this, a command like `echo v1.2.3` (which emits a trailing
    // `\n`) would fail the entirely natural pattern `^v\d+\.\d+\.\d+$`,
    // because the regex crate's `$` anchors at the true end of the haystack
    // and does not special-case a trailing newline the way some other regex
    // engines do.
    let stdout = stdout_raw.trim_end_matches('\n');

    let mut problems = Vec::new();
    if exit_code != check.expect {
        problems.push(format!("exit code {exit_code} (expected {})", check.expect));
    }
    if let Some(substr) = &check.stdout_contains
        && !stdout.contains(substr.as_str())
    {
        problems.push(format!("stdout did not contain {substr:?}"));
    }
    if let Some(re) = &matcher
        && !re.is_match(stdout)
    {
        problems.push(format!("stdout did not match /{}/", check.stdout_matches.as_deref().unwrap_or("")));
    }

    if problems.is_empty() {
        CheckResult::new(check, Status::Pass, format!("exit code {exit_code}"), source)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = if !stderr.trim().is_empty() {
            format!("{}; stderr: {}", problems.join("; "), stderr.trim())
        } else if !stdout.is_empty() {
            // `stdout` here is exactly the (trailing-newline-stripped)
            // string that was compared above -- report that, not a
            // separately-trimmed value, or the message can lie about what
            // was actually matched.
            format!("{}; {}", problems.join("; "), stdout)
        } else {
            problems.join("; ")
        };
        CheckResult::new(check, Status::Fail, detail, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn base_check(command: &str) -> CheckDef {
        CheckDef {
            name: "test-check".to_string(),
            command: command.to_string(),
            description: None,
            id: None,
            severity: Severity::Error,
            expect: 0,
            stdout_contains: None,
            stdout_matches: None,
            timeout: None,
            cwd: None,
            remediate: None,
        }
    }

    #[tokio::test]
    async fn default_expect_zero_passes_on_success() {
        let check = base_check("exit 0");
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Pass);
    }

    #[tokio::test]
    async fn default_expect_zero_fails_on_nonzero_exit() {
        let check = base_check("exit 1");
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Fail);
    }

    #[tokio::test]
    async fn non_zero_expect_satisfied_passes() {
        let mut check = base_check("exit 3");
        check.expect = 3;
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Pass);
    }

    #[tokio::test]
    async fn non_zero_expect_not_satisfied_fails() {
        let mut check = base_check("exit 2");
        check.expect = 3;
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Fail);
    }

    #[tokio::test]
    async fn stdout_contains_hit_passes() {
        let mut check = base_check("echo hello-world");
        check.stdout_contains = Some("hello".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Pass);
    }

    #[tokio::test]
    async fn stdout_contains_miss_fails() {
        let mut check = base_check("echo hello-world");
        check.stdout_contains = Some("goodbye".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Fail);
    }

    #[tokio::test]
    async fn stdout_matches_hit_passes() {
        let mut check = base_check("echo hello-123");
        check.stdout_matches = Some(r"^hello-\d+".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Pass);
    }

    #[tokio::test]
    async fn stdout_matches_miss_fails() {
        let mut check = base_check("echo hello-123");
        check.stdout_matches = Some(r"^goodbye-\d+".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Fail);
    }

    #[tokio::test]
    async fn stdout_matches_end_anchor_passes_despite_shells_trailing_newline() {
        // `echo` always emits a trailing `\n`. The natural end-anchored
        // pattern for its output must still pass -- this is the exact shape
        // of the reported defect.
        let mut check = base_check("echo v1.2.3");
        check.stdout_matches = Some(r"^v[0-9]+\.[0-9]+\.[0-9]+$".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Pass, "observed: {}", result.observed);
    }

    #[tokio::test]
    async fn failing_stdout_match_reports_the_exact_compared_value() {
        let mut check = base_check("echo v1.2.3");
        check.stdout_matches = Some(r"^v9\.9\.9$".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Fail);
        // The diagnostic must show precisely what was compared against the
        // pattern -- never a value the comparison itself disagrees with.
        assert!(result.observed.contains("v1.2.3"), "observed: {}", result.observed);
        assert!(
            !result.observed.contains("v1.2.3\n"),
            "observed retained the trailing newline that was stripped before matching: {}",
            result.observed
        );
    }

    #[tokio::test]
    async fn stdout_matches_preserves_interior_newlines_for_multiline_mode() {
        // Only the *trailing* newline is stripped -- interior newlines must
        // survive so `(?m)` line-anchoring still works, i.e. this is not a
        // general whitespace strip.
        let mut check = base_check("printf 'line1\\nline2\\n'");
        check.stdout_matches = Some(r"(?m)^line2$".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Pass, "observed: {}", result.observed);
    }

    #[tokio::test]
    async fn stdout_contains_preserves_interior_newlines() {
        let mut check = base_check("printf 'line1\\nline2\\n'");
        check.stdout_contains = Some("line1\nline2".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Pass, "observed: {}", result.observed);
    }

    #[tokio::test]
    async fn stdout_matches_invalid_regex_errors_not_passes() {
        let mut check = base_check("echo hello");
        check.stdout_matches = Some("(unclosed".to_string());
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Error);
    }

    #[tokio::test]
    async fn timeout_expiring_errors_not_fails() {
        let mut check = base_check("sleep 5");
        check.timeout = Some(1);
        let result = run_check(&check, Path::new("."), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Error);
        assert!(result.observed.contains("timed out"), "observed was: {}", result.observed);
    }

    #[tokio::test]
    async fn cwd_is_honored() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("marker.txt"), "present").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();

        let mut check = base_check("test -f marker.txt");
        check.cwd = None;
        let result = run_check(&check, dir.path(), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Pass, "expected marker.txt to be found in repo root");

        let mut check_in_sub = base_check("test -f marker.txt");
        check_in_sub.cwd = Some("sub".to_string());
        let result = run_check(&check_in_sub, dir.path(), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Fail, "marker.txt should not be visible from sub/");
    }

    #[tokio::test]
    async fn spawn_failure_errors_not_passes() {
        // A `cwd` that does not exist makes the shell itself fail to spawn
        // with that working directory.
        let mut check = base_check("echo hi");
        check.cwd = Some("this/path/does/not/exist".to_string());
        let result = run_check(&check, Path::new("/tmp"), CheckSource::Workspace).await;
        assert_eq!(result.status, Status::Error);
    }

    // -- CheckSource provenance (R2, R3) --------------------------------

    #[test]
    fn workspace_context_label_is_global() {
        assert_eq!(CheckSource::Workspace.context_label(), "Global");
    }

    #[test]
    fn app_context_label_is_the_app_name() {
        let source = CheckSource::App { app: "billing".to_string() };
        assert_eq!(source.context_label(), "billing");
    }

    #[test]
    fn module_context_label_is_module_at_short_sha_with_app() {
        let source = CheckSource::Module {
            module: "dev-standards".to_string(),
            resolved_commit: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
            app: "billing".to_string(),
        };
        assert_eq!(source.context_label(), "dev-standards@abcdef12 (billing)");
    }

    #[test]
    fn module_context_label_does_not_panic_on_a_short_commit() {
        // Defensive: production commits are always 40 hex chars, but the
        // label must not panic if a shorter string ever reaches it.
        let source = CheckSource::Module {
            module: "m".to_string(),
            resolved_commit: "abc".to_string(),
            app: "app".to_string(),
        };
        assert_eq!(source.context_label(), "m@abc (app)");
    }

    #[test]
    fn workspace_and_app_ids_are_unqualified() {
        assert_eq!(CheckSource::Workspace.qualify("no-todo"), "no-todo");
        let app_source = CheckSource::App { app: "billing".to_string() };
        assert_eq!(app_source.qualify("no-todo"), "no-todo");
    }

    #[test]
    fn module_ids_are_qualified_by_module_name() {
        let source = CheckSource::Module {
            module: "dev-standards".to_string(),
            resolved_commit: "a".repeat(40),
            app: "billing".to_string(),
        };
        assert_eq!(source.qualify("no-todo"), "dev-standards:no-todo");
    }

    #[test]
    fn two_modules_with_the_same_authored_id_qualify_to_distinct_ids() {
        let a = CheckSource::Module {
            module: "mod-a".to_string(),
            resolved_commit: "a".repeat(40),
            app: "app".to_string(),
        };
        let b = CheckSource::Module {
            module: "mod-b".to_string(),
            resolved_commit: "b".repeat(40),
            app: "app".to_string(),
        };
        assert_ne!(a.qualify("no-todo"), b.qualify("no-todo"));
    }

    #[tokio::test]
    async fn check_result_carries_its_source_and_qualified_id() {
        let mut check = base_check("exit 1");
        check.id = Some("no-todo".to_string());
        let source = CheckSource::Module {
            module: "dev-standards".to_string(),
            resolved_commit: "a".repeat(40),
            app: "billing".to_string(),
        };
        let result = run_check(&check, Path::new("."), source.clone()).await;
        assert_eq!(result.id, "no-todo");
        assert_eq!(result.qualified_id, "dev-standards:no-todo");
        assert_eq!(result.source, source);
    }
}
