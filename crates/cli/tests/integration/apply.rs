use crate::common::{TestContext, cmd};

/// `apply` on a workspace with no modules or apps succeeds and writes state.
///
/// Runs in a tempdir: `apply` rewrites `.rw/state.yaml`, so pointing it at a
/// committed fixture dirtied the working tree on every test run.
#[test]
fn test_bootstrap_empty_workspace() {
    let ctx = TestContext::new();
    ctx.write_file("weaver.yaml", "version: \"1.0\"\nmodules: []\napps: []\n");

    cmd()
        .arg("apply")
        .current_dir(&ctx.root)
        .assert()
        .success();

    assert!(ctx.root.join(".rw/state.yaml").exists());
}
