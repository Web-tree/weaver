//! `go.dep` ensure — pins a Go module requirement via `go mod edit`
//! (deterministic go.mod edit, no network), per PRD "native tool first".

use serde::Deserialize;
use wit_bindgen::generate;

generate!({
    world: "ensure-provider",
    path: "../../wit",
});

use exports::weaver::plugin::ensures::{EnsureError, EnsurePlan, EnsureRequest, Guest};
use weaver::plugin::process::{exec, ExecRequest};

struct Component;

#[derive(Deserialize)]
struct GoDepConfig {
    module: String,
    version: String,
}

fn run(program: &str, args: &[String], cwd: &str) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let req = ExecRequest {
        program: program.to_string(),
        args: args.to_vec(),
        cwd: Some(cwd.to_string()),
        env: vec![],
        inherit_env: true,
        stdin: None,
    };
    let r = exec(&req)?;
    Ok((r.status as i32, r.stdout, r.stderr))
}

impl Guest for Component {
    fn plan(req: EnsureRequest) -> Result<EnsurePlan, EnsureError> {
        let cfg: GoDepConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;
        Ok(EnsurePlan {
            description: format!("Ensure go.mod requires {}@{}", cfg.module, cfg.version),
            actions: vec![format!(
                "go mod edit -require={}@{}",
                cfg.module, cfg.version
            )],
        })
    }

    fn execute(req: EnsureRequest) -> Result<String, EnsureError> {
        let cfg: GoDepConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;
        if req.dry_run {
            return Ok(format!("Would require {}@{}", cfg.module, cfg.version));
        }
        let require = format!("-require={}@{}", cfg.module, cfg.version);
        let (status, _, stderr) = run("go", &["mod".into(), "edit".into(), require], &req.app_path)
            .map_err(EnsureError::ExecutionError)?;
        if status != 0 {
            return Err(EnsureError::ExecutionError(format!(
                "go mod edit failed: {}",
                String::from_utf8_lossy(&stderr)
            )));
        }
        Ok(format!(
            "Ensured go.mod requires {}@{}",
            cfg.module, cfg.version
        ))
    }
}

export!(Component);
