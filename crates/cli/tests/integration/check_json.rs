//! `wvr check --json` (R22; `docs/dev-standards-module-handoff.md`).
//!
//! The critical constraint under test: stdout under `--json` carries exactly
//! one JSON document and nothing else -- no "Running N checks...", no the
//! "Module '...' resolved to ..." line, no table. Every test here actually
//! deserializes stdout with `serde_json`, rather than grepping for braces,
//! because a grep would happily pass on a document corrupted by interleaved
//! log lines.

use crate::common::{TestContext, cmd};
use serde_json::Value;
use std::path::{Path, PathBuf};

const GIT_ENVS: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "Test"),
    ("GIT_AUTHOR_EMAIL", "test@example.com"),
    ("GIT_COMMITTER_NAME", "Test"),
    ("GIT_COMMITTER_EMAIL", "test@example.com"),
];

/// Create a real local git module repo at `root/<name>` containing a
/// `weaver.module.yaml` with `manifest_yaml`, committed and tagged `v1`.
/// Returns the repo's local path, usable directly as a module `source:`.
fn make_module(root: &Path, name: &str, manifest_yaml: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("weaver.module.yaml"), manifest_yaml).unwrap();

    std::process::Command::new("git").args(["init", "-q"]).current_dir(&dir).output().unwrap();
    std::process::Command::new("git").args(["add", "-A"]).current_dir(&dir).output().unwrap();
    let out = std::process::Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(&dir)
        .envs(GIT_ENVS)
        .output()
        .unwrap();
    assert!(out.status.success(), "git commit failed: {}", String::from_utf8_lossy(&out.stderr));
    let out = std::process::Command::new("git").args(["tag", "v1"]).current_dir(&dir).output().unwrap();
    assert!(out.status.success(), "git tag failed: {}", String::from_utf8_lossy(&out.stderr));

    dir
}

/// The commit `v1` resolves to in `module_dir` -- the full 40-char SHA,
/// computed independently of the CLI so the test isn't just checking the
/// engine against itself.
fn resolved_commit(module_dir: &Path) -> String {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "v1"])
        .current_dir(module_dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Run `wvr check --json` and parse stdout as a *single* JSON document.
/// Panics with the raw stdout/stderr on parse failure, or if stdout contains
/// anything beyond the one document (trailing bytes after it).
fn run_check_json(ctx: &TestContext) -> (Value, std::process::Output) {
    let mut c = cmd();
    let output = c.arg("check").arg("--json").env("HOME", ctx.temp.path()).current_dir(&ctx.root).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // `serde_json::from_str` fails if anything but trailing whitespace
    // follows the document, so this alone proves stdout is *exactly* one
    // JSON document -- not JSON with a log line appended, not JSON
    // interleaved with a table.
    let value: Value = serde_json::from_str(&stdout).unwrap_or_else(|err| {
        panic!(
            "stdout did not parse as a single JSON document: {err}\n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });

    (value, output)
}

#[test]
fn stdout_is_a_single_parseable_json_document() {
    let ctx = TestContext::new();
    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "check1"
        command: "echo 'ok'"
"#,
    );

    let (value, output) = run_check_json(&ctx);
    assert!(output.status.success(), "expected success, got: {output:?}");
    assert_eq!(value["version"], 1);
    assert!(value["results"].is_array());
    assert!(value["summary"].is_object());
}

#[test]
fn no_checks_defined_still_emits_a_valid_empty_report() {
    let ctx = TestContext::new();
    ctx.write_file("weaver.yaml", "version: '1'\napps: []");

    let (value, output) = run_check_json(&ctx);
    assert!(output.status.success());
    assert_eq!(value["results"].as_array().unwrap().len(), 0);
    assert_eq!(value["summary"]["total"], 0);
}

#[test]
fn module_inherited_failure_reports_full_fields() {
    let ctx = TestContext::new();
    let module_dir = make_module(
        &ctx.root,
        "stdmod",
        r#"
inputs: {}
checks:
  - id: no-todo-marker
    name: "No TODO markers"
    command: "! grep -r TODO ."
    severity: error
    remediate: "remove TODO markers from tracked files"
"#,
    );
    let full_commit = resolved_commit(&module_dir);

    ctx.write_file(
        "weaver.yaml",
        &format!(
            r#"
version: "1"
modules:
  - name: "stdmod"
    source: "{}"
    ref: "v1"
apps:
  - name: "app"
    module: "stdmod"
    path: "app"
"#,
            module_dir.display()
        ),
    );
    ctx.write_file("app/notes.txt", "TODO: fix this\n");

    let (value, output) = run_check_json(&ctx);
    assert_eq!(output.status.code(), Some(2), "error-severity failure must still exit 2 under --json");

    let results = value["results"].as_array().unwrap();
    let result = results
        .iter()
        .find(|r| r["qualified_id"] == "stdmod:no-todo-marker")
        .unwrap_or_else(|| panic!("no-todo-marker result missing from: {results:#?}"));

    assert_eq!(result["id"], "no-todo-marker");
    assert_eq!(result["name"], "No TODO markers");
    assert_eq!(result["severity"], "error");
    assert_eq!(result["status"], "fail");
    assert_eq!(result["source"]["kind"], "module");
    assert_eq!(result["source"]["module"], "stdmod");
    assert_eq!(result["source"]["app"], "app");
    // The FULL resolved commit, not the 8-char short form the table shows.
    assert_eq!(result["source"]["resolved_commit"], full_commit);
    assert_eq!(full_commit.len(), 40);
    assert!(result["observed"].as_str().unwrap().contains("exit code"));
    assert!(result["expected"].as_str().unwrap().contains("exit code 0"));
    assert_eq!(result["remediate"], "remove TODO markers from tracked files");
}

#[test]
fn summary_counts_match_the_results_array() {
    let ctx = TestContext::new();
    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "pass1"
        command: "exit 0"
        severity: error
      - name: "fail1"
        command: "exit 1"
        severity: error
      - name: "fail2"
        command: "exit 1"
        severity: warn
"#,
    );

    let (value, output) = run_check_json(&ctx);
    assert_eq!(output.status.code(), Some(2));

    let results = value["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);

    let summary = &value["summary"];
    assert_eq!(summary["total"], 3);
    assert_eq!(summary["pass"], 1);
    assert_eq!(summary["fail"], 2);
    assert_eq!(summary["error"], 0);
    // Only the error-severity failure counts as a gate failure.
    assert_eq!(summary["gate_failures"], 1);
}

#[test]
fn warn_only_failure_exits_zero_under_json() {
    let ctx = TestContext::new();
    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "check1"
        command: "exit 1"
        severity: warn
"#,
    );

    let (value, output) = run_check_json(&ctx);
    assert!(output.status.success(), "warn-only failure must exit 0, got: {output:?}");
    assert_eq!(value["summary"]["gate_failures"], 0);
}
