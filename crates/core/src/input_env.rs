//! Module inputs exported to check and task commands as `WVR_INPUT_<NAME>` (#119).
//! Commands are never Tera-rendered: `${{ github.workflow }}` and `${#var}` are Tera syntax too.

use crate::config::ModuleManifest;
use std::collections::{BTreeMap, HashMap};

pub const PREFIX: &str = "WVR_INPUT_";

/// `WVR_INPUT_` + the input name upper-cased, with every character outside
/// `[A-Z0-9_]` replaced by `_` (one `_` per character).
pub fn env_name(input: &str) -> String {
    let name: String = input
        .chars()
        .map(|c| match c.to_ascii_uppercase() {
            c @ ('A'..='Z' | '0'..='9' | '_') => c,
            _ => '_',
        })
        .collect();
    format!("{PREFIX}{name}")
}

/// Strings as-is, booleans `true`/`false`, numbers in decimal, and
/// sequences, mappings and null as compact JSON.
pub fn encode(value: &serde_yml::Value) -> anyhow::Result<String> {
    use serde_yml::Value;
    Ok(match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => i.to_string(),
            (_, Some(u)) => u.to_string(),
            _ => n.as_f64().to_string(),
        },
        Value::Tagged(tagged) => encode(tagged.value())?,
        Value::Null | Value::Sequence(_) | Value::Mapping(_) => {
            crate::template::yaml_to_json(value)
                .map_err(anyhow::Error::msg)?
                .to_string()
        }
    })
}

/// Reject a manifest whose input names collide once mangled by [`env_name`]
/// (`web-path` and `web_path`), so one never silently overwrites the other.
pub fn ensure_unique_names<'a>(inputs: impl IntoIterator<Item = &'a str>) -> anyhow::Result<()> {
    let mut sorted: Vec<&str> = inputs.into_iter().collect();
    sorted.sort_unstable();
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for input in sorted {
        let name = env_name(input);
        if let Some(other) = seen.get(&name) {
            anyhow::bail!(
                "module inputs '{other}' and '{input}' both map to environment variable {name}; rename one"
            );
        }
        seen.insert(name, input);
    }
    Ok(())
}

/// Each manifest input set in `provided`, else its default; an input with neither is not
/// exported, as templates have no variable for it.
pub fn resolve(
    manifest: &ModuleManifest,
    provided: &HashMap<String, serde_yml::Value>,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut env = BTreeMap::new();
    for (key, def) in &manifest.inputs {
        if let Some(value) = provided.get(key).or(def.default.as_ref()) {
            let encoded = encode(value).map_err(|e| e.context(format!("input '{key}'")))?;
            env.insert(env_name(key), encoded);
        }
    }
    Ok(env)
}

/// Add `env` to `command`'s environment after removing every `WVR_INPUT_*`
/// it would inherit, so a stale variable never stands in for an unset input.
pub fn apply(command: &mut std::process::Command, env: &BTreeMap<String, String>) {
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with(PREFIX) {
            command.env_remove(key);
        }
    }
    command.envs(env);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InputDef;
    use serde_yml::Value;

    fn yaml(src: &str) -> Value {
        serde_yml::from_str(src).unwrap()
    }

    #[test]
    fn env_name_upper_cases_and_prefixes() {
        assert_eq!(env_name("web_path"), "WVR_INPUT_WEB_PATH");
        assert_eq!(env_name("Region2"), "WVR_INPUT_REGION2");
    }

    #[test]
    fn env_name_replaces_each_disallowed_character_with_an_underscore() {
        assert_eq!(env_name("web-path"), "WVR_INPUT_WEB_PATH");
        assert_eq!(env_name("a.b c"), "WVR_INPUT_A_B_C");
        assert_eq!(env_name("café"), "WVR_INPUT_CAF_");
        assert_eq!(env_name("x--y"), "WVR_INPUT_X__Y");
    }

    #[test]
    fn encode_strings_as_is() {
        assert_eq!(encode(&yaml("apps/web")).unwrap(), "apps/web");
        assert_eq!(
            encode(&yaml("'with \"quotes\" and $dollar'")).unwrap(),
            "with \"quotes\" and $dollar"
        );
        assert_eq!(encode(&yaml("''")).unwrap(), "");
    }

    #[test]
    fn encode_booleans_as_true_false() {
        assert_eq!(encode(&yaml("true")).unwrap(), "true");
        assert_eq!(encode(&yaml("false")).unwrap(), "false");
    }

    #[test]
    fn encode_numbers_in_decimal() {
        assert_eq!(encode(&yaml("3000")).unwrap(), "3000");
        assert_eq!(encode(&yaml("-7")).unwrap(), "-7");
        assert_eq!(encode(&yaml("1.5")).unwrap(), "1.5");
    }

    #[test]
    fn encode_lists_maps_and_null_as_compact_json() {
        assert_eq!(encode(&yaml("[a, b]")).unwrap(), r#"["a","b"]"#);
        assert_eq!(encode(&yaml("[]")).unwrap(), "[]");
        assert_eq!(
            encode(&yaml("{port: 80, tls: true}")).unwrap(),
            r#"{"port":80,"tls":true}"#
        );
        assert_eq!(encode(&yaml("null")).unwrap(), "null");
    }

    #[test]
    fn colliding_input_names_are_rejected_naming_both() {
        let err = ensure_unique_names(["web_path", "region", "web-path"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("'web-path'"), "{msg}");
        assert!(msg.contains("'web_path'"), "{msg}");
        assert!(msg.contains("WVR_INPUT_WEB_PATH"), "{msg}");
    }

    #[test]
    fn distinct_input_names_are_accepted() {
        assert!(ensure_unique_names(["web_path", "api_path", "Region"]).is_ok());
    }

    fn input(default: Option<&str>) -> InputDef {
        InputDef {
            r#type: "string".to_string(),
            default: default.map(yaml),
            description: None,
            required: false,
        }
    }

    #[test]
    fn resolve_prefers_provided_values_then_defaults_and_skips_unset() {
        let mut manifest = ModuleManifest::default();
        manifest
            .inputs
            .insert("web_path".into(), input(Some("apps/web")));
        manifest
            .inputs
            .insert("region".into(), input(Some("us-east-1")));
        manifest.inputs.insert("owner".into(), input(None));
        let provided = HashMap::from([
            ("region".to_string(), yaml("eu-west-1")),
            ("undeclared".to_string(), yaml("ignored")),
        ]);

        let env = resolve(&manifest, &provided).unwrap();

        assert_eq!(
            env,
            BTreeMap::from([
                ("WVR_INPUT_REGION".to_string(), "eu-west-1".to_string()),
                ("WVR_INPUT_WEB_PATH".to_string(), "apps/web".to_string()),
            ])
        );
    }
}
