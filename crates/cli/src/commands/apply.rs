use clap::Args;
use weaver_core::app::App;
use weaver_core::config::{ModuleManifest, WeaverConfig};
use weaver_core::engine::Engine;
use weaver_core::module::ModuleResolver;
use weaver_core::plan::PlanFile;
use weaver_core::plugin::resolver::PluginResolver;
use weaver_core::secret::SecretResolver;
use weaver_core::state::{
    FileState, State, calculate_checksum, calculate_checksum_from_bytes,
};
use weaver_core::template::{TemplateEngine, build_context};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;

#[derive(Args, Clone)]
pub struct ApplyArgs {
    /// Skip interactive approval
    #[arg(long)]
    pub auto_approve: bool,

    /// Conflict resolution strategy
    #[arg(long, default_value = "stop")]
    pub strategy: String, // Parsing enum later

    /// Apply a previously saved plan file (`wvr plan --out`). Fails if the
    /// workspace inputs no longer match the captured plan.
    #[arg(long)]
    pub plan: Option<PathBuf>,

    /// Offline mode — fail instead of fetching plugins not already cached.
    #[arg(long)]
    pub offline: bool,
}

pub async fn run(args: ApplyArgs) -> anyhow::Result<()> {
    let _ = execute(args, false).await?;
    Ok(())
}

