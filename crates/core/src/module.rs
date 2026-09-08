use crate::lockfile::{Lockfile, ModuleLock};
use weaver_ops::git;
use std::path::PathBuf;

pub struct ModuleResolver {
    cache_dir: PathBuf,
    lockfile: Lockfile,
}

impl ModuleResolver {
    pub fn new(existing: Option<Lockfile>) -> anyhow::Result<Self> {
        let home =
            home::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let cache_dir = home.join(".rw").join("store");
        let mut lockfile = existing.unwrap_or_default();
        if lockfile.version.is_empty() {
            lockfile.version = "1".to_string();
        }
        Ok(Self { cache_dir, lockfile })
    }

    /// Resolve `source@ref` to a local path, pinning the ref to a concrete
    /// commit. The cache is keyed by the resolved commit so a moving branch
    /// re-resolves correctly. Records a `ModuleLock` keyed by `name`.
    pub fn resolve(&mut self, name: &str, source: &str, ref_: &str) -> anyhow::Result<PathBuf> {
        // Normalize bare local paths to absolute paths for git transport.
        // A "local path" is one that has no URL scheme (no "://") and exists on disk.
        // We canonicalize relative to CWD so that git ls-remote / clone work
        // regardless of the working directory of the calling process.
        let transport_url: String = if !source.contains("://") {
            let abs = std::path::Path::new(source)
                .canonicalize()
                .map_err(|e| anyhow::anyhow!("Cannot resolve local module path '{}': {}", source, e))?;
            abs.to_string_lossy().into_owned()
        } else {
            source.to_string()
        };

        let commit = git::rev_parse_remote(&transport_url, ref_)?;

        // Cache dir is keyed by the ORIGINAL `source` string (not the
        // normalized transport URL) — intentional. Different spellings of the
        // same local path (e.g. `../module` vs an absolute path) may cache
        // separately, which keeps the key stable to what the user wrote.
        let folder_name = urlencoding::encode(source);
        let path = self.cache_dir.join(folder_name.as_ref()).join(&commit);

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
            if let Err(e) = git::clone(&transport_url, &commit, &path) {
                std::fs::remove_dir_all(&path).ok();
                return Err(e);
            }
        }

        self.lockfile.modules.insert(
            name.to_string(),
            ModuleLock {
                source: source.to_string(),
                r#ref: ref_.to_string(),
                resolved_commit: commit,
                checksum: String::new(),
            },
        );

        Ok(path)
    }

    /// The accumulated lockfile (call after resolving all modules).
    pub fn take_lock(&self) -> Lockfile {
        self.lockfile.clone()
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    fn make_repo(dir: &std::path::Path) -> String {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("file.txt"), "hello").unwrap();
        for args in [
            vec!["init"],
            vec!["config", "user.email", "t@t.local"],
            vec!["config", "user.name", "T"],
            vec!["add", "."],
            vec!["commit", "-m", "init"],
            vec!["tag", "v1"],
        ] {
            Command::new("git").args(&args).current_dir(dir).output().unwrap();
        }
        format!("file://{}", dir.display())
    }

    #[test]
    fn resolve_caches_by_commit_and_records_lock() {
        let tmp = tempdir().unwrap();
        unsafe { std::env::set_var("HOME", tmp.path()) };
        let url = make_repo(&tmp.path().join("src"));

        let mut resolver = ModuleResolver::new(None).unwrap();
        let path = resolver.resolve("modname", &url, "v1").unwrap();

        assert!(path.join("file.txt").exists());
        let lock = resolver.take_lock();
        let entry = lock.modules.get("modname").expect("module lock recorded");
        assert_eq!(entry.resolved_commit.len(), 40);
        assert!(path.to_string_lossy().contains(&entry.resolved_commit));
    }
}
