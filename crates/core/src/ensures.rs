use crate::config::EnsureSpec;
use crate::json_merge::ensure_json_key;
use serde_json::Value;
use std::path::Path;

/// Apply a single app-level npm ensure to the workspace rooted at `app_root`.
///
/// Each `ensure.npm.*` variant converges `package.json` via
/// [`crate::json_merge::ensure_json_key`].
pub fn apply_ensure(app_root: &Path, ensure: &EnsureSpec) -> anyhow::Result<()> {
    match ensure {
        EnsureSpec::NpmScript { file, name, value } => ensure_json_key(
            &app_root.join(file),
            &["scripts"],
            name,
            Value::String(value.clone()),
        ),
        EnsureSpec::NpmDep {
            file,
            name,
            version,
        } => ensure_json_key(
            &app_root.join(file),
            &["dependencies"],
            name,
            Value::String(version.clone()),
        ),
        EnsureSpec::NpmDevDep {
            file,
            name,
            version,
        } => ensure_json_key(
            &app_root.join(file),
            &["devDependencies"],
            name,
            Value::String(version.clone()),
        ),
        EnsureSpec::NpmEngine {
            file,
            name,
            version,
        } => ensure_json_key(
            &app_root.join(file),
            &["engines"],
            name,
            Value::String(version.clone()),
        ),
        // Non-npm variants (ensure.file.*) are handled via the Ensure trait
        // dispatch (see ensure::build_app_ensure), not this native npm path.
        _ => anyhow::bail!(
            "apply_ensure only handles ensure.npm.* variants; {:?} must route via build_app_ensure",
            ensure
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_pkg(dir: &Path, body: &str) {
        fs::write(dir.join("package.json"), body).unwrap();
    }

    fn read_pkg(dir: &Path) -> String {
        fs::read_to_string(dir.join("package.json")).unwrap()
    }

    #[test]
    fn ensure_npm_script_appends_to_existing_scripts() {
        let dir = TempDir::new().unwrap();
        write_pkg(
            dir.path(),
            "{\n  \"name\": \"x\",\n  \"scripts\": {\n    \"start\": \"node .\"\n  }\n}\n",
        );

        apply_ensure(
            dir.path(),
            &EnsureSpec::NpmScript {
                file: "package.json".into(),
                name: "build".into(),
                value: "tsc".into(),
            },
        )
        .unwrap();

        let after = read_pkg(dir.path());
        assert!(after.contains("\"start\": \"node .\""));
        assert!(after.contains("\"build\": \"tsc\""));
        assert!(after.ends_with("\n"));
        // start must come before build (preserve_order).
        let start_idx = after.find("\"start\"").unwrap();
        let build_idx = after.find("\"build\"").unwrap();
        assert!(start_idx < build_idx);
    }

    #[test]
    fn ensure_npm_script_replaces_existing_key() {
        let dir = TempDir::new().unwrap();
        write_pkg(
            dir.path(),
            "{\n  \"scripts\": {\n    \"build\": \"old\"\n  }\n}\n",
        );

        apply_ensure(
            dir.path(),
            &EnsureSpec::NpmScript {
                file: "package.json".into(),
                name: "build".into(),
                value: "tsc -p .".into(),
            },
        )
        .unwrap();

        let after = read_pkg(dir.path());
        assert!(after.contains("\"build\": \"tsc -p .\""));
        assert!(!after.contains("\"old\""));
    }

    #[test]
    fn ensure_npm_engine_creates_engines_object() {
        let dir = TempDir::new().unwrap();
        write_pkg(dir.path(), "{\n  \"name\": \"x\"\n}\n");

        apply_ensure(
            dir.path(),
            &EnsureSpec::NpmEngine {
                file: "package.json".into(),
                name: "node".into(),
                version: ">=20 <21".into(),
            },
        )
        .unwrap();

        let after = read_pkg(dir.path());
        assert!(after.contains("\"engines\": {"));
        assert!(after.contains("\"node\": \">=20 <21\""));
    }

    #[test]
    fn ensure_npm_dep_and_devdep_are_separate() {
        let dir = TempDir::new().unwrap();
        write_pkg(dir.path(), "{}\n");

        apply_ensure(
            dir.path(),
            &EnsureSpec::NpmDep {
                file: "package.json".into(),
                name: "express".into(),
                version: "^4".into(),
            },
        )
        .unwrap();
        apply_ensure(
            dir.path(),
            &EnsureSpec::NpmDevDep {
                file: "package.json".into(),
                name: "typescript".into(),
                version: "^5".into(),
            },
        )
        .unwrap();

        let json: Value = serde_json::from_str(&read_pkg(dir.path())).unwrap();
        assert_eq!(json["dependencies"]["express"], "^4");
        assert_eq!(json["devDependencies"]["typescript"], "^5");
    }

    #[test]
    fn errors_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let err = apply_ensure(
            dir.path(),
            &EnsureSpec::NpmScript {
                file: "package.json".into(),
                name: "x".into(),
                value: "y".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("cannot read"));
    }

    #[test]
    fn errors_when_json_invalid() {
        let dir = TempDir::new().unwrap();
        write_pkg(dir.path(), "not json");
        let err = apply_ensure(
            dir.path(),
            &EnsureSpec::NpmScript {
                file: "package.json".into(),
                name: "x".into(),
                value: "y".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid JSON"));
    }
}
