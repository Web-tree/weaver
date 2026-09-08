use crate::ensure::{Ensure, EnsureContext, EnsurePlan};
use anyhow::Result;
use weaver_ops::git;
use std::path::PathBuf;

pub struct EnsureGitSubmodule {
    pub url: String,
    pub path: PathBuf,
    pub ref_: String,
}

impl Ensure for EnsureGitSubmodule {
    fn plan(&self, _ctx: &EnsureContext) -> Result<EnsurePlan> {
        Ok(EnsurePlan {
            description: format!(
                "Ensure git submodule {} at {:?} is {}",
                self.url, self.path, self.ref_
            ),
            actions: vec![],
        })
    }

    fn execute(&self, ctx: &EnsureContext) -> Result<()> {
        let target_path = ctx.app_path.join(&self.path);

        // Safe Checkout check
        if target_path.exists() {
            // If it's a repo, check if dirty
            if target_path.join(".git").exists() {
                if !git::is_worktree_clean(&target_path)? {
                    return Err(anyhow::anyhow!(
                        "Target path {:?} has dirty working tree. Safe Checkout aborted.",
                        target_path
                    ));
                }
            }
        }

        if ctx.dry_run {
            return Ok(());
        }

        if git::is_submodule_registered(&target_path)? {
            if let Err(e) = git::submodule_sync_init_update(&target_path, &self.ref_) {
                tracing::warn!(
                    "Failed to update submodule at {:?}: {}. Using cached version.",
                    target_path,
                    e
                );
            }
        } else {
            git::submodule_add(&self.url, &target_path, &self.ref_)?
        }
        Ok(())
    }
}

pub struct EnsureGitClonePinned {
    pub url: String,
    pub path: PathBuf,
    pub ref_: String,
}

impl Ensure for EnsureGitClonePinned {
    fn plan(&self, _ctx: &EnsureContext) -> Result<EnsurePlan> {
        Ok(EnsurePlan {
            description: format!(
                "Ensure git clone {} at {:?} pinned to {}",
                self.url, self.path, self.ref_
            ),
            actions: vec![],
        })
    }

    fn execute(&self, ctx: &EnsureContext) -> Result<()> {
        let target_path = ctx.app_path.join(&self.path);

        if target_path.exists() {
            // Check clean
            if target_path.join(".git").exists() {
                if !git::is_worktree_clean(&target_path)? {
                    return Err(anyhow::anyhow!(
                        "Target path {:?} has dirty working tree.",
                        target_path
                    ));
                }
            }
        }

        if ctx.dry_run {
            return Ok(());
        }

        if let Err(e) = git::clone_pinned(&self.url, &target_path, &self.ref_) {
            if target_path.exists() {
                tracing::warn!(
                    "Failed to update git clone at {:?}: {}. Using cached version.",
                    target_path,
                    e
                );
            } else {
                return Err(e);
            }
        }
        Ok(())
    }
}
