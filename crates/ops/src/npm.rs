use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn npm_pkg_set_script(name: &str, command: &str, cwd: &Path) -> Result<()> {
    // "scripts.name"="command"
    let key = format!("scripts.{}", name);
    // npm pkg set expecting "key=value"
    // Handle quoting? npm pkg set handles it.

    let output = Command::new("npm")
        .arg("pkg")
        .arg("set")
        .arg(format!("{}={}", key, command))
        .current_dir(cwd)
        .output()
        .context("Failed to execute npm pkg set")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("npm pkg set failed: {}", stderr);
    }
    Ok(())
}

pub fn npm_pkg_get_script(name: &str, cwd: &Path) -> Result<Option<String>> {
    // We can read package.json directly or use npm pkg get.
    // npm pkg get scripts.name returns value or {} if empty?
    // Let's read package.json for speed/simplicity effectively.

    let pkg_path = cwd.join("package.json");
    if !pkg_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(pkg_path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(scripts) = json.get("scripts")
        && let Some(cmd) = scripts.get(name)
        && let Some(s) = cmd.as_str()
    {
        return Ok(Some(s.to_string()));
    }

    Ok(None)
}
