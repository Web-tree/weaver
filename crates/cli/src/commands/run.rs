use clap::Args;
use weaver_core::config::{ModuleManifest, WeaverConfig};
use weaver_core::module::ModuleResolver;
use std::path::Path;
use std::process::Command;
use tracing::info;

#[derive(Args)]
pub struct RunArgs {
    /// App name
    pub app_name: String,

    /// Task name
    pub task_name: String,

    /// Additional arguments to pass to the task command
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

pub async fn run(args: RunArgs) -> anyhow::Result<()> {
    info!(
        "Running task '{}' for app '{}'...",
        args.task_name, args.app_name
    );

    // Load config
    let config_path = Path::new("weaver.yaml");
    if !config_path.exists() {
        anyhow::bail!("weaver.yaml not found");
    }
    let config = WeaverConfig::load(config_path)?;

    // Find app
    let app_config = config
        .apps
        .iter()
        .find(|a| a.name == args.app_name)
        .ok_or_else(|| anyhow::anyhow!("App '{}' not found", args.app_name))?;

    // Find module
    let module_config = config
        .modules
        .iter()
        .find(|m| m.name == app_config.module)
        .ok_or_else(|| anyhow::anyhow!("Module '{}' not found", app_config.module))?;

    // Resolve module
    let mut resolver = ModuleResolver::new(None)?;
    let module_path = resolver.resolve(&module_config.name, &module_config.source, &module_config.r#ref)?;

    // Load manifest
    let manifest_path = module_path.join("weaver.module.yaml");
    let manifest = ModuleManifest::load(&manifest_path)?;

    // Find task
    let task = manifest
        .tasks
        .get(&args.task_name)
        .ok_or_else(|| anyhow::anyhow!("Task '{}' not found in module", args.task_name))?;

    info!("Executing: {}", task.command);

    // Execute task command
    // Parse command (simple split on whitespace for MVP)
    let parts: Vec<&str> = task.command.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Empty task command");
    }

    let mut cmd = Command::new(parts[0]);
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    // Append user-provided args
    if !args.args.is_empty() {
        cmd.args(&args.args);
    }

    // Set working directory to app path
    cmd.current_dir(&app_config.path);

    // Execute and stream output
    let status = cmd.status()?;

    if !status.success() {
        let exit_code = status.code().unwrap_or(1);
        anyhow::bail!(
            "Task '{}' failed with exit code {}",
            args.task_name,
            exit_code
        );
    }

    info!("Task '{}' completed successfully", args.task_name);
    Ok(())
}
