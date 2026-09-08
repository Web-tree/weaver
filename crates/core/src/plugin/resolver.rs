// Plugin resolution and discovery logic

use super::{
    BuildMethod, PluginError, PluginMetadata, PluginSource, ResolvedPlugin, cache::PluginCache,
    fetcher::PluginFetcher,
};
use crate::config::PluginConfig;
use crate::lockfile::{Lockfile, PluginLock};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

pub struct PluginResolver {
    cache: PluginCache,
    project_dir: PathBuf,
    fetcher: PluginFetcher,
    offline: bool,
    // Track resolved plugins for lockfile generation
    resolved_plugins: RefCell<Vec<(String, ResolvedPlugin)>>,
}

impl PluginResolver {
    /// Create a new plugin resolver for the given project directory
    pub fn new(project_dir: PathBuf) -> Result<Self, PluginError> {
        let cache = PluginCache::default();
        cache.ensure_accessible()?;

        Ok(Self {
            cache,
            project_dir,
            fetcher: PluginFetcher::new(),
            offline: false,
            resolved_plugins: RefCell::new(Vec::new()),
        })
    }

    /// Set offline mode (error if plugin not cached)
    pub fn set_offline(&mut self, offline: bool) {
        self.offline = offline;
    }

    /// Resolve a plugin from explicit configuration
    pub async fn resolve(
        &self,
        name: &str,
        config: &PluginConfig,
    ) -> Result<ResolvedPlugin, PluginError> {
        // Determine the plugin source
        let source = if let Some(path) = &config.path {
            // Local path plugin
            PluginSource::Local {
                path: self.project_dir.join(path),
            }
        } else if let Some(git_url) = &config.git {
            // Git source plugin
            let git_ref = config.git_ref.clone().unwrap_or_else(|| "main".to_string());
            PluginSource::Git {
                url: git_url.clone(),
                git_ref,
            }
        } else {
            // Default to registry
            PluginSource::Registry {
                name: name.to_string(),
            }
        };

        self.resolve_from_source(name, source).await
    }

    /// Resolve a plugin from ensure type name (auto-discovery)
    /// Maps ensure type like "npm.script" to plugin name "npm-script"
    pub async fn resolve_ensure_type(
        &self,
        type_name: &str,
    ) -> Result<ResolvedPlugin, PluginError> {
        // Convert type name to plugin name
        // e.g., "npm.script" -> "npm-script"
        let plugin_name = type_name.replace('.', "-");

        // Use registry source for auto-discovered plugins
        let source = PluginSource::Registry {
            name: plugin_name.clone(),
        };

        self.resolve_from_source(&plugin_name, source).await
    }

