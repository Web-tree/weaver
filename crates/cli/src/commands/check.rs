use crate::exit::ExitCode;
use clap::Args;
use comfy_table::Table;
use console::style;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use weaver_core::check::{CheckResult, CheckSource, Status, run_check};
use weaver_core::config::{CheckDef, ModuleConfig, ModuleManifest, Severity, WeaverConfig};
use weaver_core::lockfile::Lockfile;
use weaver_core::module::ModuleResolver;

#[derive(Args)]
pub struct CheckArgs {
    /// Filter checks by app name
    pub app: Option<String>,
}

/// One check to run, with where it came from (R2) and the directory its
/// `cwd` resolves relative to.
struct Task {
    source: CheckSource,
    check: CheckDef,
    repo_root: PathBuf,
}

/// `wvr check --json`'s report (R22). `version` lets a future change to this
/// shape be detected by consumers instead of guessed at.
#[derive(Serialize)]
struct JsonReport<'a> {
    version: u32,
    summary: JsonSummary,
    results: Vec<JsonCheckResult<'a>>,
}

#[derive(Serialize, Default)]
struct JsonSummary {
    total: usize,
    pass: usize,
    fail: usize,
    error: usize,
    /// How many of the above are `error`-severity and did not pass -- the
    /// count the exit code (R24: `2` on any gate failure) derives from.
    gate_failures: usize,
}

/// One rule's outcome, shaped directly off [`CheckResult`] plus the pieces
/// R22 names that don't live there: the evaluated target path, and
/// `profile`.
///
/// `profile` is R6 (a later slice) and doesn't exist yet in this codebase --
/// rather than omit the key (which would make a consumer's schema differ
/// between "no profiles configured" and "profiles not implemented"), it is
/// always emitted as an explicit JSON `null`, so the key is stable across
/// this slice and the one that gives it real values.
#[derive(Serialize)]
struct JsonCheckResult<'a> {
    id: &'a str,
    qualified_id: &'a str,
    name: &'a str,
    severity: Severity,
    status: Status,
    source: &'a CheckSource,
    /// The filesystem path (relative to cwd) the check actually ran
    /// against -- R22's "target path(s)".
    target: String,
    profile: Option<String>,
    observed: &'a str,
    expected: &'a str,
    remediate: Option<&'a str>,
}

