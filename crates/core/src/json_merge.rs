use serde::Serialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// Set `root[parents[0]][parents[1]]...[key] = value` in a JSON file.
/// Intermediate objects are created in declaration order. Sibling keys and key
/// order (serde_json `preserve_order`) are preserved so the diff stays minimal.
/// Space-based indent width and trailing newline are preserved; tab-indented
/// files fall back to two-space indent. The file must already exist and contain
/// a JSON object.
pub fn ensure_json_key(
    path: &Path,
    parents: &[&str],
    key: &str,
    value: Value,
) -> anyhow::Result<()> {
    let raw = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    let mut root: Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("invalid JSON in {}: {e}", path.display()))?;

    let detected_indent = detect_indent(&raw);
    let trailing_newline = raw.ends_with('\n');

    let obj = root
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("{} is not a JSON object", path.display()))?;

    let target = navigate_or_create(obj, parents)?;
    target.insert(key.to_string(), value);

    write_pretty(path, &root, &detected_indent, trailing_newline)
}

fn navigate_or_create<'a>(
    root: &'a mut Map<String, Value>,
    path: &[&str],
) -> anyhow::Result<&'a mut Map<String, Value>> {
    let mut cur = root;
    for segment in path {
        let entry = cur
            .entry(segment.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            anyhow::bail!("expected object at `{segment}`, found non-object");
        }
        cur = entry
            .as_object_mut()
            .expect("just checked entry.is_object()");
    }
    Ok(cur)
}

fn detect_indent(raw: &str) -> String {
    for line in raw.lines().skip(1) {
        let count = line.chars().take_while(|c| *c == ' ').count();
        if count > 0 {
            return " ".repeat(count);
        }
    }
    "  ".to_string()
}

fn write_pretty(
    path: &Path,
    value: &Value,
    indent: &str,
    trailing_newline: bool,
) -> anyhow::Result<()> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser)?;
    if trailing_newline {
        buf.push(b'\n');
    }
    fs::write(path, buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sets_nested_key_preserving_siblings_and_indent() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("package.json");
        std::fs::write(
            &file,
            "{\n    \"name\": \"demo\",\n    \"scripts\": {\n        \"build\": \"tsc\"\n    }\n}\n",
        )
        .unwrap();

        ensure_json_key(
            &file,
            &["scripts"],
            "test",
            serde_json::Value::String("vitest".to_string()),
        )
        .unwrap();

        let out = std::fs::read_to_string(&file).unwrap();
        assert!(out.contains("\"build\": \"tsc\""));
        assert!(out.contains("\"test\": \"vitest\""));
        assert!(out.starts_with("{\n    \"name\""));
        assert!(out.ends_with("}\n"));
    }

    #[test]
    fn is_idempotent() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("package.json");
        std::fs::write(&file, "{\n  \"scripts\": {}\n}\n").unwrap();
        let val = serde_json::Value::String("x".into());
        ensure_json_key(&file, &["scripts"], "a", val.clone()).unwrap();
        let first = std::fs::read_to_string(&file).unwrap();
        ensure_json_key(&file, &["scripts"], "a", val).unwrap();
        let second = std::fs::read_to_string(&file).unwrap();
        assert_eq!(first, second);
    }
}
