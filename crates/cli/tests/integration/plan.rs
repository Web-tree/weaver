//! R24 acceptance: `wvr` exits with typed process codes
//! (docs/dev-standards-module-handoff.md R24; specs/004-generic-standardization-engine/spec.md
//! FR-005). These assert the *exact* exit code via `assert_cmd`'s `.code(n)`
//! -- `.success()`/`.failure()` alone (zero vs non-zero) wouldn't catch a
//! wrong non-zero code, e.g. `plan --detailed-exitcode` exiting 1 instead of
//! the documented 2.

use crate::common::{TestContext, weaver_config};
use assert_cmd::Command;

/// A converged workspace (`apply`, then `plan --detailed-exitcode`) exits 0.
#[test]
fn plan_detailed_exitcode_converged_exits_zero() {
    let ctx = TestContext::new();
    ctx.setup_module("my-mod", "v1", "file v1 content");
    ctx.write_file("weaver.yaml", &weaver_config("my-mod", "v1", &ctx.root));

    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("apply")
        .arg("--auto-approve")
        .assert()
        .success();

    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("plan")
        .arg("--detailed-exitcode")
        .assert()
        .code(0);
}

/// A workspace with an unapplied module (pending creates) exits 2 under
/// `--detailed-exitcode` -- Terraform convention, per R24's table.
#[test]
fn plan_detailed_exitcode_pending_changes_exits_two() {
    let ctx = TestContext::new();
    ctx.setup_module("my-mod", "v1", "file v1 content");
    ctx.write_file("weaver.yaml", &weaver_config("my-mod", "v1", &ctx.root));

    // Never applied: the module's `files/file.txt` has nothing to converge
    // against, so `plan` reports a pending create.
    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("plan")
        .arg("--detailed-exitcode")
        .assert()
        .code(2);
}

/// `plan` without `--detailed-exitcode` still exits 0 even with pending
/// changes -- only the flag opts into the Terraform-style signal.
#[test]
fn plan_without_detailed_exitcode_exits_zero_even_with_pending_changes() {
    let ctx = TestContext::new();
    ctx.setup_module("my-mod", "v1", "file v1 content");
    ctx.write_file("weaver.yaml", &weaver_config("my-mod", "v1", &ctx.root));

    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("plan")
        .assert()
        .code(0);
}

/// A missing `weaver.yaml` is a user error: exit 1, not 3 -- there's nothing
/// environmental about it, the user just needs to run `wvr init` (or `cd`
/// into the right directory).
#[test]
fn plan_missing_weaver_yaml_exits_one() {
    let ctx = TestContext::new();

    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("plan")
        .arg("--detailed-exitcode")
        .assert()
        .code(1);
}

/// Same as above via plain `apply` (no `--detailed-exitcode` involved) --
/// confirms the default/unclassified-error path still exits 1 post-R24.
#[test]
fn apply_missing_weaver_yaml_exits_one() {
    let ctx = TestContext::new();

    Command::cargo_bin("wvr")
        .unwrap()
        .current_dir(&ctx.root)
        .env("HOME", ctx.root.as_os_str())
        .arg("apply")
        .assert()
        .code(1);
}