    /// Resolve a plugin from a specific source
    async fn resolve_from_source(
        &self,
        name: &str,
        source: PluginSource,
    ) -> Result<ResolvedPlugin, PluginError> {
        match source {
            PluginSource::Local { path } => {
                // For local paths, validate existence and load directly
                let wasm_path = path.join("plugin.wasm");
                if !wasm_path.exists() {
                    return Err(PluginError::PathNotFound {
                        path: wasm_path.display().to_string(),
                        source: None,
                    });
                }

                let wasm_data = fs::read(&wasm_path).map_err(|e| PluginError::PathNotFound {
                    path: wasm_path.display().to_string(),
                    source: Some(e),
                })?;

                let sha256 = calculate_sha256(&wasm_data);

                let resolved = ResolvedPlugin {
                    name: name.to_string(),
                    version: "local".to_string(),
                    source: PluginSource::Local { path: path.clone() },
                    wasm_path,
                    metadata: PluginMetadata {
                        sha256,
                        resolved_at: chrono::Utc::now().to_rfc3339(),
                        source_url: format!("file://{}", path.display()),
                        build_method: BuildMethod::Local,
                    },
                };

                // Track resolved plugin for lockfile
                self.resolved_plugins
                    .borrow_mut()
                    .push((name.to_string(), resolved.clone()));

                Ok(resolved)
            }
            PluginSource::Git { url, git_ref } => {
                // For git sources, check cache first, then fetch
                let version = git_ref.clone();

                if let Some(cached_path) = self.cache.get(name, &version) {
                    let wasm_data =
                        fs::read(&cached_path).map_err(|e| PluginError::Other(e.into()))?;
                    let sha256 = calculate_sha256(&wasm_data);

                    let resolved = ResolvedPlugin {
                        name: name.to_string(),
                        version: version.clone(),
                        source: PluginSource::Git {
                            url: url.clone(),
                            git_ref: git_ref.clone(),
                        },
                        wasm_path: cached_path,
                        metadata: PluginMetadata {
                            sha256,
                            resolved_at: chrono::Utc::now().to_rfc3339(),
                            source_url: url.clone(),
                            build_method: BuildMethod::Prebuilt,
                        },
                    };

                    // Track resolved plugin for lockfile
                    self.resolved_plugins
                        .borrow_mut()
                        .push((name.to_string(), resolved.clone()));

                    return Ok(resolved);
                }

                // Not cached - check offline mode
                if self.offline {
                    return Err(PluginError::PluginNotCached {
                        name: name.to_string(),
                    });
                }

                // DEVELOPMENT MODE: Check if we're in the Weaver development directory
                // and if a local plugin exists in plugins/ directory
                if let Some(local_wasm) = self.try_load_dev_plugin(name)? {
                    let sha256 = calculate_sha256(&local_wasm);

                    // Store in cache for consistency
                    let cached_path = self.cache.store(name, &version, &local_wasm)?;

                    let resolved = ResolvedPlugin {
                        name: name.to_string(),
                        version,
                        source: PluginSource::Git {
                            url: url.clone(),
                            git_ref,
                        },
                        wasm_path: cached_path,
                        metadata: PluginMetadata {
                            sha256,
                            resolved_at: chrono::Utc::now().to_rfc3339(),
                            source_url: format!("local:plugins/{}", name),
                            build_method: BuildMethod::Local,
                        },
                    };

                    // Track resolved plugin for lockfile
                    self.resolved_plugins
                        .borrow_mut()
                        .push((name.to_string(), resolved.clone()));

                    return Ok(resolved);
                }

                // Fetch from git
                let wasm_data = self.fetcher.fetch_release(&url, &git_ref).await?;
                let sha256 = calculate_sha256(&wasm_data);

                // Store in cache
                let cached_path = self.cache.store(name, &version, &wasm_data)?;

                let resolved = ResolvedPlugin {
                    name: name.to_string(),
                    version,
                    source: PluginSource::Git {
                        url: url.clone(),
                        git_ref,
                    },
                    wasm_path: cached_path,
                    metadata: PluginMetadata {
                        sha256,
                        resolved_at: chrono::Utc::now().to_rfc3339(),
                        source_url: url,
                        build_method: BuildMethod::Prebuilt,
                    },
                };

                // Track resolved plugin for lockfile
                self.resolved_plugins
                    .borrow_mut()
                    .push((name.to_string(), resolved.clone()));

                Ok(resolved)
            }
            PluginSource::Registry { name: plugin_name } => {
                // For registry sources, resolve URL from environment/config/default
                let registry_url = self.get_registry_url();
                let download_url = format!(
                    "{}/plugins/{}/latest/plugin.wasm",
                    registry_url, plugin_name
                );

                // Use "latest" as version for now
                let version = "latest".to_string();

                // Check cache first
                if let Some(cached_path) = self.cache.get(&plugin_name, &version) {
                    let wasm_data =
                        fs::read(&cached_path).map_err(|e| PluginError::Other(e.into()))?;
                    let sha256 = calculate_sha256(&wasm_data);

                    let resolved = ResolvedPlugin {
                        name: plugin_name.clone(),
                        version: version.clone(),
                        source: PluginSource::Registry {
                            name: plugin_name.clone(),
                        },
                        wasm_path: cached_path,
                        metadata: PluginMetadata {
                            sha256,
                            resolved_at: chrono::Utc::now().to_rfc3339(),
                            source_url: download_url.clone(),
                            build_method: BuildMethod::Prebuilt,
                        },
                    };

                    // Track resolved plugin for lockfile
                    self.resolved_plugins
                        .borrow_mut()
                        .push((plugin_name.clone(), resolved.clone()));

                    return Ok(resolved);
                }

                // Not cached - check offline mode
                if self.offline {
                    return Err(PluginError::PluginNotCached {
                        name: plugin_name.clone(),
                    });
                }

                // Fetch from registry
                let wasm_data = self.fetcher.fetch_from_url(&download_url).await?;
                let sha256 = calculate_sha256(&wasm_data);

                // Store in cache
                let cached_path = self.cache.store(&plugin_name, &version, &wasm_data)?;

                let resolved = ResolvedPlugin {
                    name: plugin_name.clone(),
                    version,
                    source: PluginSource::Registry { name: plugin_name },
                    wasm_path: cached_path,
                    metadata: PluginMetadata {
                        sha256,
                        resolved_at: chrono::Utc::now().to_rfc3339(),
                        source_url: download_url,
                        build_method: BuildMethod::Prebuilt,
                    },
                };

                // Track resolved plugin for lockfile
                self.resolved_plugins
                    .borrow_mut()
                    .push((resolved.name.clone(), resolved.clone()));

                Ok(resolved)
            }
        }
    }

