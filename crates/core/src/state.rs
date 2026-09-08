use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const CURRENT_STATE_VERSION: u32 = 1;

fn default_state_version() -> u32 {
    CURRENT_STATE_VERSION
}

fn default_managed() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    #[serde(default = "default_state_version")]
    pub version: u32,
    #[serde(default)]
    pub files: HashMap<PathBuf, FileState>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: CURRENT_STATE_VERSION,
            files: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnedRegion {
    /// Region identifier (block-marker id, or `heading:<path>`).
    pub id: String,
    /// SHA-256 of the rendered content weaver last wrote into this region.
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub checksum: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default = "default_managed")]
    pub managed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_regions: Vec<OwnedRegion>,
}

impl FileState {
    pub fn new(checksum: String) -> Self {
        Self {
            checksum,
            source: None,
            managed: true,
            last_updated: None,
            owned_regions: Vec::new(),
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_last_updated(mut self, ts: impl Into<String>) -> Self {
        self.last_updated = Some(ts.into());
        self
    }

    pub fn with_owned_regions(mut self, regions: Vec<OwnedRegion>) -> Self {
        self.owned_regions = regions;
        self
    }
}

impl State {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let state = serde_yml::from_str(&content)?;
            Ok(state)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_yml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

pub fn calculate_checksum(path: &Path) -> anyhow::Result<String> {
    let content = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(hex_encode(&hasher.finalize()))
}

pub fn calculate_checksum_from_bytes(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_missing_returns_default() {
        let dir = TempDir::new().unwrap();
        let state = State::load(&dir.path().join("state.yaml")).unwrap();
        assert_eq!(state.version, CURRENT_STATE_VERSION);
        assert!(state.files.is_empty());
    }

    #[test]
    fn load_with_source_and_managed_fields() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("state.yaml");
        fs::write(
            &p,
            r#"version: 1
files:
  app/deployment.yaml:
    checksum: "abc123"
    source: "k8s-base:files/deployment.yaml"
    managed: true
"#,
        )
        .unwrap();

        let state = State::load(&p).unwrap();
        assert_eq!(state.version, 1);
        let fs_entry = state
            .files
            .get(&PathBuf::from("app/deployment.yaml"))
            .expect("entry present");
        assert_eq!(fs_entry.checksum, "abc123");
        assert_eq!(
            fs_entry.source.as_deref(),
            Some("k8s-base:files/deployment.yaml")
        );
        assert!(fs_entry.managed);
        assert!(fs_entry.last_updated.is_none());
    }

    #[test]
    fn load_legacy_last_updated_still_parses() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("state.yaml");
        fs::write(
            &p,
            r#"files:
  out.txt:
    checksum: "x"
    last_updated: "2024-01-01T00:00:00Z"
"#,
        )
        .unwrap();

        let state = State::load(&p).unwrap();
        let entry = state.files.get(&PathBuf::from("out.txt")).unwrap();
        assert_eq!(entry.last_updated.as_deref(), Some("2024-01-01T00:00:00Z"));
        assert!(entry.managed); // default
    }

    #[test]
    fn round_trip_save_and_load() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("state.yaml");

        let mut s = State::default();
        s.files.insert(
            PathBuf::from("a.txt"),
            FileState::new("checksum-a".into()).with_source("mod:files/a.txt"),
        );
        s.save(&p).unwrap();

        let loaded = State::load(&p).unwrap();
        assert_eq!(loaded.version, CURRENT_STATE_VERSION);
        let entry = loaded.files.get(&PathBuf::from("a.txt")).unwrap();
        assert_eq!(entry.checksum, "checksum-a");
        assert_eq!(entry.source.as_deref(), Some("mod:files/a.txt"));
    }

    #[test]
    fn save_omits_none_fields() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("state.yaml");
        let mut s = State::default();
        s.files
            .insert(PathBuf::from("a.txt"), FileState::new("c".into()));
        s.save(&p).unwrap();

        let raw = fs::read_to_string(&p).unwrap();
        assert!(!raw.contains("last_updated"), "yaml was: {raw}");
        assert!(!raw.contains("source:"), "yaml was: {raw}");
    }

    #[test]
    fn checksum_changes_when_content_changes() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("f");
        fs::write(&p, b"hello").unwrap();
        let h1 = calculate_checksum(&p).unwrap();
        fs::write(&p, b"world").unwrap();
        let h2 = calculate_checksum(&p).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn filestate_defaults_to_no_owned_regions_and_roundtrips() {
        let fs = FileState::new("abc".into());
        assert!(fs.owned_regions.is_empty());

        let region = OwnedRegion {
            id: "recent-changes".into(),
            checksum: "deadbeef".into(),
        };
        let fs2 = FileState::new("abc".into()).with_owned_regions(vec![region.clone()]);
        let yaml = serde_yml::to_string(&fs2).unwrap();
        let back: FileState = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(back.owned_regions.len(), 1);
        assert_eq!(back.owned_regions[0].id, "recent-changes");
    }
}
