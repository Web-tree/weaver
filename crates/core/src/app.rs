use crate::config::{AppConfig, ModuleManifest};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct App {
    pub name: String,
    pub path: PathBuf,
    pub inputs: HashMap<String, serde_yml::Value>,
}

fn validate_input_type(
    key: &str,
    value: &serde_yml::Value,
    expected_type: &str,
) -> anyhow::Result<()> {
    // Compound types: list(<element-type>) — e.g. list(string)
    if let Some(elem_type) = parse_list_type(expected_type) {
        let Some(seq) = value.as_sequence() else {
            anyhow::bail!(
                "Input '{}': expected type '{}', got {:?}",
                key,
                expected_type,
                value
            );
        };
        for (idx, item) in seq.iter().enumerate() {
            validate_input_type(&format!("{key}[{idx}]"), item, elem_type)?;
        }
        return Ok(());
    }

    let ok = match expected_type {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "bool" => value.is_bool(),
        "list" => value.is_sequence(),
        "map" => value.is_mapping(),
        other => anyhow::bail!("Unknown input type '{}' for key '{}'", other, key),
    };
    if !ok {
        anyhow::bail!(
            "Input '{}': expected type '{}', got {:?}",
            key,
            expected_type,
            value
        );
    }
    Ok(())
}

/// Parse `list(<T>)` and return the inner element type, or `None` if not a list type.
fn parse_list_type(decl: &str) -> Option<&str> {
    let rest = decl.strip_prefix("list(")?;
    rest.strip_suffix(')').map(str::trim)
}

impl App {
    pub fn instantiate(config: &AppConfig, manifest: &ModuleManifest) -> anyhow::Result<Self> {
        let mut final_inputs = HashMap::new();

        for (key, def) in &manifest.inputs {
            let val = config.inputs.get(key).or(def.default.as_ref());

            if val.is_none() && def.required {
                anyhow::bail!(
                    "App '{}': Missing required input for module '{}': {}",
                    config.name,
                    config.module,
                    key
                );
            }

            if let Some(v) = val {
                validate_input_type(key, v, &def.r#type)?;
                final_inputs.insert(key.clone(), v.clone());
            }
        }

        Ok(Self {
            name: config.name.clone(),
            path: PathBuf::from(&config.path),
            inputs: final_inputs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yml::Value;

    #[test]
    fn test_validate_string_ok() {
        assert!(validate_input_type("k", &Value::String("hi".into()), "string").is_ok());
    }

    #[test]
    fn test_validate_number_ok() {
        assert!(validate_input_type("k", &Value::Number(42.into()), "number").is_ok());
    }

    #[test]
    fn test_validate_bool_ok() {
        assert!(validate_input_type("k", &Value::Bool(true), "bool").is_ok());
    }

    #[test]
    fn test_validate_type_mismatch() {
        let err = validate_input_type("port", &Value::String("not_a_number".into()), "number");
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains("expected type 'number'")
        );
    }

    #[test]
    fn test_validate_unknown_type() {
        let err = validate_input_type("k", &Value::String("x".into()), "object");
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("Unknown input type"));
    }

    #[test]
    fn test_validate_list_of_strings_ok() {
        let v = Value::Sequence(vec![Value::String("a".into()), Value::String("b".into())]);
        assert!(validate_input_type("ports", &v, "list(string)").is_ok());
    }

    #[test]
    fn test_validate_list_element_type_mismatch() {
        let v = Value::Sequence(vec![Value::String("a".into()), Value::Number(2.into())]);
        let err = validate_input_type("ports", &v, "list(string)").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ports[1]"), "msg was: {msg}");
        assert!(msg.contains("expected type 'string'"), "msg was: {msg}");
    }

    #[test]
    fn test_validate_list_value_not_a_sequence() {
        let v = Value::String("not-a-list".into());
        let err = validate_input_type("ports", &v, "list(string)").unwrap_err();
        assert!(err.to_string().contains("expected type 'list(string)'"));
    }

    #[test]
    fn test_validate_empty_list_ok() {
        let v = Value::Sequence(vec![]);
        assert!(validate_input_type("ports", &v, "list(string)").is_ok());
    }

    #[test]
    fn test_validate_bare_list_type_ok() {
        let v = Value::Sequence(vec![Value::Number(1.into()), Value::String("x".into())]);
        assert!(validate_input_type("items", &v, "list").is_ok());
    }

    #[test]
    fn test_parse_list_type_helper() {
        assert_eq!(parse_list_type("list(string)"), Some("string"));
        assert_eq!(parse_list_type("list(  number  )"), Some("number"));
        assert_eq!(parse_list_type("list"), None);
        assert_eq!(parse_list_type("string"), None);
    }
}
