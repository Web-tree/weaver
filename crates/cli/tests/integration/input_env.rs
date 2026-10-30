//! Module inputs exported to module-provided commands as `WVR_INPUT_<NAME>`
//! (#119).

use crate::common::{TestContext, cmd};
use crate::module_checks::make_module;
use predicates::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

const MANIFEST: &str = r#"
inputs:
  web_path:
    type: string
    default: "apps/web"
  strict-mode:
    type: bool
    default: false
  envs:
    type: "list(string)"
  team:
    type: string
  owner:
    type: string
checks:
  - id: web-path
    name: "string input"
    command: '[ "$WVR_INPUT_WEB_PATH" = apps/web ]'
  - id: strict
    name: "bool input"
    command: '[ "$WVR_INPUT_STRICT_MODE" = true ]'
  - id: envs
    name: "list input"
    command: '[ "$WVR_INPUT_ENVS" = "[\"dev\",\"prod\"]" ]'
  - id: team
    name: "input from saved answers"
    command: '[ "$WVR_INPUT_TEAM" = platform ]'
  - id: owner-unset
    name: "unset input is not exported"
    command: '[ -z "${WVR_INPUT_OWNER+x}" ]'
"#;

fn weaver_yaml(module_dir: &Path, strict: bool, envs: &str) -> String {
    format!(
        r#"
version: "1"
checks:
  - id: workspace-gets-none
    name: "workspace check"
    command: '[ -z "${{WVR_INPUT_WEB_PATH+x}}" ] && [ -z "${{WVR_INPUT_OWNER+x}}" ]'
modules:
  - name: "stdmod"
    source: "{}"
    ref: "v1"
apps:
  - name: "app"
    module: "stdmod"
    path: "app"
    inputs:
      strict-mode: {strict}
      envs: {envs}
    checks:
      - id: app-gets-inputs
        name: "app check"
        command: '[ "$WVR_INPUT_WEB_PATH" = apps/web ] && [ "$WVR_INPUT_STRICT_MODE" = {strict} ]'
"#,
        module_dir.display()
    )
}

fn statuses(ctx: &TestContext) -> BTreeMap<String, String> {
    let output = cmd()
        .args(["check", "--json"])
        .env("HOME", ctx.temp.path())
        .env("WVR_INPUT_WEB_PATH", "stale")
        .env("WVR_INPUT_OWNER", "stale")
        .current_dir(&ctx.root)
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "{err}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    report["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["qualified_id"].as_str().unwrap().to_string(),
                r["status"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn checks_read_resolved_module_inputs_from_wvr_input_env_vars() {
    let ctx = TestContext::new();
    let module_dir = make_module(&ctx.root, "stdmod", MANIFEST);
    ctx.write_file("app/.keep", "");
    ctx.write_file(".rw/answers.yaml", "app:\n  team: platform\n");

    ctx.write_file(
        "weaver.yaml",
        &weaver_yaml(&module_dir, true, "[dev, prod]"),
    );
    let pass = |id: &str| (id.to_string(), "pass".to_string());
    assert_eq!(
        statuses(&ctx),
        BTreeMap::from([
            pass("workspace-gets-none"),
            pass("app-gets-inputs"),
            pass("stdmod:web-path"),
            pass("stdmod:strict"),
            pass("stdmod:envs"),
            pass("stdmod:team"),
            pass("stdmod:owner-unset"),
        ])
    );

    ctx.write_file("weaver.yaml", &weaver_yaml(&module_dir, false, "[dev]"));
    let got = statuses(&ctx);
    assert_eq!(got["stdmod:strict"], "fail", "{got:?}");
    assert_eq!(got["stdmod:envs"], "fail", "{got:?}");
    assert_eq!(got["stdmod:web-path"], "pass", "{got:?}");
    assert_eq!(got["app-gets-inputs"], "pass", "{got:?}");
}

#[test]
fn check_commands_are_never_tera_rendered() {
    let ctx = TestContext::new();
    let module_dir = make_module(
        &ctx.root,
        "stdmod",
        r#"
inputs:
  web_path:
    type: string
    default: "apps/web"
checks:
  - id: literal
    name: "shell text that is also Tera syntax"
    command: "printf '%s' '${{ github.workflow }}-${{ github.ref }} {{ web_path }} ${#x}'"
    stdout_contains: "${{ github.workflow }}-${{ github.ref }} {{ web_path }} ${#x}"
"#,
    );
    ctx.write_file("app/.keep", "");
    ctx.write_file(
        "weaver.yaml",
        &format!(
            r#"
version: "1"
modules:
  - name: "stdmod"
    source: "{}"
    ref: "v1"
apps:
  - name: "app"
    module: "stdmod"
    path: "app"
"#,
            module_dir.display()
        ),
    );

    cmd()
        .arg("check")
        .env("HOME", ctx.temp.path())
        .current_dir(&ctx.root)
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn run_exports_module_inputs_to_the_task() {
    let ctx = TestContext::new();
    let module_dir = make_module(
        &ctx.root,
        "stdmod",
        r#"
inputs:
  region:
    type: string
  replicas:
    type: number
    default: 3
  owner:
    type: string
tasks:
  show:
    command: "sh show-inputs.sh"
"#,
    );
    ctx.write_file(
        "app/show-inputs.sh",
        r#"[ "$WVR_INPUT_REGION" = eu-west-1 ] && [ "$WVR_INPUT_REPLICAS" = 3 ] && [ -z "${WVR_INPUT_OWNER+x}" ]"#,
    );
    ctx.write_file(
        "weaver.yaml",
        &format!(
            r#"
version: "1"
modules:
  - name: "stdmod"
    source: "{}"
    ref: "v1"
apps:
  - name: "app"
    module: "stdmod"
    path: "app"
    inputs:
      region: eu-west-1
"#,
            module_dir.display()
        ),
    );

    cmd()
        .args(["run", "app", "show"])
        .env("HOME", ctx.temp.path())
        .env("WVR_INPUT_OWNER", "stale")
        .current_dir(&ctx.root)
        .assert()
        .success();
}
