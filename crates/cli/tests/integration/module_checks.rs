//! Module-inherited checks (R1, R2, R8; `docs/dev-standards-module-handoff.md`).
//!
//! Unlike `check.rs`'s existing tests (which reference a module name that's
//! never declared under `modules:`, and so never touch module resolution at
//! all), every test here builds a *real* local git module repo -- module
//! resolution always shells out to `git`, even for local paths (see
//! `apply.rs`'s `test_apply_then_plan_is_idempotent_for_heading_and_block_marker_sections`
//! for the same pattern).

use crate::common::{TestContext, cmd};
use predicates::prelude::*;
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

/// The commit `v1` resolves to in `module_dir` -- exactly what
/// `git::rev_parse_remote` would pin for a lightweight tag, computed
/// independently so the test isn't just checking the engine against itself.
fn resolved_commit(module_dir: &Path) -> String {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "v1"])
        .current_dir(module_dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn single_app_weaver_yaml(module_dir: &Path, extra_top_level: &str, app_extra: &str) -> String {
    format!(
        r#"
version: "1"
{extra_top_level}
modules:
  - name: "stdmod"
    source: "{}"
    ref: "v1"
apps:
  - name: "app"
    module: "stdmod"
    path: "app"
{app_extra}
"#,
        module_dir.display()
    )
}

/// R1 acceptance, exactly as written in the handoff doc: a module declaring
/// one check, adopted by a repo whose own `weaver.yaml` declares no checks,
/// makes `wvr check` fail when the repo violates it, and pass when it
/// doesn't.
#[test]
fn r1_acceptance_module_check_gates_a_repo_with_no_checks_of_its_own() {
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
"#,
    );

    // weaver.yaml declares NO checks of its own -- neither top-level nor on
    // the app.
    ctx.write_file("weaver.yaml", &single_app_weaver_yaml(&module_dir, "", ""));

    // Violating: app/ contains a TODO marker.
    ctx.write_file("app/notes.txt", "TODO: fix this\n");
    cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .code(2)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("no-todo-marker"));

    // Conformant: fix the violation.
    ctx.write_file("app/notes.txt", "all good\n");
    cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"))
        .stdout(predicate::str::contains("no-todo-marker"));
}

/// R2 acceptance: the failing result's output contains the module name, the
/// resolved commit, and the rule id.
#[test]
fn r2_acceptance_failing_module_check_names_module_commit_and_rule_id() {
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
"#,
    );
    let commit = resolved_commit(&module_dir);
    assert_eq!(commit.len(), 40, "expected a full 40-char sha, got {commit:?}");
    let short = &commit[..8];

    ctx.write_file("weaver.yaml", &single_app_weaver_yaml(&module_dir, "", ""));
    ctx.write_file("app/notes.txt", "TODO: fix this\n");

    let assert = cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .code(2);

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("stdmod"), "module name missing from output:\n{stdout}");
    assert!(stdout.contains(&commit), "full resolved commit missing from output:\n{stdout}");
    assert!(
        stdout.contains(&format!("stdmod@{short}")),
        "module@<short-sha> context label missing from output:\n{stdout}"
    );
    assert!(stdout.contains("no-todo-marker"), "rule id missing from output:\n{stdout}");
}

/// R8: resolve the module once (via `apply`), then make the module's source
/// repo unavailable, and confirm `wvr check` still runs the inherited
/// checks from cache. This is the real proof there's no network dependency.
#[test]
fn r8_offline_check_runs_inherited_checks_from_cache_after_source_disappears() {
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
"#,
    );

    ctx.write_file("weaver.yaml", &single_app_weaver_yaml(&module_dir, "", ""));
    ctx.write_file("app/notes.txt", "all good\n");

    // Resolve the module once via `apply` -- pins the commit in
    // weaver.lock and warms the module cache under $HOME/.rw/store.
    cmd()
        .arg("apply")
        .arg("--auto-approve")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success();
    assert!(ctx.root.join("weaver.lock").exists(), "apply should have written weaver.lock");

    // Make the module's source repo unavailable.
    std::fs::rename(&module_dir, ctx.root.join("stdmod-moved-away")).unwrap();

    // `check` must still run the inherited check from the warm cache,
    // without ever trying to reach the (now-gone) module source.
    cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success()
        .stdout(predicate::str::contains("no-todo-marker"))
        .stdout(predicate::str::contains("PASS"));
}

