//! Symlinks committed inside a module: the pinned commit vouches for the
//! checkout only, so a link may point anywhere inside it and nowhere else.
#![cfg(unix)]

use crate::common::{TestContext, cmd};
use predicates::prelude::*;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

const GIT_ENVS: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "Test"),
    ("GIT_AUTHOR_EMAIL", "test@example.com"),
    ("GIT_COMMITTER_NAME", "Test"),
    ("GIT_COMMITTER_EMAIL", "test@example.com"),
];

/// A module repo at `root/standards`, shaped by `build` and tagged `v1`.
fn module_repo(root: &Path, build: impl FnOnce(&Path)) -> PathBuf {
    let dir = root.join("standards");
    std::fs::create_dir_all(dir.join("files")).unwrap();
    std::fs::write(dir.join("weaver.module.yaml"), "inputs: {}\n").unwrap();
    build(&dir);
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

fn use_module(ctx: &TestContext, repo: &Path) {
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
            repo.display()
        ),
    );
}

#[test]
fn apply_follows_a_symlink_that_stays_inside_the_checkout() {
    let ctx = TestContext::new();
    let repo = module_repo(&ctx.root, |dir| {
        std::fs::create_dir_all(dir.join("shared")).unwrap();
        std::fs::write(dir.join("shared/editorconfig"), "root = true\n").unwrap();
        symlink("../shared/editorconfig", dir.join("files/.editorconfig")).unwrap();
        symlink("shared", dir.join("templates")).unwrap();
    });
    use_module(&ctx, &repo);

    cmd()
        .args(["apply", "--auto-approve"])
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(ctx.root.join("app/.editorconfig")).unwrap(),
        "root = true\n"
    );
    assert_eq!(
        std::fs::read_to_string(ctx.root.join("app/editorconfig")).unwrap(),
        "root = true\n"
    );
}

#[test]
fn a_manifest_symlinked_onto_the_host_fails_to_load() {
    let ctx = TestContext::new();
    let host_manifest = ctx.root.join("host-manifest.yaml");
    std::fs::write(&host_manifest, "checks: []\n").unwrap();
    let repo = module_repo(&ctx.root, |dir| {
        std::fs::remove_file(dir.join("weaver.module.yaml")).unwrap();
        symlink(&host_manifest, dir.join("weaver.module.yaml")).unwrap();
    });
    use_module(&ctx, &repo);

    for args in [vec!["apply", "--auto-approve"], vec!["check"]] {
        cmd()
            .args(&args)
            .env("HOME", ctx.temp.path())
            .current_dir(&ctx.root)
            .assert()
            .failure()
            .stderr(predicate::str::contains("'weaver.module.yaml'"))
            .stderr(predicate::str::contains("outside the module repository"));
    }
}

#[test]
fn a_files_entry_symlinked_onto_the_host_is_never_copied() {
    let ctx = TestContext::new();
    let host_file = ctx.root.join("host-secret");
    std::fs::write(&host_file, "secret\n").unwrap();
    let repo = module_repo(&ctx.root, |dir| {
        symlink(&host_file, dir.join("files/config")).unwrap();
    });
    use_module(&ctx, &repo);

    cmd()
        .args(["apply", "--auto-approve"])
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .failure()
        .stderr(predicate::str::contains("'files/config'"));
    assert!(!ctx.root.join("app/config").exists());
}
