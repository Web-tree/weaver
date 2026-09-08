use clap::{Args, Subcommand};
use comfy_table::Table;
use weaver_core::config::{ModuleConfig, WeaverConfig};
use weaver_core::lockfile::Lockfile;
use weaver_core::module::ModuleResolver;
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct ModuleArgs {
    #[command(subcommand)]
    pub command: ModuleCommands,
}

#[derive(Subcommand)]
pub enum ModuleCommands {
    /// List defined modules
    List(ListArgs),
    /// Update a module's ref
    Update(UpdateArgs),
    /// Add a module to weaver.yaml and pin it in weaver.lock
    Add(AddArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Module source (git URL or local path)
    pub source: String,
    /// Module name (defaults to the repo name derived from the source)
    #[arg(long)]
    pub name: Option<String>,
    /// Git ref (branch, tag, or commit)
    #[arg(long, default_value = "main")]
    pub r#ref: String,
}

#[derive(Args)]
pub struct ListArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct UpdateArgs {
    /// Module name
    pub name: String,
    /// New git ref
    #[arg(long)]
    pub r#ref: String,
    /// Skip fetching/clearing cache
    #[arg(long)]
    pub no_fetch: bool,
}

pub fn execute(args: ModuleArgs) -> anyhow::Result<()> {
    match args.command {
        ModuleCommands::List(args) => run_list(args),
        ModuleCommands::Update(args) => run_update(args),
        ModuleCommands::Add(args) => run_add(args),
    }
}

fn run_list(args: ListArgs) -> anyhow::Result<()> {
    let config = WeaverConfig::load(Path::new("weaver.yaml"))?;

    if args.json {
        let mut modules_json = Vec::new();
        for module in &config.modules {
            modules_json.push(json!({
                "name": module.name,
                "source": module.source,
                "ref": module.r#ref,
            }));
        }
        println!("{}", serde_json::to_string_pretty(&modules_json)?);
    } else {
        if config.modules.is_empty() {
            println!("No modules defined in weaver.yaml");
            return Ok(());
        }

        println!("MODULES:");
        let mut table = Table::new();
        table.set_header(vec!["Name", "Source", "Ref"]);

        for module in &config.modules {
            table.add_row(vec![&module.name, &module.source, &module.r#ref]);
        }
        println!("{table}");
    }

    Ok(())
}

fn derive_name(source: &str) -> String {
    source
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("module")
        .trim_end_matches(".git")
        .to_string()
}

fn run_add(args: AddArgs) -> anyhow::Result<()> {
    let config_path = Path::new("weaver.yaml");
    let mut config = WeaverConfig::load(config_path)?;

    let name = args.name.clone().unwrap_or_else(|| derive_name(&args.source));

    // Idempotent: if the module already exists, update it in place; otherwise
    // append a new entry. Resolve/pin and file writes happen the same way in
    // both cases (below the branch).
    let is_update = match config.modules.iter_mut().find(|m| m.name == name) {
        Some(existing) => {
            existing.source = args.source.clone();
            existing.r#ref = args.r#ref.clone();
            true
        }
        None => {
            config.modules.push(ModuleConfig {
                name: name.clone(),
                source: args.source.clone(),
                r#ref: args.r#ref.clone(),
                path: None,
            });
            false
        }
    };

    // Resolve + pin the commit (also clones into the global store).
    let mut resolver = ModuleResolver::new(None)?;
    resolver.resolve(&name, &args.source, &args.r#ref)?;

    let f = std::fs::File::create(config_path)?;
    serde_yml::to_writer(f, &config)?;

    // Persist the module lock (merge into existing weaver.lock if present).
    let lock_path = Path::new("weaver.lock");
    let mut lock = if lock_path.exists() {
        serde_yml::from_str::<Lockfile>(&std::fs::read_to_string(lock_path)?)?
    } else {
        Lockfile {
            version: "1".to_string(),
            ..Default::default()
        }
    };
    let resolved = resolver.take_lock();
    if let Some(ml) = resolved.modules.get(&name) {
        lock.modules.insert(name.clone(), ml.clone());
    }
    std::fs::write(lock_path, serde_yml::to_string(&lock)?)?;

    if is_update {
        println!("Updated module '{}' ({} @ {})", name, args.source, args.r#ref);
    } else {
        println!("Added module '{}' ({} @ {})", name, args.source, args.r#ref);
    }
    println!("Run 'wvr plan' to preview what will converge.");
    Ok(())
}

fn run_update(args: UpdateArgs) -> anyhow::Result<()> {
    let config_path = Path::new("weaver.yaml");
    // We need to read as raw string to preserve structure/comments if possible,
    // but for now we'll load/modify/save using WeaverConfig for simplicity
    // as we don't have a widespread comment-preserving yaml editor yet.
    // Wait, the plan says "Read as raw string... but for now load/modify/save".
    // Actually, `serde_yml` might destroy comments.
    // If we want to be safe, we should probably stick to `WeaverConfig` load/save
    // acknowledging we might lose comments, or try a regex replacement if we want to be surgical.
    // Given MVP, let's stick to `WeaverConfig` load/save but be aware of comment loss.
    // Or better, let's try to just update the specific module ref if we can find it in the file content strings.
    // BUT, reliability first. Let's use deserialization/serialization for correctness of structure.

    let mut config = WeaverConfig::load(config_path)?;

    let module = config.modules.iter_mut().find(|m| m.name == args.name);

    match module {
        Some(m) => {
            let old_ref = m.r#ref.clone();
            m.r#ref = args.r#ref.clone();

            // Save back
            let f = std::fs::File::create(config_path)?;
            serde_yml::to_writer(f, &config)?;

            println!(
                "Updated module '{}' from {} to {}",
                args.name, old_ref, args.r#ref
            );

            if !args.no_fetch {
                // Clear cache
                let cache_dir = PathBuf::from(".rw/cache").join(&args.name);
                if cache_dir.exists() {
                    std::fs::remove_dir_all(&cache_dir)?;
                    println!("Module cache cleared. Run 'wvr apply' to fetch new version.");
                }
            }
        }
        None => {
            // Build available list
            let available: Vec<_> = config.modules.iter().map(|m| &m.name).collect();
            anyhow::bail!(
                "Module '{}' not found. Available: {}",
                args.name,
                available
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    Ok(())
}
