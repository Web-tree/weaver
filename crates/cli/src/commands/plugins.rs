use clap::{Args, Subcommand};
use weaver_core::config::WeaverConfig;
use weaver_core::lockfile::Lockfile;
use weaver_core::plugin::cache::PluginCache;
use weaver_core::plugin::resolver::PluginResolver;
use std::path::{Path, PathBuf};

/// Manage plugins
#[derive(Args)]
pub struct PluginsArgs {
    #[command(subcommand)]
    command: PluginCommands,
}

#[derive(Subcommand)]
enum PluginCommands {
    /// List all configured plugins
    List,
    /// Verify lockfile integrity
    Verify,
    /// Update plugin versions
    Update {
        /// Plugin name to update (omit for --all)
        plugin: Option<String>,
        /// Update all plugins
        #[arg(long)]
        all: bool,
    },
    /// Remove unused plugin versions from cache
    Prune,
}

pub async fn execute(args: PluginsArgs) -> anyhow::Result<()> {
    match args.command {
        PluginCommands::List => execute_list(),
        PluginCommands::Verify => execute_verify(),
        PluginCommands::Update { plugin, all } => execute_update(plugin, all).await,
        PluginCommands::Prune => {
            println!("Plugin prune command - not yet implemented");
            Ok(())
        }
    }
}

fn execute_list() -> anyhow::Result<()> {
    let lockfile_path = Path::new("weaver.lock");

    if !lockfile_path.exists() {
        println!("No plugins found (weaver.lock does not exist)");
        println!("Run 'wvr apply' to resolve and lock plugins");
        return Ok(());
    }

    let content = std::fs::read_to_string(lockfile_path)?;
    let lockfile: Lockfile = serde_yml::from_str(&content)?;

    if lockfile.plugins.is_empty() {
        println!("No plugins configured");
        return Ok(());
    }

    let cache = PluginCache::default();

    // Print header
    println!(
        "{:<20} {:<15} {:<50} {:<10}",
        "NAME", "VERSION", "SOURCE", "STATUS"
    );
    println!("{}", "-".repeat(95));

    for (name, lock) in &lockfile.plugins {
        let status = if lock.source.starts_with("path:") {
            "local".to_string()
        } else if cache.has(name, &lock.version) {
            "cached".to_string()
        } else {
            "missing".to_string()
        };

        println!(
            "{:<20} {:<15} {:<50} {:<10}",
            name, lock.version, &lock.source, status
        );
    }

    Ok(())
}

fn execute_verify() -> anyhow::Result<()> {
    let lockfile_path = Path::new("weaver.lock");

    if !lockfile_path.exists() {
        anyhow::bail!("weaver.lock not found. Run 'wvr apply' first.");
    }

    let content = std::fs::read_to_string(lockfile_path)?;
    let lockfile: Lockfile = serde_yml::from_str(&content)?;

    if lockfile.plugins.is_empty() {
        println!("✓ No plugins to verify");
        return Ok(());
    }

    let resolver = PluginResolver::new(PathBuf::from("."))?;
    let mut all_valid = true;

    for (name, lock) in &lockfile.plugins {
        // Skip local path plugins
        if lock.source.starts_with("path:") {
            println!("  {} - skipped (local path)", name);
            continue;
        }

        match resolver.verify(name, lock) {
            Ok(true) => {
                println!("✓ {} - checksum valid", name);
            }
            Err(e) => {
                println!("✗ {} - {}", name, e);
                all_valid = false;
            }
            Ok(false) => {
                // This shouldn't happen based on current verify() implementation
                println!("✗ {} - verification failed", name);
                all_valid = false;
            }
        }
    }

    if all_valid {
        println!("\n✓ All plugins verified successfully");
        Ok(())
    } else {
        anyhow::bail!("Some plugins failed verification. Run 'wvr plugins update' to fix.");
    }
}

async fn execute_update(plugin: Option<String>, all: bool) -> anyhow::Result<()> {
    // Validate arguments
    if !all && plugin.is_none() {
        anyhow::bail!("Specify a plugin name or use --all");
    }

    // Load weaver.yaml for plugin configurations
    let config_path = Path::new("weaver.yaml");
    if !config_path.exists() {
        anyhow::bail!("weaver.yaml not found. No plugins configured.");
    }

    let config = WeaverConfig::load(config_path)?;

    if config.plugins.is_empty() {
        println!("No plugins configured in weaver.yaml");
        return Ok(());
    }

    // Determine which plugins to update
    let plugins_to_update: Vec<(&String, &weaver_core::config::PluginConfig)> = if all {
        config.plugins.iter().collect()
    } else if let Some(ref name) = plugin {
        if let Some(plugin_config) = config.plugins.get(name) {
            vec![(&name, plugin_config)]
        } else {
            anyhow::bail!("Plugin '{}' not found in weaver.yaml", name);
        }
    } else {
        unreachable!()
    };

    // Create resolver
    let resolver = PluginResolver::new(PathBuf::from("."))?;
    let mut updated_plugins = Vec::new();
    let mut warnings = Vec::new();

    println!("Updating {} plugin(s)...\n", plugins_to_update.len());

    for (name, plugin_config) in plugins_to_update {
        // Check if plugin is pinned to a commit hash
        if let Some(ref git_ref) = plugin_config.git_ref {
            if is_commit_hash(git_ref) {
                let warning = format!(
                    "⚠️  {} - Plugin pinned to commit hash '{}'. No update available.",
                    name, git_ref
                );
                warnings.push(warning.clone());
                println!("{}", warning);
                continue;
            }
        }

        // Skip local path plugins
        if plugin_config.path.is_some() {
            println!("  {} - skipped (local path)", name);
            continue;
        }

        // Resolve the plugin to get the latest version
        print!("  {} - resolving...", name);
        match resolver.resolve(name, plugin_config).await {
            Ok(resolved) => {
                println!(" ✓ updated to {}", resolved.version);
                updated_plugins.push((name.to_string(), resolved));
            }
            Err(e) => {
                println!(" ✗ failed: {}", e);
                anyhow::bail!("Failed to update plugin '{}': {}", name, e);
            }
        }
    }

    // Update lockfile if we have any updates
    if !updated_plugins.is_empty() {
        let lockfile_path = Path::new("weaver.lock");
        resolver.update_lockfile(lockfile_path, &updated_plugins)?;
        println!(
            "\n✓ Updated {} plugin(s) in weaver.lock",
            updated_plugins.len()
        );
    } else if warnings.is_empty() {
        println!("\nNo plugins were updated.");
    }

    Ok(())
}

/// Check if a git ref looks like a commit hash (40 hex characters)
fn is_commit_hash(git_ref: &str) -> bool {
    git_ref.len() == 40 && git_ref.chars().all(|c| c.is_ascii_hexdigit())
}