/// A module that has never been resolved (no lockfile pin, no warm cache)
/// and whose source is unreachable must fail loudly, not silently skip its
/// checks (no false green, requirements §6).
#[test]
fn unresolvable_module_fails_the_check_run_instead_of_being_skipped() {
    let ctx = TestContext::new();
    let module_dir = ctx.root.join("never-created-stdmod");

    ctx.write_file("weaver.yaml", &single_app_weaver_yaml(&module_dir, "", ""));
    ctx.write_file("app/notes.txt", "all good\n");

    let assert = cmd().arg("check").env("HOME", ctx.temp.path()).current_dir(&ctx.root).assert().failure();

    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("wvr apply"),
        "expected a clear 'run wvr apply first' error, got:\n{stderr}"
    );
}

/// Workspace-, app- and module-level checks all run together in one
/// invocation, and each is attributed correctly.
#[test]
fn workspace_app_and_module_checks_all_run_together_with_correct_attribution() {
    let ctx = TestContext::new();
    let module_dir = make_module(
        &ctx.root,
        "stdmod",
        r#"
inputs: {}
checks:
  - id: module-rule
    name: "Module rule"
    command: "exit 0"
"#,
    );

    ctx.write_file(
        "weaver.yaml",
        &single_app_weaver_yaml(
            &module_dir,
            "checks:\n  - id: workspace-rule\n    name: \"Workspace rule\"\n    command: \"exit 0\"\n",
            "    checks:\n      - id: app-rule\n        name: \"App rule\"\n        command: \"exit 0\"\n",
        ),
    );
    ctx.write_file("app/.gitkeep", "");

    let assert = cmd().arg("check").env("HOME", ctx.temp.path()).current_dir(&ctx.root).assert().success();

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Workspace check, attributed to "Global".
    assert!(stdout.contains("workspace-rule"), "{stdout}");
    assert!(stdout.contains("Global"), "workspace context missing:\n{stdout}");
    // App check, attributed to the app name.
    assert!(stdout.contains("app-rule"), "{stdout}");
    // Module check, attributed to `module@<sha> (app)`.
    assert!(stdout.contains("module-rule"), "{stdout}");
    assert!(stdout.contains("stdmod@"), "module context missing:\n{stdout}");
    assert!(stdout.contains("(app)"), "module check not attributed to its consuming app:\n{stdout}");
}

/// Two apps consuming the same module run its checks once per app, each
/// attributed to the right app.
#[test]
fn two_apps_sharing_a_module_run_its_checks_once_per_app() {
    let ctx = TestContext::new();
    let module_dir = make_module(
        &ctx.root,
        "stdmod",
        r#"
inputs: {}
checks:
  - id: shared-rule
    name: "Shared rule"
    command: "exit 0"
"#,
    );

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
  - name: "app-one"
    module: "stdmod"
    path: "app-one"
  - name: "app-two"
    module: "stdmod"
    path: "app-two"
"#,
            module_dir.display()
        ),
    );
    ctx.write_file("app-one/.gitkeep", "");
    ctx.write_file("app-two/.gitkeep", "");

    let assert = cmd().arg("check").env("HOME", ctx.temp.path()).current_dir(&ctx.root).assert().success();

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        stdout.matches("(app-one)").count(),
        1,
        "expected exactly one module-check row attributed to app-one:\n{stdout}"
    );
    assert_eq!(
        stdout.matches("(app-two)").count(),
        1,
        "expected exactly one module-check row attributed to app-two:\n{stdout}"
    );

    // The module itself is resolved once per run, not once per consuming app.
    assert_eq!(
        stdout.matches("Module 'stdmod' resolved to").count(),
        1,
        "module should be resolved once, not once per app:\n{stdout}"
    );
}
