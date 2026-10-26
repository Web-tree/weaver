//! `modules[].path`: a module that lives in a subdirectory of its git repo,
//! so a standards repo can ship modules beside the code they describe.

use crate::common::{TestContext, cmd};
use predicates::prelude::*;
use std::path::{Path, PathBuf};

const GIT_ENVS: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "Test"),
    ("GIT_AUTHOR_EMAIL", "test@example.com"),
    ("GIT_COMMITTER_NAME", "Test"),
    ("GIT_COMMITTER_EMAIL", "test@example.com"),
];

/// A git repo at `root/<name>` whose module manifest sits at `subdir`, with a
/// decoy manifest at the repo root that must never be read. Tagged `v1`.
fn make_repo_with_module_in(root: &Path, name: &str, subdir: &str, manifest_yaml: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join(subdir)).unwrap();
    std::fs::write(dir.join(subdir).join("weaver.module.yaml"), manifest_yaml).unwrap();
    std::fs::write(
        dir.join("weaver.module.yaml"),
        "checks:\n  - id: decoy\n    name: decoy\n    command: \"false\"\n",
    )
    .unwrap();

    for args in [
        vec!["init", "-q"],
        vec!["add", "-A"],
        vec!["commit", "-q", "-m", "init"],
        vec!["tag", "v1"],
    ] {
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(&dir)
            .envs(GIT_ENVS)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    dir
}

fn weaver_yaml(repo: &Path, path_line: &str) -> String {
    format!(
        r#"
version: "1"
modules:
  - name: "stdmod"
    source: "{}"
    ref: "v1"
{path_line}
apps:
  - name: "app"
    module: "stdmod"
    path: "app"
"#,
        repo.display()
    )
}

#[test]
fn check_runs_the_checks_of_a_module_in_a_subdirectory() {
    let ctx = TestContext::new();
    let repo = make_repo_with_module_in(
        &ctx.root,
        "standards",
        "weaver/stdmod",
        r#"
checks:
  - id: has-readme
    name: "README present"
    command: "test -f README.md"
"#,
    );
    ctx.write_file(
        "weaver.yaml",
        &weaver_yaml(&repo, "    path: \"weaver/stdmod\""),
    );
    ctx.write_file("app/other.txt", "x\n");

    cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .code(2)
        .stdout(predicate::str::contains("has-readme"))
        .stdout(predicate::str::contains("decoy").not());

    ctx.write_file("app/README.md", "hi\n");
    cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success()
        .stdout(predicate::str::contains("has-readme"));
}

#[test]
fn apply_renders_templates_from_the_module_subdirectory() {
    let ctx = TestContext::new();
    let repo = make_repo_with_module_in(&ctx.root, "standards", "weaver/stdmod", "inputs: {}\n");
    std::fs::create_dir_all(repo.join("weaver/stdmod/templates")).unwrap();
    std::fs::write(
        repo.join("weaver/stdmod/templates/hello.txt"),
        "from subdir\n",
    )
    .unwrap();
    for args in [
        vec!["add", "-A"],
        vec!["commit", "-q", "-m", "tpl"],
        vec!["tag", "-f", "v1"],
    ] {
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(&repo)
            .envs(GIT_ENVS)
            .output()
            .unwrap();
        assert!(out.status.success());
    }
    ctx.write_file(
        "weaver.yaml",
        &weaver_yaml(&repo, "    path: \"weaver/stdmod\""),
    );

    cmd()
        .args(["apply", "--auto-approve"])
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(ctx.root.join("app/hello.txt")).unwrap(),
        "from subdir\n"
    );
}

#[test]
fn a_path_escaping_the_module_repo_is_rejected() {
    let ctx = TestContext::new();
    let repo = make_repo_with_module_in(&ctx.root, "standards", "weaver/stdmod", "inputs: {}\n");
    ctx.write_file(
        "weaver.yaml",
        &weaver_yaml(&repo, "    path: \"../outside\""),
    );

    cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("../outside"));
}

#[test]
fn a_missing_module_path_is_an_error_not_an_empty_module() {
    let ctx = TestContext::new();
    let repo = make_repo_with_module_in(&ctx.root, "standards", "weaver/stdmod", "inputs: {}\n");
    ctx.write_file(
        "weaver.yaml",
        &weaver_yaml(&repo, "    path: \"weaver/nope\""),
    );

    cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("weaver/nope"))
        .stderr(predicate::str::contains("modules found: weaver/stdmod"));
}

fn describe_json(ctx: &TestContext) -> serde_json::Value {
    let out = cmd()
        .args(["describe", "app", "--json"])
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn describe_shows_the_module_path() {
    let ctx = TestContext::new();
    let repo = make_repo_with_module_in(&ctx.root, "standards", "weaver/stdmod", "inputs: {}\n");
    ctx.write_file(
        "weaver.yaml",
        &weaver_yaml(&repo, "    path: \"weaver/stdmod\""),
    );

    cmd()
        .args(["describe", "app"])
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Module path: weaver/stdmod"));
    assert_eq!(describe_json(&ctx)["module_path"], "weaver/stdmod");
}

#[test]
fn describe_json_has_a_null_module_path_when_unset() {
    let ctx = TestContext::new();
    let repo = make_repo_with_module_in(&ctx.root, "standards", "weaver/stdmod", "inputs: {}\n");
    ctx.write_file("weaver.yaml", &weaver_yaml(&repo, ""));

    assert_eq!(
        describe_json(&ctx).get("module_path"),
        Some(&serde_json::Value::Null)
    );
    cmd()
        .args(["describe", "app"])
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Module path").not());
}