pub async fn execute(args: ApplyArgs, dry_run: bool) -> anyhow::Result<usize> {
    info!(
        "Running {} (strategy: {}, auto-approve: {})...",
        if dry_run { "plan" } else { "apply" },
        args.strategy,
        args.auto_approve
    );

    let mut planned_changes: Vec<weaver_core::plan::PlannedChange> = Vec::new();

    // 1. Load config
    let config_path = Path::new("weaver.yaml");
    if !config_path.exists() {
        anyhow::bail!("weaver.yaml not found");
    }
    let config = WeaverConfig::load_with_includes(config_path)?;

    // Validate saved plan against current config before any mutations.
    if let Some(plan_path) = args.plan.as_deref() {
        let plan = PlanFile::load(plan_path)?;
        let current_inputs = collect_current_inputs(&config);
        if let Err(stale) = plan.verify_against_inputs(&current_inputs) {
            anyhow::bail!("{stale}");
        }
    }

    // Load State
    let state_path = Path::new(".rw/state.yaml");
    let mut state = State::load(state_path)?;

    // 2. Init components
    let mut resolver = ModuleResolver::new(None)?;
    let template_engine = TemplateEngine::new()?;
    let mut tera_context = tera::Context::new();

    // Plugin resolver for module-declared ensures backed by WASM plugins.
    let mut plugin_resolver = PluginResolver::new(PathBuf::from("."))?;
    plugin_resolver.set_offline(args.offline);
    // Honor `plugins:` overrides (path:/git:) for generic `type:` ensure
    // dispatch, not just the `wvr plugins update/list/verify` subcommands.
    plugin_resolver.set_plugins_config(config.plugins.clone());

    // Resolve declared secrets and expose them to templates under `secrets.*`.
    // The `env` provider reads env vars; other providers run as WASM plugins.
    if !config.secrets.is_empty() {
        let mut secret_values = serde_json::Map::new();
        for (name, secret_cfg) in &config.secrets {
            let value = SecretResolver::resolve(secret_cfg, &plugin_resolver)
                .await
                .map_err(|e| anyhow::anyhow!("secret '{name}': {e}"))?;
            secret_values.insert(
                name.clone(),
                serde_json::Value::String(value.expose().clone()),
            );
        }
        tera_context.insert("secrets", &secret_values);
    }

    // 3. Process Apps
    for app_config in &config.apps {
        info!("Processing app: {}", app_config.name);

        let module_config = config
            .modules
            .iter()
            .find(|m| m.name == app_config.module)
            .ok_or_else(|| anyhow::anyhow!("Module '{}' not found", app_config.module))?;

        let module_path = resolver.resolve(&module_config.name, &module_config.source, &module_config.r#ref)?;
        let manifest_path = module_path.join("weaver.module.yaml");
        let manifest = ModuleManifest::load(&manifest_path)?;

        // Resolve missing inputs (Interactive)
        let answers_path = Path::new(".rw/answers.yaml");
        let interactive = !args.auto_approve;
        let resolved_inputs = crate::prompts::resolve_missing_inputs(
            &manifest,
            &app_config.inputs,
            interactive,
            &answers_path,
            &app_config.name,
        )?;

        // Clean up stale answers for removed inputs
        let current_keys: std::collections::HashSet<String> =
            manifest.inputs.keys().cloned().collect();
        crate::prompts::cleanup_stale_answers(
            &answers_path,
            &app_config.name,
            &current_keys,
            interactive,
        )?;

        // Merge resolved inputs into config clone
        let mut app_config_resolved = app_config.clone();
        app_config_resolved.inputs.extend(resolved_inputs);

        let app = App::instantiate(&app_config_resolved, &manifest)?;
        let dest_root = PathBuf::from(&app.path);

        // Files Processing
        let files_src = module_path.join("files");
        if files_src.exists() {
            for entry in WalkDir::new(&files_src) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let rel_path = entry.path().strip_prefix(&files_src)?;
                    let dest_path = dest_root.join(rel_path);

                    // Check Drift
                    if dest_path.exists()
                        && let Some(file_state) = state.files.get(&dest_path)
                    {
                        let current_chk = calculate_checksum(&dest_path)?;
                        if file_state.checksum != current_chk {
                            if dry_run {
                                planned_changes.push(weaver_core::plan::PlannedChange {
                                    action: "drift".into(),
                                    path: dest_path.display().to_string(),
                                    preview: None,
                                });
                                info!("Drift detected for {:?}.", dest_path);
                                continue; // drifted managed file: count once as drift, skip the create push
                            } else if args.strategy == "stop" {
                                anyhow::bail!(
                                    "Drift detected for {:?}. Use --strategy overwrite to force.",
                                    dest_path
                                );
                            }
                            // !dry_run && strategy != "stop" -> fall through and overwrite below
                        }
                    }

                    // Write File
                    if dry_run {
                        planned_changes.push(weaver_core::plan::PlannedChange {
                            action: "create".into(),
                            path: dest_path.display().to_string(),
                            preview: None,
                        });
                        info!("Would copy {:?} to {:?}", entry.path(), dest_path);
                    } else {
                        Engine::ensure_file_copy(entry.path(), &dest_path)?;

                        let new_chk = calculate_checksum(&dest_path)?;
                        state
                            .files
                            .insert(dest_path.clone(), FileState::new(new_chk));
                    }
                }
            }
        }

        // Templates Processing
        let templates_src = module_path.join("templates");
        if templates_src.exists() {
            for entry in walkdir::WalkDir::new(&templates_src) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let rel_path = entry.path().strip_prefix(&templates_src)?;
                    let content = std::fs::read_to_string(entry.path())?;
                    let mut context = tera_context.clone();
                    let input_ctx = build_context(&app.inputs)?;
                    context.extend(input_ctx);

                    // Destination logic
                    let file_name = entry.file_name().to_string_lossy();
                    let dest_path = if file_name.ends_with(".j2") {
                        dest_root
                            .join(rel_path.parent().unwrap_or(Path::new("")))
                            .join(rel_path.file_stem().unwrap())
                    } else {
                        dest_root.join(rel_path)
                    };

                    // Drift Check
                    if dest_path.exists()
                        && let Some(file_state) = state.files.get(&dest_path)
                    {
                        let current_chk = calculate_checksum(&dest_path)?;
                        if file_state.checksum != current_chk {
                            if dry_run {
                                planned_changes.push(weaver_core::plan::PlannedChange {
                                    action: "drift".into(),
                                    path: dest_path.display().to_string(),
                                    preview: None,
                                });
                                info!("Drift detected for {:?}.", dest_path);
                                continue; // drifted managed file: count once as drift, skip the create push
                            } else if args.strategy == "stop" {
                                anyhow::bail!(
                                    "Drift detected for {:?}. Use --strategy overwrite to force.",
                                    dest_path
                                );
                            }
                            // !dry_run && strategy != "stop" -> fall through and overwrite below
                        }
                    }

                    if dry_run {
                        planned_changes.push(weaver_core::plan::PlannedChange {
                            action: "create".into(),
                            path: dest_path.display().to_string(),
                            preview: None,
                        });
                        info!("Would render {:?} to {:?}", entry.path(), dest_path);
                    } else {
                        let rendered = template_engine.render(&content, &context)?;
                        if let Some(parent) = dest_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(&dest_path, &rendered)?;

                        let new_chk = calculate_checksum_from_bytes(rendered.as_bytes());
                        state
                            .files
                            .insert(dest_path.clone(), FileState::new(new_chk));
                    }
                }
            }
        }

        // Build the per-app render context (mirrors the templates loop).
        // Constructed here so BOTH the app-level and module-declared ensures
        // loops can share a single EnsureContext.
        let mut app_tera_context = tera_context.clone();
        let input_ctx = build_context(&app.inputs)?;
        app_tera_context.extend(input_ctx);

        let ensure_ctx = weaver_core::ensure::EnsureContext {
            app_path: dest_root.clone(),
            dry_run,
            module_path: module_path.clone(),
            tera_context: app_tera_context,
        };

        // App-level ensures: file.* via the Ensure trait (with module/template
        // context), npm.* via the native JSON path.
        // Applied after files/templates so they can mutate generated artefacts.
        for ensure in &app_config.ensures {
            if let Some(built) = weaver_core::ensure::build_app_ensure(ensure) {
                let plan = built.plan(&ensure_ctx)?;
                if dry_run {
                    if !plan.actions.is_empty() {
                        planned_changes.push(weaver_core::plan::PlannedChange {
                            action: "update".into(),
                            path: plan.description.clone(),
                            preview: None,
                        });
                    }
                    info!("Would ensure: {}", plan.description);
                } else {
                    info!("Ensuring: {}", plan.description);
                    built.execute(&ensure_ctx)?;
                }
            } else if dry_run {
                planned_changes.push(weaver_core::plan::PlannedChange {
                    action: "update".into(),
                    path: format!("{:?}", ensure),
                    preview: None,
                });
                info!("Would apply ensure {:?} for {}", ensure, app_config.name);
            } else {
                weaver_core::ensures::apply_ensure(&dest_root, ensure)?;
            }
        }

        // Module-declared ensures (git submodules, plugin-backed npm/ai, ...).
        // These are dispatched through the ensure builder, which routes plugin
        // types to WASM plugins resolved via `plugin_resolver`.
        for ensure_config in &manifest.ensures {
            let ensure =
                weaver_core::ensure::build_ensure(ensure_config, Some(&plugin_resolver))
                    .await?;
            let plan = ensure.plan(&ensure_ctx)?;
            if dry_run {
                if !plan.actions.is_empty() {
                    planned_changes.push(weaver_core::plan::PlannedChange {
                        action: "update".into(),
                        path: plan.description.clone(),
                        preview: None,
                    });
                }
                info!("Would ensure: {}", plan.description);
            } else {
                info!("Ensuring: {}", plan.description);
                ensure.execute(&ensure_ctx)?;
            }
        }
    }

    // Persist resolved plugin versions/checksums to the lockfile.
    let resolved_plugins = plugin_resolver.get_resolved_plugins();
    if !resolved_plugins.is_empty() && !dry_run {
        let lockfile_path = Path::new("weaver.lock");
        plugin_resolver.update_lockfile(lockfile_path, &resolved_plugins)?;
    }

    // Persist accumulated module locks (commit pins) into weaver.lock,
    // merging into any existing on-disk lockfile so plugin locks survive.
    if !dry_run {
        let module_lock = resolver.take_lock();
        if !module_lock.modules.is_empty() {
            let lockfile_path = Path::new("weaver.lock");
            let mut on_disk = if lockfile_path.exists() {
                serde_yml::from_str::<weaver_core::lockfile::Lockfile>(
                    &std::fs::read_to_string(lockfile_path)?,
                )?
            } else {
                weaver_core::lockfile::Lockfile {
                    version: "1".to_string(),
                    ..Default::default()
                }
            };
            for (k, v) in module_lock.modules {
                on_disk.modules.insert(k, v);
            }
            if on_disk.version.is_empty() {
                on_disk.version = "1".to_string();
            }
            std::fs::write(lockfile_path, serde_yml::to_string(&on_disk)?)?;
        }
    }

    if !dry_run {
        state.save(state_path)?;
        info!("Apply complete.");
    } else {
        if !planned_changes.is_empty() {
            print!("{}", weaver_core::plan::render_changes(&planned_changes));
            info!("{} change(s) to converge.", planned_changes.len());
        }
        info!("Plan complete. No changes made.");
    }

    Ok(planned_changes.len())
}

