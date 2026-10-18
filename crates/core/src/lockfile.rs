use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Lockfile {
    pub version: String,
    pub modules: HashMap<String, ModuleLock>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginLock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleLock {
    pub source: String,
    pub r#ref: String,
    /// Concrete object SHA the symbolic `ref` resolved to (ref pinning).
    /// A commit for branches, lightweight tags, and SHAs; a tag object SHA
    /// for annotated tags (clone+checkout still lands on the right commit).
    #[serde(default)]
    pub resolved_commit: String,
    pub checksum: String,
    /// `modules[].path`: the module's subdirectory inside the checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLock {
    pub version: String,
    pub source: String,
    pub sha256: String,
    pub resolved_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_lock(path: Option<&str>) -> ModuleLock {
        ModuleLock {
            source: "https://example.com/std.git".to_string(),
            r#ref: "v1".to_string(),
            resolved_commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            checksum: String::new(),
            path: path.map(str::to_string),
        }
    }

    #[test]
    fn module_path_round_trips() {
        let mut lock = Lockfile { version: "1".to_string(), ..Default::default() };
        lock.modules.insert("std".to_string(), module_lock(Some("modules/a")));

        let yaml = serde_yml::to_string(&lock).unwrap();
        let back: Lockfile = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(back.modules["std"].path.as_deref(), Some("modules/a"));
    }

    #[test]
    fn module_without_path_omits_it_and_still_loads() {
        let mut lock = Lockfile { version: "1".to_string(), ..Default::default() };
        lock.modules.insert("std".to_string(), module_lock(None));
        assert!(!serde_yml::to_string(&lock).unwrap().contains("path"));

        let legacy = "version: '1'\nmodules:\n  std:\n    source: s\n    ref: v1\n    resolved_commit: abc\n    checksum: ''\n";
        let back: Lockfile = serde_yml::from_str(legacy).unwrap();
        assert_eq!(back.modules["std"].path, None);
    }
}
