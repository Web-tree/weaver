use clap::Args;
use comfy_table::Table;
use console::style;
use weaver_core::config::{CheckDef, WeaverConfig};
use std::path::Path;
use std::process::Command;

#[derive(Args)]
pub struct CheckArgs {
    /// Filter checks by app name
    pub app: Option<String>,
}

pub fn execute(args: CheckArgs) -> anyhow::Result<()> {
    let config = WeaverConfig::load(Path::new("weaver.yaml"))?;

    // Collect all checks to run
    // Structure: App Name -> [CheckDef]
    // Global checks associated with "Global" or empty app name?
    // Let's use a list of (App Name, CheckDef).

    let mut tasks: Vec<(String, CheckDef)> = Vec::new(); // (Context, Check)

    // Global checks
    if args.app.is_none() {
        for check in &config.checks {
            tasks.push(("Global".to_string(), check.clone()));
        }
    }

    // App checks
    for app in &config.apps {
        if let Some(target_app) = &args.app {
            if &app.name != target_app {
                continue;
            }
        }

        for check in &app.checks {
            tasks.push((app.name.clone(), check.clone()));
        }

        // Also look for checks in module logic?
        // Plan says: "For each check in config.checks and app.checks".
        // Module checks are not mentioned in plan explicitly but logic might be similar.
        // For MVP, stick to config.checks and app.checks.
    }

    if tasks.is_empty() {
        // If app was specified but no checks found, maybe app doesn't exist?
        if let Some(target_app) = &args.app {
            let app_exists = config.apps.iter().any(|a| &a.name == target_app);
            if !app_exists {
                anyhow::bail!("App '{}' not found", target_app);
            }
        }
        println!("No checks defined in weaver.yaml");
        return Ok(());
    }

    println!("Running {} checks...", tasks.len());

    let mut failures = 0;
    let mut table = Table::new();
    table.set_header(vec!["Context", "Check", "Status", "Message"]);

    for (context, check) in tasks {
        match run_check_command(&check.command) {
            Ok(_) => {
                table.add_row(vec![
                    &context,
                    &check.name,
                    &style("PASS").green().to_string(),
                    "",
                ]);
            }
            Err(e) => {
                failures += 1;
                table.add_row(vec![
                    &context,
                    &check.name,
                    &style("FAIL").red().to_string(),
                    &e.to_string(),
                ]);
            }
        }
    }

    println!("{table}");

    if failures > 0 {
        anyhow::bail!("{} checks failed", failures);
    }

    Ok(())
}

fn run_check_command(command: &str) -> anyhow::Result<()> {
    let output = Command::new("sh").arg("-c").arg(command).output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Combine output for error message
        let msg = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("Exit code {}", output.status)
        };
        anyhow::bail!("{}", msg);
    }
}