/// Flatten per-app inputs into the `<app>.<input>` keys used by saved plans.
fn collect_current_inputs(config: &WeaverConfig) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    for app in &config.apps {
        for (key, value) in &app.inputs {
            let flat_key = format!("{}.{}", app.name, key);
            if let Ok(json) = serde_json::to_value(value) {
                out.insert(flat_key, json);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_core::config::{AppConfig, WeaverConfig};
    use std::collections::HashMap;

    fn make_config(inputs: HashMap<String, serde_yml::Value>) -> WeaverConfig {
        WeaverConfig {
            version: "1".into(),
            includes: vec![],
            modules: vec![],
            apps: vec![AppConfig {
                name: "billing".into(),
                module: "m".into(),
                path: ".".into(),
                inputs,
                ensures: vec![],
                checks: vec![],
            }],
            checks: vec![],
            secrets: Default::default(),
            plugins: Default::default(),
        }
    }

    #[test]
    fn collect_current_inputs_flattens_keys() {
        let mut inputs = HashMap::new();
        inputs.insert("retention_days".into(), serde_yml::Value::Number(90.into()));
        inputs.insert("name".into(), serde_yml::Value::String("svc".into()));

        let flat = collect_current_inputs(&make_config(inputs));
        assert_eq!(
            flat.get("billing.retention_days"),
            Some(&serde_json::json!(90))
        );
        assert_eq!(flat.get("billing.name"), Some(&serde_json::json!("svc")));
    }

    #[test]
    fn collect_current_inputs_empty_when_no_inputs() {
        let flat = collect_current_inputs(&make_config(HashMap::new()));
        assert!(flat.is_empty());
    }
}
