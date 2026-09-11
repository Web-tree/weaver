// Global plugin cache management

use super::PluginError;
use std::fs;
use std::path::{Path, PathBuf};

pub struct PluginCache {
    root: PathBuf,
}

impl PluginCache {
    /// Create a new plugin cache at the specified root directory
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Get the default cache directory (`$WVR_HOME/plugins`, default `~/.rw/plugins`)
    pub fn default_root() -> PathBuf {
        crate::paths::plugins_dir()
    }

    /// Check if the cache directory is accessible (writable)
    /// This should be called on startup
    pub fn ensure_accessible(&self) -> Result<(), PluginError> {
        // Create the directory if it doesn't exist
        if let Err(e) = fs::create_dir_all(&self.root) {
            return Err(PluginError::CacheNotWritable {
                path: self.root.display().to_string(),
                source: e,
            });
        }

        // Test write access by creating a temporary file
        let test_file = self.root.join(".write_test");
        if let Err(e) = fs::write(&test_file, b"test") {
            return Err(PluginError::CacheNotWritable {
                path: self.root.display().to_string(),
                source: e,
            });
        }
        let _ = fs::remove_file(&test_file);

        Ok(())
    }

    /// Check if a plugin with the given name and version exists in cache
    pub fn has(&self, name: &str, version: &str) -> bool {
        let plugin_path = self.get_plugin_path(name, version);
        plugin_path.exists()
    }

    /// Get the path to a cached plugin
    pub fn get(&self, name: &str, version: &str) -> Option<PathBuf> {
        let plugin_path = self.get_plugin_path(name, version);
        if plugin_path.exists() {
            Some(plugin_path)
        } else {
            None
        }
    }

    /// Store a plugin in the cache
    pub fn store(
        &self,
        name: &str,
        version: &str,
        wasm_data: &[u8],
    ) -> Result<PathBuf, PluginError> {
        let dir = self.root.join(name).join(version);
        fs::create_dir_all(&dir).map_err(|e| PluginError::CacheNotWritable {
            path: dir.display().to_string(),
            source: e,
        })?;

        let plugin_path = dir.join("plugin.wasm");
        fs::write(&plugin_path, wasm_data).map_err(|e| PluginError::CacheNotWritable {
            path: plugin_path.display().to_string(),
            source: e,
        })?;

        Ok(plugin_path)
    }

    /// Create a symlink from project-local .rw/plugins/ to global cache
    /// Only works on Unix systems (macOS, Linux)
    #[cfg(unix)]
    pub fn link(
        &self,
        name: &str,
        version: &str,
        target_dir: &Path,
    ) -> Result<PathBuf, PluginError> {
        use std::os::unix::fs as unix_fs;

        let source = self.get_plugin_path(name, version);
        if !source.exists() {
            return Err(PluginError::Other(anyhow::anyhow!(
                "Plugin not found in cache: {}/{}",
                name,
                version
            )));
        }

        // Create target directory
        fs::create_dir_all(target_dir).map_err(|e| {
            PluginError::Other(anyhow::anyhow!("Failed to create target directory: {}", e))
        })?;

        let link_path = target_dir.join(format!("{}.wasm", name));

        // Remove existing symlink if it exists
        if link_path.exists() || link_path.symlink_metadata().is_ok() {
            fs::remove_file(&link_path).map_err(|e| {
                PluginError::Other(anyhow::anyhow!("Failed to remove existing symlink: {}", e))
            })?;
        }

        // Create symlink
        unix_fs::symlink(&source, &link_path)
            .map_err(|e| PluginError::Other(anyhow::anyhow!("Failed to create symlink: {}", e)))?;

        Ok(link_path)
    }

    /// On non-Unix systems, copying is not implemented yet
    #[cfg(not(unix))]
    pub fn link(
        &self,
        name: &str,
        version: &str,
        target_dir: &Path,
    ) -> Result<PathBuf, PluginError> {
        Err(PluginError::Other(anyhow::anyhow!(
            "Symlink creation is only supported on Unix systems (macOS, Linux). Use WSL on Windows."
        )))
    }

    /// Detect and remove broken symlinks in the target directory
    pub fn cleanup_broken_links(target_dir: &Path) -> Result<(), PluginError> {
        if !target_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(target_dir)
            .map_err(|e| PluginError::Other(anyhow::anyhow!("Failed to read directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                PluginError::Other(anyhow::anyhow!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();

            // Check if it's a symlink
            if let Ok(metadata) = path.symlink_metadata()
                && metadata.is_symlink()
            {
                // Check if the target exists
                if !path.exists() {
                    // Broken symlink - remove it
                    fs::remove_file(&path).map_err(|e| {
                        PluginError::Other(anyhow::anyhow!(
                            "Failed to remove broken symlink: {}",
                            e
                        ))
                    })?;
                }
            }
        }

        Ok(())
    }

    /// Get the full path to a plugin in the cache
    fn get_plugin_path(&self, name: &str, version: &str) -> PathBuf {
        self.root.join(name).join(version).join("plugin.wasm")
    }
}

impl Default for PluginCache {
    fn default() -> Self {
        Self::new(Self::default_root())
    }
}
