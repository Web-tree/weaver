use crate::common::{TestContext, cmd};
use predicates::prelude::*;

#[test]
fn test_check_pass() {
    let ctx = TestContext::new();

    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "check1"
        command: "echo 'ok'"
        description: "Always passes"
"#,
    );

    let mut cmd = cmd();
    let assert = cmd.arg("check").current_dir(&ctx.root).assert();

    assert
        .success()
        .stdout(predicate::str::contains("PASS"))
        .stdout(predicate::str::contains("check1"));
}

#[test]
fn test_check_fail() {
    let ctx = TestContext::new();

    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "check1"
        command: "exit 1"
        description: "Always fails"
"#,
    );

    let mut cmd = cmd();
    let assert = cmd.arg("check").current_dir(&ctx.root).assert();

    assert
        .failure()
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("check1"));
}

#[test]
fn test_check_global_checks() {
    let ctx = TestContext::new();

    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
checks:
  - name: "global_check"
    command: "echo 'global ok'"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
"#,
    );

    let mut cmd = cmd();
    let assert = cmd.arg("check").current_dir(&ctx.root).assert();

    assert
        .success()
        .stdout(predicate::str::contains("PASS"))
        .stdout(predicate::str::contains("global_check"));
}

#[test]
fn test_check_filter() {
    let ctx = TestContext::new();

    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "check1"
        command: "echo 'ok'"
  - name: "app2"
    module: "mod1"
    path: "app2"
    checks:
      - name: "check2"
        command: "echo 'ok'"
"#,
    );

    let mut cmd = cmd();
    let assert = cmd.arg("check").arg("app1").current_dir(&ctx.root).assert();

    assert
        .success()
        .stdout(predicate::str::contains("check1"))
        .stdout(predicate::str::contains("check2").not());
}

#[test]
fn test_check_error_severity_failure_exits_2() {
    let ctx = TestContext::new();

    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "check1"
        command: "exit 1"
        severity: error
"#,
    );

    let mut cmd = cmd();
    let assert = cmd.arg("check").current_dir(&ctx.root).assert();

    assert.code(2).stdout(predicate::str::contains("FAIL")).stdout(predicate::str::contains("check1"));
}

#[test]
fn test_check_legacy_three_field_failure_also_exits_2() {
    // A legacy check (no `severity:` key) defaults to `error`, so it still
    // fails the gate -- just now with the typed exit code 2 instead of 1.
    let ctx = TestContext::new();

    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "check1"
        command: "exit 1"
        description: "Always fails"
"#,
    );

    let mut cmd = cmd();
    let assert = cmd.arg("check").current_dir(&ctx.root).assert();

    assert.code(2);
}

#[test]
fn test_check_warn_severity_failure_exits_0_but_is_reported() {
    let ctx = TestContext::new();

    ctx.write_file(
        "weaver.yaml",
        r#"
version: "1"
apps:
  - name: "app1"
    module: "mod1"
    path: "app1"
    checks:
      - name: "check1"
        command: "exit 1"
        severity: warn
"#,
    );

    let mut cmd = cmd();
    let assert = cmd.arg("check").current_dir(&ctx.root).assert();

    assert
        .code(0)
        .stdout(predicate::str::contains("FAIL"))
        .stdout(predicate::str::contains("check1"));
}

#[test]
fn test_check_no_checks() {
    let ctx = TestContext::new();
    ctx.write_file("weaver.yaml", "version: '1'\napps: []");

    let mut cmd = cmd();
    let assert = cmd.arg("check").current_dir(&ctx.root).assert();

    assert
        .success() // Should be success per spec? Or failure? T062 says "Handle... gracefully". Usually clean exit if nothing to do.
        // Wait, "No checks defined" message. CLI contract usually implies success if no errors found (even if no checks).
        // But let's check tasks.md: "T062... Handle 'No checks defined' message gracefully".
        .stdout(predicate::str::contains("No checks defined"));
}
