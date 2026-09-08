use crate::common::{TestContext, weaver_config};
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_update_no_drift() {
    let ctx = TestContext::new();

    // 1. Initial Apply
    ctx.setup_module("my-mod", "v1", "file v1 content");
    ctx.write_file("weaver.yaml", &weaver_config("my-mod", "v1", &ctx.root));

    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("apply")
        .assert()
        .success();

    assert!(ctx.root.join("app/file.txt").exists());
    assert_eq!(ctx.read_file("app/file.txt"), "file v1 content");

    // 2. Update Config
    ctx.setup_module("my-mod", "v2", "file v2 content");
    ctx.write_file("weaver.yaml", &weaver_config("my-mod", "v2", &ctx.root));

    // 3. Apply Update
    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("apply")
        .assert()
        .success();

    assert_eq!(ctx.read_file("app/file.txt"), "file v2 content");
}

#[test]
fn test_drift_detected_stop() {
    let ctx = TestContext::new();

    // 1. Initial Apply
    ctx.setup_module("my-mod", "v1", "file v1 content");
    ctx.write_file("weaver.yaml", &weaver_config("my-mod", "v1", &ctx.root));

    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("apply")
        .assert()
        .success();

    // 2. User modifies file (Drift)
    ctx.write_file("app/file.txt", "user modified content");

    // 3. Apply Update (Default strategy: stop)
    // Note: Even without config change, apply should detect drift on managed file
    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("apply")
        .assert()
        .failure() // Should fail due to drift
        .stderr(predicate::str::contains("Drift detected"));

    // Verify content preserved
    assert_eq!(ctx.read_file("app/file.txt"), "user modified content");
}

#[test]
fn test_drift_overwrite() {
    let ctx = TestContext::new();

    // 1. Initial Apply
    ctx.setup_module("my-mod", "v1", "file v1 content");
    ctx.write_file("weaver.yaml", &weaver_config("my-mod", "v1", &ctx.root));

    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("apply")
        .assert()
        .success();

    // 2. User modifies file (Drift)
    ctx.write_file("app/file.txt", "user modified content");

    // 3. Apply Update with overwrite strategy
    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .args(&["apply", "--strategy", "overwrite", "--auto-approve"])
        .assert()
        .success();

    // Verify content overwritten
    assert_eq!(ctx.read_file("app/file.txt"), "file v1 content");
}
