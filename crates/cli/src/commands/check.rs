use crate::exit::ExitCode;
use clap::Args;
use comfy_table::Table;
use console::style;
use std::path::Path;
use weaver_core::check::{CheckResult, Status, run_check};
use weaver_core::config::{CheckDef, Severity, WeaverConfig};

#[derive(Args)]
pub struct CheckArgs {
    /// Filter checks by app name
    pub app: Option<String>,
}

pub async fn execute(args: CheckArgs) -> anyhow::Result<ExitCode> {
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
        return Ok(ExitCode::Success);
    }

    println!("Running {} checks...", tasks.len());

    // `cwd` on a check is resolved relative to the repo root, which today is
    // always the directory `weaver.yaml` was loaded from (cwd itself, since
    // the path above is relative -- see R27's not-yet-built `-C`/`--repo`).
    let repo_root = Path::new(".");

    let mut results: Vec<(String, CheckResult)> = Vec::with_capacity(tasks.len());
    for (context, check) in &tasks {
        let result = run_check(check, repo_root).await;
        results.push((context.clone(), result));
    }

    let mut table = Table::new();
    table.set_header(vec!["Context", "Check", "Severity", "Status", "Message"]);

    // R23: the exit code derives from severity -- only `error`-severity
    // checks that didn't pass fail the gate. `warn`/`info` are still shown
    // as not-passing, but never flip the exit code.
    let mut gate_failures = 0;
    let mut other_failures = 0;

    for (context, result) in &results {
        let status_cell = match result.status {
            Status::Pass => style("PASS").green().to_string(),
            Status::Fail => style("FAIL").red().to_string(),
            Status::Error => style("ERROR").red().bold().to_string(),
        };

        if result.status != Status::Pass {
            if result.severity == Severity::Error {
                gate_failures += 1;
            } else {
                other_failures += 1;
            }
        }

        table.add_row(vec![
            context.as_str(),
            &result.name,
            severity_label(result.severity),
            &status_cell,
            &result.observed,
        ]);
    }

    println!("{table}");

    if gate_failures > 0 {
        eprintln!("{} error-severity check(s) failed", gate_failures);
        if other_failures > 0 {
            eprintln!("({} warn/info check(s) also did not pass)", other_failures);
        }
        return Ok(ExitCode::Violations);
    }

    if other_failures > 0 {
        eprintln!("{} warn/info check(s) did not pass (not failing the gate)", other_failures);
    }

    Ok(ExitCode::Success)
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}
