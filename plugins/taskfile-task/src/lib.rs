//! `taskfile.task` ensure — verifies a named task is defined in the project's
//! Taskfile using the native `task --list-all`, per PRD "native tool first".
//!
//! Scaffold maturity: this checks presence and reports drift. Auto-inserting a
//! task into Taskfile.yml (YAML surgery) is intentionally left as future work;
//! a missing task is reported as an actionable error rather than silently
//! edited.

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
struct TaskfileTaskConfig {
    /// Name of the task that must exist in the Taskfile.
    name: String,
}

fn task_exists(name: &str, cwd: &str) -> Result<bool, String> {
    let req = ExecRequest {
        program: "task".to_string(),
        args: vec!["--list-all".to_string()],
        cwd: Some(cwd.to_string()),
        env: vec![],
        inherit_env: true,
        stdin: None,
    };
    let r = exec(&req)?;
    if r.status != 0 {
        return Err(format!(
            "`task --list-all` failed: {}",
            String::from_utf8_lossy(&r.stderr)
        ));
    }
    let listing = String::from_utf8_lossy(&r.stdout);
    // `task --list-all` prints "* <name>: ..." lines.
    Ok(listing.lines().any(|l| {
        l.trim_start_matches("* ")
            .starts_with(&format!("{}:", name))
    }))
}

impl Guest for Component {
    fn plan(req: EnsureRequest) -> Result<EnsurePlan, EnsureError> {
        let cfg: TaskfileTaskConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;
        let exists = task_exists(&cfg.name, &req.app_path).map_err(EnsureError::ExecutionError)?;
        if exists {
            Ok(EnsurePlan {
                description: format!("taskfile: task '{}' already defined", cfg.name),
                actions: vec![],
            })
        } else {
            Ok(EnsurePlan {
                description: format!("taskfile: task '{}' is missing", cfg.name),
                actions: vec![format!("define task '{}' in Taskfile.yml", cfg.name)],
            })
        }
    }

    fn execute(req: EnsureRequest) -> Result<String, EnsureError> {
        let cfg: TaskfileTaskConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;
        let exists = task_exists(&cfg.name, &req.app_path).map_err(EnsureError::ExecutionError)?;
        if exists {
            return Ok(format!("taskfile: task '{}' present", cfg.name));
        }
        if req.dry_run {
            return Ok(format!("Would require task '{}' in Taskfile.yml", cfg.name));
        }
        Err(EnsureError::NotApplicable(format!(
            "task '{}' is not defined in Taskfile.yml; add it manually (auto-insert not yet supported)",
            cfg.name
        )))
    }
}

export!(Component);
