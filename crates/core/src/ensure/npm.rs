use crate::config::EnsureSpec;
use crate::ensure::{Ensure, EnsureContext, EnsurePlan};
use anyhow::Result;

/// Native `npm.script` ensure for module manifests.
///
/// package.json is plain JSON, which Weaver may edit directly (PRD §2:
/// "Weaver may parse only ... JSON"). Editing it natively is deterministic
/// and needs no `npm` on PATH, so the built-in npm.script shares the same
/// implementation as the app-level `ensure.npm.script` (see [`crate::ensures`]).
///
/// The `npm-script` WASM plugin remains as the reference example of the plugin
/// SDK (it shells `npm pkg set`); it is not the built-in handler.
pub struct EnsureNpmScript {
    pub name: String,
    pub command: String,
}

impl Ensure for EnsureNpmScript {
    fn plan(&self, _ctx: &EnsureContext) -> Result<EnsurePlan> {
        Ok(EnsurePlan {
            description: format!("Ensure npm script '{}' = '{}'", self.name, self.command),
            actions: vec![format!("set scripts.{} in package.json", self.name)],
        })
    }

    fn execute(&self, ctx: &EnsureContext) -> Result<()> {
        if ctx.dry_run {
            return Ok(());
        }
        let spec = EnsureSpec::NpmScript {
            file: "package.json".to_string(),
            name: self.name.clone(),
            value: self.command.clone(),
        };
        crate::ensures::apply_ensure(&ctx.app_path, &spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn module_npm_script_sets_package_json_natively() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"app","scripts":{"start":"node ."}}"#,
        )
        .unwrap();

        let ensure = EnsureNpmScript {
            name: "test".to_string(),
            command: "vitest run".to_string(),
        };
        let ctx = EnsureContext {
            app_path: dir.path().to_path_buf(),
            dry_run: false,
            module_path: PathBuf::from("."),
            tera_context: tera::Context::new(),
        };

        // Plan describes the change without mutating.
        let plan = ensure.plan(&ctx).unwrap();
        assert!(plan.description.contains("test"));

        // Execute performs the deterministic JSON edit — no npm on PATH needed.
        ensure.execute(&ctx).unwrap();

        let written = fs::read_to_string(dir.path().join("package.json")).unwrap();
        assert!(written.contains("\"test\""));
        assert!(written.contains("vitest run"));
        // Existing scripts are preserved.
        assert!(written.contains("\"start\""));
    }

    #[test]
    fn module_npm_script_dry_run_is_noop() {
        let dir = TempDir::new().unwrap();
        let original = r#"{"name":"app","scripts":{}}"#;
        fs::write(dir.path().join("package.json"), original).unwrap();

        let ensure = EnsureNpmScript {
            name: "build".to_string(),
            command: "tsc".to_string(),
        };
        let ctx = EnsureContext {
            app_path: dir.path().to_path_buf(),
            dry_run: true,
            module_path: PathBuf::from("."),
            tera_context: tera::Context::new(),
        };
        ensure.execute(&ctx).unwrap();

        let after = fs::read_to_string(dir.path().join("package.json")).unwrap();
        assert_eq!(after, original, "dry-run must not modify package.json");
    }
}
