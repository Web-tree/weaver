use crate::config::ModuleConfig;
use crate::lockfile::{Lockfile, ModuleLock};
use std::path::{Component, Path, PathBuf};
use weaver_ops::git;

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
        Ok(Self {
            cache_dir,
            lockfile,
        })
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
            let abs = std::path::Path::new(source).canonicalize().map_err(|e| {
                anyhow::anyhow!("Cannot resolve local module path '{}': {}", source, e)
            })?;
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
                path: None,
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
    pub fn resolve_from_cache(
        &self,
        name: &str,
        source: &str,
        ref_: &str,
    ) -> Option<(PathBuf, String)> {
        let lock = self.lockfile.modules.get(name)?;
        if lock.source != source || lock.r#ref != ref_ || lock.resolved_commit.is_empty() {
            return None;
        }
        let folder_name = urlencoding::encode(&lock.source);
        let path = self
            .cache_dir
            .join(folder_name.as_ref())
            .join(&lock.resolved_commit);
        if path.exists() {
            Some((path, lock.resolved_commit.clone()))
        } else {
            None
        }
    }

    /// Like [`Self::resolve`], but also returns the commit the ref resolved
    /// to (reading it back out of the lock entry `resolve` just recorded),
    /// for callers that need to attribute results to a specific pinned
    /// commit (R2) without a second round trip through the lockfile.
    pub fn resolve_with_commit(
        &mut self,
        name: &str,
        source: &str,
        ref_: &str,
    ) -> anyhow::Result<(PathBuf, String)> {
        let path = self.resolve(name, source, ref_)?;
        let commit = self
            .lockfile
            .modules
            .get(name)
            .map(|m| m.resolved_commit.clone())
            .unwrap_or_default();
        Ok((path, commit))
    }

    /// [`Self::resolve`] for a declared module, returning its directory
    /// (the checkout, or `path` below it) and recording `path` in the lock.
    pub fn resolve_module(&mut self, module: &ModuleConfig) -> anyhow::Result<PathBuf> {
        Ok(self.resolve_module_with_commit(module)?.0)
    }

    /// [`Self::resolve_with_commit`] for a declared module.
    pub fn resolve_module_with_commit(
        &mut self,
        module: &ModuleConfig,
    ) -> anyhow::Result<(PathBuf, String)> {
        let (checkout, commit) =
            self.resolve_with_commit(&module.name, &module.source, &module.r#ref)?;
        let dir = module_dir(checkout, module.path.as_deref())?;
        if let Some(lock) = self.lockfile.modules.get_mut(&module.name) {
            lock.path = module.path.clone().filter(|p| !p.is_empty());
        }
        Ok((dir, commit))
    }

    /// [`Self::resolve_from_cache`] for a declared module. `None` on a cache
    /// miss; `Some(Err)` when the cached checkout lacks a valid `path`.
    pub fn resolve_module_from_cache(
        &self,
        module: &ModuleConfig,
    ) -> Option<anyhow::Result<(PathBuf, String)>> {
        let (checkout, commit) =
            self.resolve_from_cache(&module.name, &module.source, &module.r#ref)?;
        Some(module_dir(checkout, module.path.as_deref()).map(|dir| (dir, commit)))
    }
}

/// The directory holding a module's `weaver.module.yaml`: the checkout root,
/// or `subpath` below it, so one repo can ship modules beside other content.
/// `subpath` must be relative and stay inside the checkout, and must hold a
/// module -- a wrong directory would otherwise load as an empty module whose
/// checks silently never run.
pub fn module_dir(checkout: PathBuf, subpath: Option<&str>) -> anyhow::Result<PathBuf> {
    let Some(subpath) = subpath.filter(|p| !p.is_empty()) else {
        return Ok(checkout);
    };
    let rel = Path::new(subpath);
    if !rel
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        anyhow::bail!(
            "Module path '{subpath}' must be a relative path inside the module repository; {}",
            modules_found(&checkout)
        );
    }
    let dir = checkout.join(rel);
    if !dir.is_dir() {
        anyhow::bail!(
            "Module path '{subpath}' is not a directory in the module repository; {}",
            modules_found(&checkout)
        );
    }
    // The lexical check above cannot see a committed symlink, which would let
    // the pinned commit vouch for files elsewhere on the host.
    if !dir.canonicalize()?.starts_with(checkout.canonicalize()?) {
        anyhow::bail!(
            "Module path '{subpath}' resolves outside the module repository; {}",
            modules_found(&checkout)
        );
    }
    if !is_module_dir(&dir) {
        anyhow::bail!(
            "Module path '{subpath}' has no weaver.module.yaml, files/ or templates/; {}",
            modules_found(&checkout)
        );
    }
    Ok(dir)
}

fn is_module_dir(dir: &Path) -> bool {
    dir.join("weaver.module.yaml").is_file()
        || dir.join("files").is_dir()
        || dir.join("templates").is_dir()
}

