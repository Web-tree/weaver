use dialoguer::{theme::ColorfulTheme, Input};
use weaver_core::config::ModuleManifest;
use serde_yml::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

type AnswersMap = HashMap<String, HashMap<String, Value>>;

pub fn load_answers(path: &Path) -> anyhow::Result<AnswersMap> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(HashMap::new());
    }

    // Try parsing as namespaced format first
    if let Ok(answers) = serde_yml::from_str::<AnswersMap>(&content) {
        return Ok(answers);
    }

    // Legacy flat format migration: wrap in "_default" namespace
    if let Ok(flat) = serde_yml::from_str::<HashMap<String, Value>>(&content) {
        let mut migrated = HashMap::new();
        migrated.insert("_default".to_string(), flat);
        return Ok(migrated);
    }

    Ok(HashMap::new())
}

pub fn save_answers(
    path: &Path,
    app_name: &str,
    answers: &HashMap<String, Value>,
) -> anyhow::Result<()> {
    let mut all = load_answers(path).unwrap_or_default();

    let app_answers = all.entry(app_name.to_string()).or_default();
    for (k, v) in answers {
        app_answers.insert(k.clone(), v.clone());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Sort for stable output
    let sorted: BTreeMap<_, BTreeMap<_, _>> = all
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect();

    let content = serde_yml::to_string(&sorted)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn cleanup_stale_answers(
    path: &Path,
    app_name: &str,
    current_keys: &HashSet<String>,
    interactive: bool,
) -> anyhow::Result<()> {
    let mut all = load_answers(path).unwrap_or_default();
    let Some(app_answers) = all.get_mut(app_name) else {
        return Ok(());
    };

    let stale: Vec<String> = app_answers
        .keys()
        .filter(|k| !current_keys.contains(k.as_str()))
        .cloned()
        .collect();

    if stale.is_empty() {
        return Ok(());
    }

    tracing::warn!(
        "App '{}': stale answers found for removed inputs: {:?}",
        app_name,
        stale
    );

    let should_remove = if interactive {
        let theme = ColorfulTheme::default();
        dialoguer::Confirm::with_theme(&theme)
            .with_prompt("Remove stale answers?")
            .default(false)
            .interact()?
    } else {
        false
    };

    if should_remove {
        for k in &stale {
            app_answers.remove(k);
        }
        save_answers(path, app_name, &HashMap::new())?;
        tracing::info!("Removed stale answers for app '{}'", app_name);
    }

    Ok(())
}

pub fn resolve_missing_inputs(
    manifest: &ModuleManifest,
    provided_inputs: &HashMap<String, Value>,
    interactive: bool,
    answers_file: &Path,
    app_name: &str,
) -> anyhow::Result<HashMap<String, Value>> {
    let mut resolved = HashMap::new();
    let theme = ColorfulTheme::default();

    let all_answers = load_answers(answers_file).unwrap_or_default();
    let saved_answers = all_answers.get(app_name).cloned().unwrap_or_default();
    let mut new_answers = HashMap::new();

    for (key, def) in &manifest.inputs {
        // 1. Provided explicitly?
        if let Some(val) = provided_inputs.get(key) {
            resolved.insert(key.clone(), val.clone());
            continue;
        }

        // 2. Saved answer?
        if let Some(val) = saved_answers.get(key) {
            resolved.insert(key.clone(), val.clone());
            continue;
        }

        // 3. Default?
        if let Some(default_val) = &def.default {
            resolved.insert(key.clone(), default_val.clone());
            continue;
        }

        // 4. Missing and Required -> Prompt
        if !interactive {
            anyhow::bail!(
                "Missing required input '{}' and interactive mode disabled.",
                key
            );
        }

        let prompt_text = if let Some(desc) = &def.description {
            format!("{} ({})", key, desc)
        } else {
            key.clone()
        };

        let input_str: String = Input::with_theme(&theme)
            .with_prompt(&prompt_text)
            .interact_text()?;

        let val = match def.r#type.as_str() {
            "number" => {
                let n: f64 = input_str.parse().map_err(|_| {
                    anyhow::anyhow!("Expected a number for '{}', got '{}'", key, input_str)
                })?;
                Value::Number(serde_yml::Number::from(n as i64))
            }
            "bool" => {
                let b: bool = input_str.parse().map_err(|_| {
                    anyhow::anyhow!("Expected true/false for '{}', got '{}'", key, input_str)
                })?;
                Value::Bool(b)
            }
            _ => Value::String(input_str),
        };
        resolved.insert(key.clone(), val.clone());
        new_answers.insert(key.clone(), val);
    }

    // Save newly collected answers
    if !new_answers.is_empty() {
        save_answers(answers_file, app_name, &new_answers)?;
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_namespaced_answers_isolation() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        let mut answers_a = HashMap::new();
        answers_a.insert("key1".to_string(), Value::String("val_a".into()));
        save_answers(path, "app-a", &answers_a).unwrap();

        let mut answers_b = HashMap::new();
        answers_b.insert("key1".to_string(), Value::String("val_b".into()));
        save_answers(path, "app-b", &answers_b).unwrap();

        let loaded = load_answers(path).unwrap();
        assert_eq!(
            loaded.get("app-a").unwrap().get("key1").unwrap(),
            &Value::String("val_a".into())
        );
        assert_eq!(
            loaded.get("app-b").unwrap().get("key1").unwrap(),
            &Value::String("val_b".into())
        );
    }

    #[test]
    fn test_legacy_flat_migration() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        std::fs::write(path, "key1: value1\nkey2: value2\n").unwrap();

        let loaded = load_answers(path).unwrap();
        let default_ns = loaded.get("_default").unwrap();
        assert_eq!(
            default_ns.get("key1").unwrap(),
            &Value::String("value1".into())
        );
    }

    #[test]
    fn test_cleanup_stale_answers_noninteractive() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        let mut answers = HashMap::new();
        answers.insert("current".to_string(), Value::String("v".into()));
        answers.insert("stale_key".to_string(), Value::String("old".into()));
        save_answers(path, "my-app", &answers).unwrap();

        let current_keys: HashSet<String> = ["current".to_string()].into_iter().collect();
        // Non-interactive: should warn but not remove
        cleanup_stale_answers(path, "my-app", &current_keys, false).unwrap();

        let loaded = load_answers(path).unwrap();
        // Stale key still present (non-interactive doesn't remove)
        assert!(loaded.get("my-app").unwrap().contains_key("stale_key"));
    }
}
