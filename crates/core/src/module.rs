use crate::lockfile::{Lockfile, ModuleLock};
use weaver_ops::git;
use std::path::PathBuf;

pub struct ModuleResolver {
    cache_dir: PathBuf,
    lockfile: Lockfile,
}

impl ModuleResolver {
    pub fn new(existing: Option<Lockfile>) -> anyhow::Result<Self> {
        let cache_dir = crate::paths::store_dir();
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

    /// Resolve `name` using only the lockfile's pin and the already-warm
    /// module cache -- no `git` network operation (R8). Returns `None` when
    /// there is no matching lock entry, the entry's `source`/`ref` no longer
    /// match what's being asked for (stale pin -- re-resolution needed), or
    /// the cache directory for the pinned commit isn't present on disk.
    pub fn resolve_from_cache(&self, name: &str, source: &str, ref_: &str) -> Option<(PathBuf, String)> {
        let lock = self.lockfile.modules.get(name)?;
        if lock.source != source || lock.r#ref != ref_ || lock.resolved_commit.is_empty() {
            return None;
        }
        let folder_name = urlencoding::encode(&lock.source);
        let path = self.cache_dir.join(folder_name.as_ref()).join(&lock.resolved_commit);
        if path.exists() { Some((path, lock.resolved_commit.clone())) } else { None }
    }

    /// Like [`Self::resolve`], but also returns the commit the ref resolved
    /// to (reading it back out of the lock entry `resolve` just recorded),
    /// for callers that need to attribute results to a specific pinned
    /// commit (R2) without a second round trip through the lockfile.
    pub fn resolve_with_commit(&mut self, name: &str, source: &str, ref_: &str) -> anyhow::Result<(PathBuf, String)> {
        let path = self.resolve(name, source, ref_)?;
        let commit = self
            .lockfile
            .modules
            .get(name)
            .map(|m| m.resolved_commit.clone())
            .unwrap_or_default();
        Ok((path, commit))
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::*;
    use std::process::Command;
    use std::sync::Mutex;
    use tempfile::tempdir;

    /// `WVR_HOME` (read by `paths::store_dir`) is process-global, and cargo
    /// runs tests in this file in parallel by default -- serialize every
    /// test that sets it so one test's cache root can't leak into another's
    /// assertions.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Point `WVR_HOME` at `dir` for the life of the returned guard. Holds
    /// `ENV_LOCK` until dropped.
    fn scoped_wvr_home(dir: &std::path::Path) -> impl Drop {
        struct Guard<'a>(std::sync::MutexGuard<'a, ()>);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                unsafe { std::env::remove_var(crate::paths::HOME_ENV) };
            }
        }
        let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var(crate::paths::HOME_ENV, dir) };
        Guard(guard)
    }

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
        let _home = scoped_wvr_home(tmp.path());
        let url = make_repo(&tmp.path().join("src"));

        let mut resolver = ModuleResolver::new(None).unwrap();
        let path = resolver.resolve("modname", &url, "v1").unwrap();

        assert!(path.join("file.txt").exists());
        let lock = resolver.take_lock();
        let entry = lock.modules.get("modname").expect("module lock recorded");
        assert_eq!(entry.resolved_commit.len(), 40);
        assert!(path.to_string_lossy().contains(&entry.resolved_commit));
    }

    #[test]
    fn resolve_with_commit_returns_the_pinned_sha() {
        let tmp = tempdir().unwrap();
        let _home = scoped_wvr_home(tmp.path());
        let url = make_repo(&tmp.path().join("src"));

        let mut resolver = ModuleResolver::new(None).unwrap();
        let (path, commit) = resolver.resolve_with_commit("modname", &url, "v1").unwrap();

        assert_eq!(commit.len(), 40);
        assert!(path.to_string_lossy().contains(&commit));
    }

    #[test]
    fn resolve_from_cache_hits_after_a_prior_resolve_with_a_matching_pin() {
        let tmp = tempdir().unwrap();
        let _home = scoped_wvr_home(tmp.path());
        let url = make_repo(&tmp.path().join("src"));

        let mut resolver = ModuleResolver::new(None).unwrap();
        let (path, commit) = resolver.resolve_with_commit("modname", &url, "v1").unwrap();

        // Same resolver instance (carries the lock it just wrote) -- a cache
        // hit must require no further git operation.
        let cached = resolver.resolve_from_cache("modname", &url, "v1");
        assert_eq!(cached, Some((path, commit)));
    }

    #[test]
    fn resolve_from_cache_misses_without_a_prior_lock() {
        let tmp = tempdir().unwrap();
        let _home = scoped_wvr_home(tmp.path());
        let url = make_repo(&tmp.path().join("src"));

        let resolver = ModuleResolver::new(None).unwrap();
        assert_eq!(resolver.resolve_from_cache("modname", &url, "v1"), None);
    }

    #[test]
    fn resolve_from_cache_misses_when_the_pinned_ref_differs() {
        let tmp = tempdir().unwrap();
        let _home = scoped_wvr_home(tmp.path());
        let url = make_repo(&tmp.path().join("src"));

        let mut resolver = ModuleResolver::new(None).unwrap();
        resolver.resolve_with_commit("modname", &url, "v1").unwrap();

        // The lock was pinned against `v1`; asking for `v2` is a stale pin,
        // not a cache hit, even though the module name matches.
        assert_eq!(resolver.resolve_from_cache("modname", &url, "v2"), None);
    }

    /// Loading a resolver from a persisted lockfile (the `wvr check` case:
    /// no `resolve()` call happened in this process at all) still finds the
    /// cache -- this is the shape that matters for R8.
    #[test]
    fn resolve_from_cache_works_from_a_freshly_loaded_lockfile() {
        let tmp = tempdir().unwrap();
        let _home = scoped_wvr_home(tmp.path());
        let url = make_repo(&tmp.path().join("src"));

        let mut resolver = ModuleResolver::new(None).unwrap();
        let (path, commit) = resolver.resolve_with_commit("modname", &url, "v1").unwrap();
        let lock = resolver.take_lock();

        // Fresh resolver, as `wvr check` constructs one from the on-disk
        // weaver.lock -- no in-process resolve() call precedes this.
        let fresh = ModuleResolver::new(Some(lock)).unwrap();
        assert_eq!(fresh.resolve_from_cache("modname", &url, "v1"), Some((path, commit)));
    }
}
