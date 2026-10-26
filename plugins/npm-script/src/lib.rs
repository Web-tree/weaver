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
struct NpmScriptConfig {
    name: String,
    command: String,
}

impl Guest for Component {
    fn plan(req: EnsureRequest) -> Result<EnsurePlan, EnsureError> {
        let config: NpmScriptConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;

        // Check if script exists and matches
        let check_args = vec![
            "pkg".to_string(),
            "get".to_string(),
            format!("scripts.{}", config.name),
        ];

        let exec_req = ExecRequest {
            program: "npm".to_string(),
            args: check_args,
            cwd: Some(req.app_path.clone()),
            env: vec![],
            inherit_env: true,
            stdin: None,
        };

        let result = exec(&exec_req).map_err(|e| EnsureError::ExecutionError(e))?;
        let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();

        if stdout == config.command {
            Ok(EnsurePlan {
                description: format!("npm script '{}' already set", config.name),
                actions: vec![],
            })
        } else {
            Ok(EnsurePlan {
                description: format!("Set npm script '{}' to '{}'", config.name, config.command),
                actions: vec![format!(
                    "npm pkg set scripts.{}='{}'",
                    config.name, config.command
                )],
            })
        }
    }

    fn execute(req: EnsureRequest) -> Result<String, EnsureError> {
        let config: NpmScriptConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;

        if req.dry_run {
            return Ok(format!(
                "Would set npm script '{}' to '{}'",
                config.name, config.command
            ));
        }

        let set_args = vec![
            "pkg".to_string(),
            "set".to_string(),
            format!("scripts.{}={}", config.name, config.command),
        ];

        let exec_req = ExecRequest {
            program: "npm".to_string(),
            args: set_args,
            cwd: Some(req.app_path),
            env: vec![],
            inherit_env: true,
            stdin: None,
        };

        let result = exec(&exec_req).map_err(|e| EnsureError::ExecutionError(e))?;

        if result.status != 0 {
            let stderr = String::from_utf8_lossy(&result.stderr);
            return Err(EnsureError::ExecutionError(format!(
                "npm pkg set failed: {}",
                stderr
            )));
        }

        Ok(format!("Ensured npm script '{}'", config.name))
    }
}

export!(Component);
