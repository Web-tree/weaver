use crate::common::{TestContext, cmd};

use predicates::prelude::*;

#[test]
fn test_config_includes() {
    let ctx = TestContext::new();

    // Setup a dummy module
    ctx.setup_module("dummy-mod", "v1", "dummy content");
    let remote_path = ctx.root.join("remotes").join("dummy-mod");
    let remote_url = format!("file://{}", remote_path.display());

    // Create weaver.yaml with includes
    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
includes:
  - "includes/*.yaml"
modules:
  - name: "dummy"
    source: "PLACEHOLDER_URL"
    ref: "v1"
"#
        .replace("PLACEHOLDER_URL", &remote_url)
        .as_str(),
    );

    // Create includes directory
    std::fs::create_dir_all(ctx.root.join("includes")).unwrap();

    // Create included fragments
    ctx.write_file(
        "includes/app1.yaml",
        r#"
apps:
  - name: "app1"
    module: "dummy"
    path: "app1"
    inputs: {}
"#,
    );

    ctx.write_file(
        "includes/app2.yaml",
        r#"
apps:
  - name: "app2"
    module: "dummy"
    path: "app2"
    inputs: {}
"#,
    );

    // Initialise answers (interactive prompts skip)
    // The test runner might hang if prompts appear. apply command has --auto-approve for actual apply.
    // For --dry-run (plan), it might still prompt if inputs are missing.
    // We provided empty inputs {} and module has empty inputs {}. So no prompt expected.
    // But we need .rw/state.yaml or it might complain?
    // apply.rs:44: let mut state = State::load(state_path)?;
    // State::load usually handles missing file by creating empty state?
    // Let's check State::load implementation if possible, or just ensure .weaver dir exists.
    std::fs::create_dir_all(ctx.root.join(".rw")).unwrap();
    ctx.write_file(".rw/state.yaml", "files: {}");
    ctx.write_file(".rw/answers.yaml", "{}");

    // Run wvr apply --dry-run (plan)
    let mut cmd = cmd();
    let assert = cmd.arg("plan").current_dir(&ctx.root).assert();

    assert
        .success()
        .stdout(predicate::str::contains("Processing app: app1"))
        .stdout(predicate::str::contains("Processing app: app2"));
}
