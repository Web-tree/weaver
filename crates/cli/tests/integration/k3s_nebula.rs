use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir; // Added for fs_extra::dir::copy

fn setup_module_repo(fixture_path: &Path) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Copy fixture files to temp repo root
    let status = std::process::Command::new("cp")
        .arg("-r")
        .arg(fixture_path.join(".").to_str().unwrap()) // Copy contents
        .arg(root)
        .status()?;

    if !status.success() {
        return Err(Box::from("Failed to copy fixture files"));
    }

    // Initialize git repo
    std::process::Command::new("git")
        .current_dir(root)
        .arg("init")
        .output()?;

    std::process::Command::new("git")
        .current_dir(root)
        .args(&["config", "user.email", "test@example.com"])
        .output()?;

    std::process::Command::new("git")
        .current_dir(root)
        .args(&["config", "user.name", "Test User"])
        .output()?;

    std::process::Command::new("git")
        .current_dir(root)
        .args(&["add", "."])
        .output()?;

    std::process::Command::new("git")
        .current_dir(root)
        .args(&["commit", "-m", "Initial commit"])
        .output()?;

    // Tag it so we can target a specific ref if needed, or just use branch name
    std::process::Command::new("git")
        .current_dir(root)
        .args(&["tag", "v1.0.0"])
        .output()?;

    Ok(temp_dir)
}

#[test]
fn test_k3s_nebula_bootstrap() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup
    let workspace_dir = TempDir::new()?;
    let root = workspace_dir.path();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_path = Path::new(manifest_dir).join("../../tests/fixtures/modules/k3s-nebula");

    // Create the "remote" module repo
    let module_repo = setup_module_repo(&fixture_path)?;
    let module_source = module_repo.path().to_str().unwrap();

    // Create weaver.yaml
    let weaver_yaml = format!(
        r#"
version: "1.0"
modules:
  - name: k3s-nebula
    source: "{}"
    ref: "v1.0.0" 
    path: ""

apps:
  - name: my-cluster
    module: k3s-nebula
    path: apps/my-cluster
    inputs:
      cluster_name: "prod-cluster"
      region: "eu-central-1"
"#,
        module_source
    );

    fs::write(root.join("weaver.yaml"), weaver_yaml)?;

    // 2. Execution (wvr apply)
    // We expect this to execute successfully and create files
    let mut cmd = Command::cargo_bin("wvr")?;
    cmd.current_dir(root)
        .arg("apply")
        .arg("--auto-approve")
        .assert()
        .success()
        .stdout(predicate::str::contains("Apply complete"));

    // 3. Verification
    let app_dir = root.join("apps/my-cluster");

    // Check Taskfile exists and contains resolved inputs
    let taskfile = fs::read_to_string(app_dir.join("Taskfile.yml"))?;
    assert!(taskfile.contains("Installing k3s cluster prod-cluster in eu-central-1"));
    assert!(taskfile.contains("Nodes: 3")); // Default value

    // Check terraform/main.tf exists
    assert!(app_dir.join("terraform/main.tf").exists());

    Ok(())
}
