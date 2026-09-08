use crate::common::{TestContext, cmd};
use assert_cmd::Command;
use std::path::Path;

fn init_git_modules(root: &Path) {
    let modules_dir = root.join("modules");
    if modules_dir.exists() {
        for entry in std::fs::read_dir(modules_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                std::process::Command::new("git")
                    .arg("init")
                    .current_dir(&path)
                    .output()
                    .expect("Failed to git init module");

                // Ensure master branch
                std::process::Command::new("git")
                    .args(&["symbolic-ref", "HEAD", "refs/heads/master"])
                    .current_dir(&path)
                    .output()
                    .ok();

                std::process::Command::new("git")
                    .args(&["config", "user.email", "test@example.com"])
                    .current_dir(&path)
                    .output()
                    .ok();
                std::process::Command::new("git")
                    .args(&["config", "user.name", "Test"])
                    .current_dir(&path)
                    .output()
                    .ok();

                std::process::Command::new("git")
                    .args(&["add", "."])
                    .current_dir(&path)
                    .output()
                    .expect("Failed to git add");

                std::process::Command::new("git")
                    .args(&["commit", "-m", "Initial"])
                    .current_dir(&path)
                    .output()
                    .expect("Failed to git commit");
            }
        }
    }
}

#[test]
fn test_example_git_module() {
    let ctx = TestContext::new();
    let example_dir = ctx.temp.path().join("git-module");

    let src = Path::new("../../examples/git-module");
    std::process::Command::new("cp")
        .arg("-r")
        .arg(src)
        .arg(ctx.temp.path())
        .status()
        .expect("cp failed");

    // Init root git (workspace)
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&example_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(&example_dir)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(&["config", "user.name", "Test"])
        .current_dir(&example_dir)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(&["config", "protocol.file.allow", "always"])
        .current_dir(&example_dir)
        .output()
        .ok();

    // Explicitly set HEAD to master in root
    std::process::Command::new("git")
        .args(&["symbolic-ref", "HEAD", "refs/heads/master"])
        .current_dir(&example_dir)
        .output()
        .ok();

    std::process::Command::new("git")
        .args(&["commit", "--allow-empty", "-m", "Root init"])
        .current_dir(&example_dir)
        .output()
        .ok();

    // Init modules as git repos
    init_git_modules(&example_dir);

    // Mock upstream
    let upstream_path = ctx.temp.path().join("upstream-hello");
    std::fs::create_dir(&upstream_path).unwrap();
    std::process::Command::new("git")
        .arg("init")
        .arg("--bare")
        .current_dir(&upstream_path)
        .output()
        .unwrap();
    // Force upstream HEAD to master
    std::process::Command::new("git")
        .args(&["symbolic-ref", "HEAD", "refs/heads/master"])
        .current_dir(&upstream_path)
        .output()
        .ok();

    // Create upstream content
    let upstream_src = ctx.temp.path().join("upstream-src");
    std::fs::create_dir(&upstream_src).unwrap();
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&upstream_src)
        .output()
        .unwrap();
    // Force source branch to master
    std::process::Command::new("git")
        .args(&["symbolic-ref", "HEAD", "refs/heads/master"])
        .current_dir(&upstream_src)
        .output()
        .ok();

    std::fs::write(upstream_src.join("README.md"), "# Hello").unwrap();
    std::process::Command::new("git")
        .arg("add")
        .arg(".")
        .current_dir(&upstream_src)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(&upstream_src)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(&["config", "user.name", "Test"])
        .current_dir(&upstream_src)
        .output()
        .ok();
    std::process::Command::new("git")
        .args(&["commit", "-m", "Init"])
        .current_dir(&upstream_src)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(&["remote", "add", "origin", upstream_path.to_str().unwrap()])
        .current_dir(&upstream_src)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(&["push", "origin", "master"])
        .current_dir(&upstream_src)
        .output()
        .unwrap();

    // Modify modules/hello-module/weaver.module.yaml ON DISK to point to upstream
    let module_yaml_path = example_dir.join("modules/hello-module/weaver.module.yaml");
    let content = std::fs::read_to_string(&module_yaml_path).unwrap();
    let new_content = content.replace(
        "https://github.com/octocat/Hello-World.git",
        &format!("file://{}", upstream_path.to_str().unwrap()),
    );
    std::fs::write(module_yaml_path, new_content).unwrap();

    // Commit change to module
    let module_dir = example_dir.join("modules/hello-module");
    std::process::Command::new("git")
        .arg("add")
        .arg("weaver.module.yaml")
        .current_dir(&module_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(&["commit", "-m", "Mock URL"])
        .current_dir(&module_dir)
        .output()
        .unwrap();

    // Update weaver.yaml to point to master
    let weaver_yaml_path = example_dir.join("weaver.yaml");
    let weaver_content = std::fs::read_to_string(&weaver_yaml_path).unwrap();
    let new_weaver_content = weaver_content.replace("ref: \"HEAD\"", "ref: \"master\"");
    std::fs::write(weaver_yaml_path, new_weaver_content).unwrap();

    // Run wvr apply
    let mut cmd = cmd();
    let assert = cmd
        .arg("apply")
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("HOME", ctx.temp.path()) // Mock HOME
        .current_dir(&example_dir)
        .assert();

    assert.success();
}
