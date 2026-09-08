use clap::Args;
use weaver_core::config::{ModuleManifest, WeaverConfig};
use weaver_core::module::ModuleResolver;
use std::collections::HashMap;
use std::path::Path;

#[derive(Args)]
pub struct ListArgs {
    /// Output in JSON format
    #[arg(long)]
    pub json: bool,

    /// Show only apps
    #[arg(long, conflicts_with = "tasks_only")]
    pub apps_only: bool,

    /// Show only tasks
    #[arg(long, conflicts_with = "apps_only")]
    pub tasks_only: bool,
}

pub async fn run(args: ListArgs) -> anyhow::Result<()> {
    // Load config
    let config_path = Path::new("weaver.yaml");
    if !config_path.exists() {
        // T027: Handle empty workspace
        if args.json {
            println!("{{ \"apps\": [], \"tasks\": [] }}");
        } else {
            eprintln!("No apps defined (weaver.yaml not found)");
        }
        std::process::exit(2);
    }

    let config = WeaverConfig::load_with_includes(config_path)?;

    if args.json {
        print_json(&config, &args)?;
    } else {
        print_table(&config, &args)?;
    }

    Ok(())
}

fn print_json(config: &WeaverConfig, args: &ListArgs) -> anyhow::Result<()> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct AppInfo<'a> {
        name: &'a str,
        path: &'a str,
        module: &'a str,
    }

    #[derive(Serialize)]
    struct TaskInfoOwned {
        app: String,
        name: String,
        description: String,
    }

    let apps: Vec<AppInfo> = config
        .apps
        .iter()
        .map(|a| AppInfo {
            name: &a.name,
            path: &a.path,
            module: &a.module,
        })
        .collect();

    let mut tasks_owned = Vec::new();
    if !args.apps_only {
        // Logic for task listing
        let mut resolver = ModuleResolver::new(None)?;
        let module_map: HashMap<_, _> = config.modules.iter().map(|m| (&m.name, m)).collect();

        for app in &config.apps {
            if let Some(mod_config) = module_map.get(&app.module) {
                match resolver.resolve(&mod_config.name, &mod_config.source, &mod_config.r#ref) {
                    Ok(path) => {
                        let manifest_path = path.join("weaver.module.yaml");
                        if manifest_path.exists() {
                            if let Ok(manifest) = ModuleManifest::load(&manifest_path) {
                                for (task_name, task_def) in manifest.tasks {
                                    tasks_owned.push(TaskInfoOwned {
                                        app: app.name.clone(),
                                        name: task_name,
                                        description: task_def.description.unwrap_or_default(),
                                    });
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        // Sort
        tasks_owned.sort_by(|a, b| a.app.cmp(&b.app).then(a.name.cmp(&b.name)));
    }

    let json_output = serde_json::json!({
        "apps": if !args.tasks_only { Some(apps) } else { None },
        "tasks": if !args.apps_only { Some(tasks_owned) } else { None }
    });

    println!("{}", serde_json::to_string_pretty(&json_output)?);
    Ok(())
}

fn print_table(config: &WeaverConfig, args: &ListArgs) -> anyhow::Result<()> {
    if !args.tasks_only {
        if config.apps.is_empty() {
            println!("No apps defined.");
        } else {
            println!("{:<20} {:<30} {:<20}", "APP", "PATH", "MODULE");
            println!("{:<20} {:<30} {:<20}", "---", "----", "------");
            for app in &config.apps {
                println!("{:<20} {:<30} {:<20}", app.name, app.path, app.module);
            }
        }
    }

    if !args.apps_only {
        let mut resolver = ModuleResolver::new(None)?;
        let module_map: HashMap<_, _> = config.modules.iter().map(|m| (&m.name, m)).collect();
        let mut tasks = Vec::new();

        for app in &config.apps {
            if let Some(mod_config) = module_map.get(&app.module) {
                if let Ok(path) = resolver.resolve(&mod_config.name, &mod_config.source, &mod_config.r#ref) {
                    let manifest_path = path.join("weaver.module.yaml");
                    if manifest_path.exists() {
                        if let Ok(manifest) = ModuleManifest::load(&manifest_path) {
                            for (task_name, task_def) in manifest.tasks {
                                tasks.push((
                                    app.name.clone(),
                                    task_name,
                                    task_def.description.unwrap_or_default(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        if !tasks.is_empty() {
            // Sort
            tasks.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

            println!("\nTASKS:");
            for (app, task, desc) in tasks {
                // app:task format
                let full_name = format!("{}:{}", app, task);
                println!("  {:<30} {}", full_name, desc);
            }
        } else if args.tasks_only {
            println!("No tasks found (modules might not be resolved or define no tasks).");
        }
    }

    Ok(())
}