    /// Get all resolved plugins for lockfile generation
    pub fn get_resolved_plugins(&self) -> Vec<(String, ResolvedPlugin)> {
        self.resolved_plugins.borrow().clone()
    }

    /// Verify a plugin against lockfile checksum
    pub fn verify(&self, name: &str, lock: &PluginLock) -> Result<bool, PluginError> {
        // Get the cached plugin
        let cached_path =
            self.cache
                .get(name, &lock.version)
                .ok_or_else(|| PluginError::PluginNotCached {
                    name: name.to_string(),
                })?;

        // Calculate checksum
        let wasm_data = fs::read(&cached_path).map_err(|e| PluginError::Other(e.into()))?;
        let sha256 = calculate_sha256(&wasm_data);

        // Compare with lockfile
        if sha256 != lock.sha256 {
            return Err(PluginError::ChecksumMismatch {
                name: name.to_string(),
                expected: lock.sha256.clone(),
                actual: sha256,
            });
        }

        Ok(true)
    }

    /// Update lockfile with resolved plugins
    pub fn update_lockfile(
        &self,
        lockfile_path: &Path,
        plugins: &[(String, ResolvedPlugin)],
    ) -> Result<(), PluginError> {
        // Load existing lockfile or create new one
        let mut lockfile = if lockfile_path.exists() {
            let content =
                fs::read_to_string(lockfile_path).map_err(|e| PluginError::Other(e.into()))?;
            serde_yml::from_str::<Lockfile>(&content).map_err(|e| PluginError::Other(e.into()))?
        } else {
            Lockfile {
                version: "1".to_string(),
                ..Default::default()
            }
        };

        // Update plugin locks
        for (name, resolved) in plugins {
            lockfile.plugins.insert(
                name.clone(),
                PluginLock {
                    version: resolved.version.clone(),
                    source: format_source(&resolved.source),
                    sha256: resolved.metadata.sha256.clone(),
                    resolved_at: resolved.metadata.resolved_at.clone(),
                },
            );
        }

        // Write lockfile
        let content = serde_yml::to_string(&lockfile).map_err(|e| PluginError::Other(e.into()))?;
        fs::write(lockfile_path, content).map_err(|e| PluginError::Other(e.into()))?;

        Ok(())
    }

    /// Get the registry URL from environment, config, or default
    fn get_registry_url(&self) -> String {
        // Check environment variable first
        if let Ok(url) = std::env::var("RW_REGISTRY_URL") {
            return url;
        }

        // TODO: Check config file

        // Default registry
        "https://plugins.repo-weaver.dev".to_string()
    }

    /// Try to load a plugin from the local development plugins/ directory
    /// Returns Some(wasm_bytes) if found, None otherwise
    fn try_load_dev_plugin(&self, name: &str) -> Result<Option<Vec<u8>>, PluginError> {
        // Check if we're in a development environment by looking for plugins/ directory
        // relative to the project root
        let potential_paths = vec![
            // From project directory
            self.project_dir
                .join("plugins")
                .join(name)
                .join("plugin.wasm"),
            // Go up one level (in case we're in a subdirectory like examples/)
            self.project_dir
                .join("..")
                .join("plugins")
                .join(name)
                .join("plugin.wasm"),
            // Go up two levels
            self.project_dir
                .join("../..")
                .join("plugins")
                .join(name)
                .join("plugin.wasm"),
        ];

        for path in potential_paths {
            if let Ok(canonical) = path.canonicalize() {
                if canonical.exists() {
                    match fs::read(&canonical) {
                        Ok(data) => {
                            eprintln!(
                                "✓ Loaded plugin from local development: {}",
                                canonical.display()
                            );
                            return Ok(Some(data));
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        Ok(None)
    }
}

/// Calculate SHA256 checksum of data
pub fn calculate_sha256(data: &[u8]) -> String {
    crate::state::calculate_checksum_from_bytes(data)
}

/// Format plugin source for lockfile
fn format_source(source: &PluginSource) -> String {
    match source {
        PluginSource::Local { path } => format!("path:{}", path.display()),
        PluginSource::Git { url, git_ref } => format!("git:{}@{}", url, git_ref),
        PluginSource::Registry { name } => format!("registry:{}", name),
    }
}
