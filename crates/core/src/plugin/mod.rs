pub mod ensure_wasm;
pub mod wasm;

// Plugin management modules
pub mod cache;
pub mod fetcher;
pub mod resolver;

use std::path::PathBuf;

/// Plugin source type
#[derive(Debug, Clone)]
pub enum PluginSource {
    Local { path: PathBuf },
    Git { url: String, git_ref: String },
    Registry { name: String },
}

/// Build method for the plugin
#[derive(Debug, Clone)]
pub enum BuildMethod {
    Prebuilt,
    SourceBuild { container: String },
    Local,
}

/// Plugin metadata
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub sha256: String,
    pub resolved_at: String,
    pub source_url: String,
    pub build_method: BuildMethod,
}

/// Resolved plugin
#[derive(Debug, Clone)]
pub struct ResolvedPlugin {
    pub name: String,
    pub version: String,
    pub source: PluginSource,
    pub wasm_path: PathBuf,
    pub metadata: PluginMetadata,
}

/// Plugin resolution and management errors
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin path not found: {path}")]
    PathNotFound {
        path: String,
        #[source]
        source: Option<std::io::Error>,
    },

    #[error("Checksum mismatch for plugin '{name}': expected {expected}, got {actual}")]
    ChecksumMismatch {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("Plugin cache directory is not writable: {path}")]
    CacheNotWritable {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid WASM component: {path}")]
    InvalidWasm {
        path: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Plugin configuration error: {message}")]
    ConfigError { message: String },

    #[error("Failed to download plugin: {message}")]
    FetchError {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    #[error("Container runtime not found. Please install Docker or Podman.")]
    ContainerRuntimeNotFound,

    #[error("Plugin build failed: {message}")]
    BuildError {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    #[error("Plugin not found in cache. Run online first or check configuration.")]
    PluginNotCached { name: String },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl PluginError {
    /// Get remediation suggestion for the error
    pub fn remediation(&self) -> Option<&str> {
        match self {
            PluginError::PathNotFound { .. } => {
                Some("Check that the plugin path exists and is accessible")
            }
            PluginError::ChecksumMismatch { .. } => {
                Some("Run 'wvr plugins update' to update the lockfile")
            }
            PluginError::CacheNotWritable { .. } => {
                Some("Check permissions on the cache directory (~/.rw/plugins/)")
            }
            PluginError::InvalidWasm { .. } => Some("Ensure the WASM file is a valid component"),
            PluginError::ConfigError { .. } => Some("Fix the plugin configuration in weaver.yaml"),
            PluginError::FetchError { .. } => Some("Check network connection and try again"),
            PluginError::ContainerRuntimeNotFound => Some(
                "Install Docker (https://docs.docker.com/get-docker/) or Podman (https://podman.io/getting-started/installation)",
            ),
            PluginError::BuildError { .. } => {
                Some("Check build logs and ensure the plugin source is valid")
            }
            PluginError::PluginNotCached { .. } => {
                Some("Connect to the internet and run 'wvr apply' to download the plugin")
            }
            PluginError::Other(_) => None,
        }
    }
}
