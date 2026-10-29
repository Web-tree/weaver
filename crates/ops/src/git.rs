use std::path::Path;
use std::process::Command;
use tracing::{info, warn};

/// Resolve a symbolic ref (branch/tag/sha) at a remote URL to a concrete
/// 40-char object SHA using `git ls-remote`. The result is a commit for
/// branches, lightweight tags, and SHAs; for annotated tags it is the tag
/// object SHA (clone+checkout still lands on the right commit). Used to pin
/// module refs so a moving branch is captured immutably in the lockfile/cache.
pub fn rev_parse_remote(url: &str, ref_: &str) -> anyhow::Result<String> {
    if ref_.len() == 40 && ref_.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(ref_.to_string());
    }
    let output = Command::new("git")
        .arg("ls-remote")
        .arg(url)
        .arg(ref_)
        .output()?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git ls-remote failed for {url}@{ref_}: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let sha = stdout
        .lines()
        .find_map(|line| line.split_whitespace().next())
        .ok_or_else(|| anyhow::anyhow!("ref '{ref_}' not found at {url}"))?;
    Ok(sha.to_string())
}

pub fn clone(url: &str, ref_: &str, dest: &Path) -> anyhow::Result<()> {
    info!("Cloning {} @ {} to {:?}", url, ref_, dest);

    // 1. Clone
    let status = Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(dest)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Git clone failed"));
    }

    // 2. Checkout ref
    let status = Command::new("git")
        .arg("checkout")
        .arg(ref_)
        .current_dir(dest)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Git checkout failed"));
    }

    Ok(())
}

// T034: clone_pinned (alias to clone for now or improved)
pub fn clone_pinned(url: &str, path: &Path, ref_: &str) -> anyhow::Result<()> {
    clone(url, ref_, path)
}

// T031: is_worktree_clean
pub fn is_worktree_clean(path: &Path) -> anyhow::Result<bool> {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(path)
        .output()?;

    Ok(output.stdout.is_empty())
}

// T032: submodule_add
// Assumption: CWD is the repo root, logic should handle it.
// However, ensure might pass absolute path?
// If `path` is absolute, we need to convert to relative for `git submodule add`?
// `git submodule add` expects path relative to CWD (root).
// But `path` arg might be absolute if ensures resolve path.
// Let's assume `path` is the target location for submodule.
pub fn submodule_add(url: &str, path: &Path, ref_: &str) -> anyhow::Result<()> {
    info!("Adding submodule {} @ {} at {:?}", url, ref_, path);
    // TODO: Handle if already exists?

    // We assume running from workspace root.
    // ensure `path` is relative?

    let status = Command::new("git")
        .arg("submodule")
        .arg("add")
        .arg("--force")
        .arg(url)
        .arg(path)
        .status()?;

    if !status.success() {
        // If it fails, maybe it already exists?
        // We should check beforehand.
        // For simple MVP implementation, we fail.
        return Err(anyhow::anyhow!("Git submodule add failed"));
    }

    // Update to correct ref
    submodule_update(path, ref_)
}

// T033: submodule_update
pub fn submodule_update(path: &Path, ref_: &str) -> anyhow::Result<()> {
    info!("Updating submodule at {:?} to {}", path, ref_);

    // Fetch logic needed if ref not present?
    // Run fetch in submodule
    let status = Command::new("git").arg("fetch").current_dir(path).status();

    if let Err(e) = status {
        warn!("Git fetch failed in submodule (might be offline): {}", e);
    }

    let status = Command::new("git")
        .arg("checkout")
        .arg(ref_)
        .current_dir(path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Git checkout failed in submodule"));
    }

    Ok(())
}

// Check if path is a registered submodule
pub fn is_submodule_registered(path: &Path) -> anyhow::Result<bool> {
    // git submodule status <path> returns 0 if known, 1 if unknown
    // It works even if directory is missing (returns with - prefix)
    let output = Command::new("git")
        .arg("submodule")
        .arg("status")
        .arg(path)
        .output()?;

    Ok(output.status.success())
}

// Robust sync/update/init
pub fn submodule_sync_init_update(path: &Path, ref_: &str) -> anyhow::Result<()> {
    info!("Syncing and updating submodule at {:?} to {}", path, ref_);

    // 1. Sync (updates URL from .gitmodules if needed)
    let _ = Command::new("git")
        .arg("submodule")
        .arg("sync")
        .arg(path)
        .status();

    // 2. Update --init (restores missing submodule dir)
    let status = Command::new("git")
        .arg("submodule")
        .arg("update")
        .arg("--init")
        .arg("--recursive")
        .arg(path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Git submodule update --init failed"));
    }

    // 3. Checkout ref
    let status = Command::new("git")
        .arg("checkout")
        .arg(ref_)
        .current_dir(path)
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Git checkout failed in submodule"));
    }

    Ok(())
}

#[cfg(test)]
mod rev_parse_tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    #[test]
    fn rev_parse_remote_resolves_a_tag_to_a_40_char_sha() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        for args in [
            vec!["init"],
            vec!["config", "user.email", "t@t.local"],
            vec!["config", "user.name", "T"],
            vec!["commit", "--allow-empty", "-m", "init"],
            vec!["tag", "v1"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&repo)
                .output()
                .unwrap();
        }
        let url = format!("file://{}", repo.display());
        let sha = rev_parse_remote(&url, "v1").unwrap();
        assert_eq!(sha.len(), 40);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