pub async fn execute(args: CheckArgs, json: bool) -> anyhow::Result<ExitCode> {
    let config = WeaverConfig::load(Path::new("weaver.yaml"))?;

    // `check` is strictly read-only and must not require the network once a
    // module is already resolved (R8) -- load the lockfile so module
    // resolution below can prefer the pinned commit + warm cache over a
    // fresh `git` round trip.
    let lockfile_path = Path::new("weaver.lock");
    let lockfile = if lockfile_path.exists() {
        serde_yml::from_str::<Lockfile>(&std::fs::read_to_string(lockfile_path)?)?
    } else {
        Lockfile::default()
    };
    let mut resolver = ModuleResolver::new(Some(lockfile))?;

    let mut tasks: Vec<Task> = Vec::new();
    let workspace_root = PathBuf::from(".");

    // Global (workspace) checks.
    if args.app.is_none() {
        for check in &config.checks {
            tasks.push(Task {
                source: CheckSource::Workspace,
                check: check.clone(),
                repo_root: workspace_root.clone(),
            });
        }
    }

    // A module is resolved (and its manifest loaded) at most once per run,
    // no matter how many apps consume it -- its checks are then attributed
    // to each consuming app separately (R1's "runs once per consuming app").
    let mut resolved_modules: HashMap<String, (String, ModuleManifest)> = HashMap::new();

    // App checks, plus checks inherited from the module each app consumes.
    for app in &config.apps {
        if let Some(target_app) = &args.app
            && &app.name != target_app
        {
            continue;
        }

        for check in &app.checks {
            tasks.push(Task {
                source: CheckSource::App { app: app.name.clone() },
                check: check.clone(),
                repo_root: workspace_root.clone(),
            });
        }

        // The app's module must be declared to inherit anything from it.
        let module_config = match config.modules.iter().find(|m| m.name == app.module) {
            Some(m) => m,
            None if config.modules.is_empty() => {
                // No `modules:` block at all: there is genuinely nothing in
                // this workspace to inherit checks from (a bare `check.command`
                // fixture, or a workspace that hasn't adopted any module
                // yet), so this is not a rule silently failing to run --
                // there was never a rule. Still warn, so a real typo in
                // `module:` is visible rather than silent, and keep going
                // (this is what the pre-existing check fixtures rely on).
                eprintln!(
                    "Warning: app '{}' references module '{}', but weaver.yaml declares no modules at all -- skipping module-inherited checks for this app.",
                    app.name, app.module
                );
                continue;
            }
            None => {
                // `modules:` IS declared, but `app.module` doesn't match any
                // entry -- this is a typo, not an absence, and it means a
                // module's rules silently never run while the report still
                // looks green. That is the worst possible outcome here (no
                // false green, requirements §6), so this is a hard error,
                // not a skip.
                let available: Vec<&str> = config.modules.iter().map(|m| m.name.as_str()).collect();
                anyhow::bail!(
                    "App '{}' references module '{}', which is not declared in weaver.yaml. \
                     Declared modules: {}.",
                    app.name,
                    app.module,
                    available.join(", "),
                );
            }
        };

        if !resolved_modules.contains_key(&module_config.name) {
            let (module_path, resolved_commit) = resolve_module_for_check(&mut resolver, module_config)?;
            // Under `--json`, stdout must carry the JSON report and nothing
            // else (R22) -- this line is presentation, not the report
            // itself (the resolved commit is already on every module
            // check's `source` in the report).
            if !json {
                println!(
                    "Module '{}' resolved to {} @ {}",
                    module_config.name, module_config.source, resolved_commit
                );
            }
            let manifest = ModuleManifest::load(&module_path.join("weaver.module.yaml"))?;
            resolved_modules.insert(module_config.name.clone(), (resolved_commit, manifest));
        }

        let (resolved_commit, manifest) =
            resolved_modules.get(&module_config.name).expect("just inserted above");

        // Module checks run once per consuming app, with `cwd` resolved
        // relative to that app's own path (R1) -- so a module check can
        // assert about the app it configures.
        let app_root = PathBuf::from(&app.path);
        for check in &manifest.checks {
            tasks.push(Task {
                source: CheckSource::Module {
                    module: module_config.name.clone(),
                    resolved_commit: resolved_commit.clone(),
                    app: app.name.clone(),
                },
                check: check.clone(),
                repo_root: app_root.clone(),
            });
        }
    }

    if tasks.is_empty() {
        // If app was specified but no checks found, maybe app doesn't exist?
        if let Some(target_app) = &args.app {
            let app_exists = config.apps.iter().any(|a| &a.name == target_app);
            if !app_exists {
                anyhow::bail!("App '{}' not found", target_app);
            }
        }
        if json {
            print_json_report(&[], &[]);
        } else {
            println!("No checks defined in weaver.yaml or any adopted module");
        }
        return Ok(ExitCode::Success);
    }

    if !json {
        println!("Running {} checks...", tasks.len());
    }

    let mut results: Vec<CheckResult> = Vec::with_capacity(tasks.len());
    for task in &tasks {
        let result = run_check(&task.check, &task.repo_root, task.source.clone()).await;
        results.push(result);
    }

    // R23: the exit code derives from severity -- only `error`-severity
    // checks that didn't pass fail the gate. `warn`/`info` are still shown
    // as not-passing, but never flip the exit code. Computed once and shared
    // by both the table and the JSON report so the two can never disagree.
    let mut gate_failures = 0;
    let mut other_failures = 0;
    for result in &results {
        if result.status != Status::Pass {
            if result.severity == Severity::Error {
                gate_failures += 1;
            } else {
                other_failures += 1;
            }
        }
    }

    if json {
        print_json_report(&tasks, &results);
    } else {
        print_table(&results);
    }

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

/// Render the human-readable `comfy_table` report to stdout, exactly as
/// before `--json` existed.
fn print_table(results: &[CheckResult]) {
    let mut table = Table::new();
    table.set_header(vec!["Context", "Check", "Rule ID", "Severity", "Status", "Message"]);

    for result in results {
        let status_cell = match result.status {
            Status::Pass => style("PASS").green().to_string(),
            Status::Fail => style("FAIL").red().to_string(),
            Status::Error => style("ERROR").red().bold().to_string(),
        };

        let context = result.source.context_label();
        table.add_row(vec![
            context.as_str(),
            &result.name,
            &result.qualified_id,
            severity_label(result.severity),
            &status_cell,
            &result.observed,
        ]);
    }

    println!("{table}");
}

/// Emit the `--json` report (R22) as a single JSON document on stdout, and
/// nothing else -- see this module's and `logging.rs`'s docs for how stdout
/// purity is guaranteed. `tasks` and `results` are index-aligned: both are
/// built by appending to the same list, one entry per check, in the same
/// order.
fn print_json_report(tasks: &[Task], results: &[CheckResult]) {
    debug_assert_eq!(tasks.len(), results.len());

    let mut summary = JsonSummary::default();
    let mut json_results = Vec::with_capacity(results.len());

    for (task, result) in tasks.iter().zip(results.iter()) {
        summary.total += 1;
        match result.status {
            Status::Pass => summary.pass += 1,
            Status::Fail => summary.fail += 1,
            Status::Error => summary.error += 1,
        }
        if result.status != Status::Pass && result.severity == Severity::Error {
            summary.gate_failures += 1;
        }

        json_results.push(JsonCheckResult {
            id: &result.id,
            qualified_id: &result.qualified_id,
            name: &result.name,
            severity: result.severity,
            status: result.status,
            source: &result.source,
            target: task.repo_root.display().to_string(),
            profile: None,
            observed: &result.observed,
            expected: &result.expected,
            remediate: result.remediate.as_deref(),
        });
    }

    let report = JsonReport { version: 1, summary, results: json_results };
    // `serde_json::to_string` cannot fail for these plain-data types (no
    // maps with non-string keys, no `f32`/`f64` NaN/inf); `expect` documents
    // that rather than threading a spurious `Result` through a print path.
    println!("{}", serde_json::to_string(&report).expect("JsonReport always serializes"));
}

/// Resolve a module for `check`, preferring the lockfile's pin and the
/// already-warm module cache so a module that's already resolved needs no
/// `git` network operation at all (R8). Only falls back to a real
/// resolution when there's no usable pin/cache, and if that also fails,
/// returns a clear, actionable error -- `check` must never silently skip a
/// module's checks (no false green, §6).
fn resolve_module_for_check(
    resolver: &mut ModuleResolver,
    module_config: &ModuleConfig,
) -> anyhow::Result<(PathBuf, String)> {
    if let Some(hit) = resolver.resolve_from_cache(&module_config.name, &module_config.source, &module_config.r#ref)
    {
        return Ok(hit);
    }

    resolver
        .resolve_with_commit(&module_config.name, &module_config.source, &module_config.r#ref)
        .map_err(|err| {
            anyhow::anyhow!(
                "Module '{}' ({}@{}) is not pinned/cached and could not be resolved now: {err}\n\
                 Run `wvr apply` first to pin and cache this module, then re-run `wvr check`.",
                module_config.name,
                module_config.source,
                module_config.r#ref,
            )
        })
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}
