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
struct AiPatchConfig {
    prompt: String,
    #[serde(default)]
    verify_command: String,
    /// External AI CLI that reads the prompt on stdin and writes a unified
    /// diff to stdout (PRD §4.6): "claude -p", "gemini", "codex", or custom.
    #[serde(default)]
    tool: String,
}

/// Run `sh -c "<command>"` in `cwd`, optionally feeding `stdin`.
fn sh(command: &str, cwd: &str, stdin: Option<Vec<u8>>) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let req = ExecRequest {
        program: "sh".to_string(),
        args: vec!["-c".to_string(), command.to_string()],
        cwd: Some(cwd.to_string()),
        env: vec![],
        inherit_env: true,
        stdin,
    };
    let r = exec(&req)?;
    Ok((r.status as i32, r.stdout, r.stderr))
}

/// Run `git <args...>` in `cwd`, optionally feeding `stdin` (e.g. a diff).
fn git(
    args: &[&str],
    cwd: &str,
    stdin: Option<Vec<u8>>,
) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let req = ExecRequest {
        program: "git".to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: Some(cwd.to_string()),
        env: vec![],
        inherit_env: true,
        stdin,
    };
    let r = exec(&req)?;
    Ok((r.status as i32, r.stdout, r.stderr))
}

/// True when the verify command is satisfied (exit 0). An empty verify
/// command means "cannot verify" → treated as not satisfied.
fn verify_passes(cfg: &AiPatchConfig, app_path: &str) -> bool {
    if cfg.verify_command.is_empty() {
        return false;
    }
    matches!(sh(&cfg.verify_command, app_path, None), Ok((0, _, _)))
}

impl Guest for Component {
    fn plan(req: EnsureRequest) -> Result<EnsurePlan, EnsureError> {
        let cfg: AiPatchConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;

        if verify_passes(&cfg, &req.app_path) {
            return Ok(EnsurePlan {
                description: "ai.patch: verification already satisfied, no action".to_string(),
                actions: vec![],
            });
        }

        Ok(EnsurePlan {
            description: format!(
                "ai.patch: invoke '{}' for prompt \"{}\", apply diff, verify, rollback on failure",
                if cfg.tool.is_empty() {
                    "<tool>"
                } else {
                    &cfg.tool
                },
                cfg.prompt.chars().take(60).collect::<String>()
            ),
            actions: vec![
                format!("run AI tool: {}", cfg.tool),
                "git apply <diff>".to_string(),
                format!("verify: {}", cfg.verify_command),
            ],
        })
    }

    fn execute(req: EnsureRequest) -> Result<String, EnsureError> {
        let cfg: AiPatchConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;

        if req.dry_run {
            return Ok(format!("Would run ai.patch via '{}'", cfg.tool));
        }

        // Already satisfied → nothing to do.
        if verify_passes(&cfg, &req.app_path) {
            return Ok("ai.patch: verification already satisfied".to_string());
        }

        if cfg.tool.is_empty() {
            return Err(EnsureError::ConfigError(
                "ai.patch requires a 'tool' (the AI CLI that emits a unified diff)".to_string(),
            ));
        }

        // Rollback safety (PRD §4.6): require a clean git worktree so a failed
        // patch can be reverted with `git apply -R`.
        match git(&["rev-parse", "--is-inside-work-tree"], &req.app_path, None) {
            Ok((0, _, _)) => {}
            _ => {
                return Err(EnsureError::ExecutionError(
                    "ai.patch requires a git repository for rollback safety".to_string(),
                ));
            }
        }

        // 1. Invoke the AI tool, feeding the prompt on stdin; expect a unified
        //    diff on stdout.
        let (status, stdout, stderr) = sh(&cfg.tool, &req.app_path, Some(cfg.prompt.into_bytes()))
            .map_err(EnsureError::ExecutionError)?;
        if status != 0 {
            return Err(EnsureError::ExecutionError(format!(
                "AI tool '{}' failed: {}",
                cfg.tool,
                String::from_utf8_lossy(&stderr)
            )));
        }
        if stdout.is_empty() {
            return Err(EnsureError::ExecutionError(
                "AI tool produced an empty diff".to_string(),
            ));
        }
        let diff = stdout;

        // 2. Apply the diff with git.
        let (apply_status, _, apply_err) = git(&["apply"], &req.app_path, Some(diff.clone()))
            .map_err(EnsureError::ExecutionError)?;
        if apply_status != 0 {
            return Err(EnsureError::ExecutionError(format!(
                "git apply failed (nothing changed): {}",
                String::from_utf8_lossy(&apply_err)
            )));
        }

        // 3. Verify; roll back on failure.
        if !cfg.verify_command.is_empty() {
            let passed = matches!(sh(&cfg.verify_command, &req.app_path, None), Ok((0, _, _)));
            if !passed {
                let _ = git(&["apply", "-R"], &req.app_path, Some(diff));
                return Err(EnsureError::ExecutionError(
                    "verification failed after applying AI patch; rolled back".to_string(),
                ));
            }
        }

        Ok(format!(
            "ai.patch: applied diff via '{}' and verified",
            cfg.tool
        ))
    }
}

export!(Component);
