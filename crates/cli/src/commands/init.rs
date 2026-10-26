use clap::Args;
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Args)]
pub struct InitArgs {
    /// Directory to initialize
    #[arg(default_value = ".")]
    path: PathBuf,
}

pub fn run(args: InitArgs) -> anyhow::Result<()> {
    let root = args.path;
    let config_path = root.join("weaver.yaml");
    let gitignore_path = root.join(".gitignore");

    if config_path.exists() {
        anyhow::bail!("weaver.yaml already exists");
    }

    if root.exists()
        && let Ok(entries) = std::fs::read_dir(&root)
        && entries.count() > 0
    {
        warn!("Initializing in a non-empty directory");
    }

    std::fs::create_dir_all(&root)?;

    // Create weaver.yaml
    let default_config = r#"version: "1"
modules: []
apps: []
"#;
    std::fs::write(&config_path, default_config)?;
    info!("Created {}", config_path.display());

    // Create .gitignore if it doesn't exist
    if !gitignore_path.exists() {
        let default_ignore = r#".rw/
"#;
        std::fs::write(&gitignore_path, default_ignore)?;
        info!("Created {}", gitignore_path.display());
    } else {
        // Append if exists? For now, just warn or skip.
        // Spec says "Creates ... .gitignore".
        // Let's just log that we skipped it to be safe.
        info!("{} already exists, skipping", gitignore_path.display());
    }

    println!("Initialized empty workspace in {}", root.display());

    Ok(())
}