/// Lists the subdirectories of `checkout` (up to 4 levels deep, skipping
/// `.git`) that hold a module, for an error that points at the right `path`.
fn modules_found(checkout: &Path) -> String {
    fn walk(root: &Path, dir: &Path, depth: usize, found: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || entry.file_name() == ".git" {
                continue;
            }
            if is_module_dir(&path) {
                found.push(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            } else if depth < 4 {
                walk(root, &path, depth + 1, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(checkout, checkout, 1, &mut found);
    found.sort();
    if found.is_empty() {
        "no modules found below the repository root".to_string()
    } else {
        format!("modules found: {}", found.join(", "))
    }
}

#[cfg(test)]
mod module_dir_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn no_path_is_the_checkout_root() {
        let tmp = tempdir().unwrap();
        assert_eq!(
            module_dir(tmp.path().to_path_buf(), None).unwrap(),
            tmp.path()
        );
        assert_eq!(
            module_dir(tmp.path().to_path_buf(), Some("")).unwrap(),
            tmp.path()
        );
    }

    #[test]
    fn a_relative_path_selects_the_subdirectory() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("weaver/web")).unwrap();
        std::fs::write(
            tmp.path().join("weaver/web/weaver.module.yaml"),
            "inputs: {}\n",
        )
        .unwrap();
        assert_eq!(
            module_dir(tmp.path().to_path_buf(), Some("weaver/web")).unwrap(),
            tmp.path().join("weaver/web")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_checkout_is_rejected() {
        let tmp = tempdir().unwrap();
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("weaver.module.yaml"), "inputs: {}\n").unwrap();
        let checkout = tmp.path().join("checkout");
        std::fs::create_dir_all(checkout.join("weaver")).unwrap();
        std::os::unix::fs::symlink(&outside, checkout.join("weaver/web")).unwrap();

        let err = module_dir(checkout, Some("weaver/web"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("outside the module repository"), "{err}");
    }

    #[test]
    fn parent_and_absolute_paths_are_rejected() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("a")).unwrap();
        for bad in ["../a", "a/../../x", "/etc"] {
            let err = module_dir(tmp.path().to_path_buf(), Some(bad))
                .unwrap_err()
                .to_string();
            assert!(err.contains(bad), "{bad}: {err}");
        }
    }

    #[test]
    fn a_missing_directory_is_an_error() {
        let tmp = tempdir().unwrap();
        let err = module_dir(tmp.path().to_path_buf(), Some("nope"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("nope"), "{err}");
    }

    #[test]
    fn the_error_names_the_modules_the_repo_does_ship() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        for manifest in [
            "weaver.module.yaml",
            "weaver/stdmod/weaver.module.yaml",
            "modules/a/weaver.module.yaml",
            ".git/hooks/weaver.module.yaml",
            "d1/d2/d3/d4/d5/weaver.module.yaml",
        ] {
            std::fs::create_dir_all(root.join(manifest).parent().unwrap()).unwrap();
            std::fs::write(root.join(manifest), "inputs: {}\n").unwrap();
        }
        std::fs::create_dir_all(root.join("modules/b/files")).unwrap();

        let err = module_dir(root.to_path_buf(), Some("weaver/nope"))
            .unwrap_err()
            .to_string();

        assert!(err.contains("weaver/nope"), "{err}");
        assert!(
            err.contains("modules found: modules/a, modules/b, weaver/stdmod"),
            "{err}"
        );
        assert!(!err.contains(".git") && !err.contains("d5"), "{err}");
    }

    #[test]
    fn a_directory_that_is_not_a_module_is_an_error() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("modules/a/templates")).unwrap();

        let err = module_dir(tmp.path().to_path_buf(), Some("modules"))
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("'modules'") && err.contains("modules found: modules/a"),
            "{err}"
        );
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
        struct Guard<'a> {
            _lock: std::sync::MutexGuard<'a, ()>,
        }
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                unsafe { std::env::remove_var(crate::paths::HOME_ENV) };
            }
        }
        let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var(crate::paths::HOME_ENV, dir) };
        Guard { _lock: guard }
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
            Command::new("git")
                .args(&args)
                .current_dir(dir)
                .output()
                .unwrap();
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

    #[test]
    fn resolve_module_records_the_path_in_the_lock() {
        let tmp = tempdir().unwrap();
        let _home = scoped_wvr_home(tmp.path());
        let src = tmp.path().join("src");
        std::fs::create_dir_all(src.join("modules/a")).unwrap();
        std::fs::write(src.join("modules/a/weaver.module.yaml"), "inputs: {}\n").unwrap();
        let url = make_repo(&src);

        let mut resolver = ModuleResolver::new(None).unwrap();
        let module = ModuleConfig {
            name: "modname".to_string(),
            source: url,
            r#ref: "v1".to_string(),
            path: Some("modules/a".to_string()),
        };
        let dir = resolver.resolve_module(&module).unwrap();

        assert!(dir.ends_with("modules/a"));
        let lock = resolver.take_lock();
        assert_eq!(lock.modules["modname"].path.as_deref(), Some("modules/a"));
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
        assert_eq!(
            fresh.resolve_from_cache("modname", &url, "v1"),
            Some((path, commit))
        );
    }
}
