use crate::common::{TestContext, cmd};
use std::path::Path;

/// `apply` on a workspace with no modules or apps succeeds and writes state.
///
/// Runs in a tempdir: `apply` rewrites `.rw/state.yaml`, so pointing it at a
/// committed fixture dirtied the working tree on every test run.
#[test]
fn test_bootstrap_empty_workspace() {
    let ctx = TestContext::new();
    ctx.write_file("weaver.yaml", "version: \"1.0\"\nmodules: []\napps: []\n");

    cmd()
        .arg("apply")
        .current_dir(&ctx.root)
        .assert()
        .success();

    assert!(ctx.root.join(".rw/state.yaml").exists());
}

/// R5 acceptance (docs/dev-standards-module-handoff.md): "apply, then plan;
/// exit code 0 and an empty change list." Exercises example 23, which mixes
/// a heading-selector `md_section` ensure, a block-marker `md_section`
/// ensure on the SAME file, and `from_template` ensures in one app.
///
/// This is also the regression test for two bugs:
/// - `upsert_heading` running a heading region to EOF and swallowing a
///   following `rw:section` block-marker region (crates/core/src/ensure/file.rs).
/// - the files/templates walk in `dry_run` reporting "create" unconditionally
///   instead of comparing actual bytes (crates/cli/src/commands/apply.rs).
#[test]
fn test_apply_then_plan_is_idempotent_for_heading_and_block_marker_sections() {
    let ctx = TestContext::new();
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/23-ai-agents-skills-and-agents-md");
    assert!(example.exists(), "fixture example missing: {}", example.display());

    // The module source must be a real git repo — ModuleResolver::resolve
    // always shells out to `git rev-parse`/`git clone`, even for local paths.
    let module_dir = ctx.root.join("module");
    std::process::Command::new("cp")
        .args(["-r", example.join("module").to_str().unwrap(), module_dir.to_str().unwrap()])
        .status()
        .expect("cp module failed");
    let git_envs = [
        ("GIT_AUTHOR_NAME", "Test"),
        ("GIT_AUTHOR_EMAIL", "test@example.com"),
        ("GIT_COMMITTER_NAME", "Test"),
        ("GIT_COMMITTER_EMAIL", "test@example.com"),
    ];
    std::process::Command::new("git").args(["init", "-q"]).current_dir(&module_dir).output().unwrap();
    std::process::Command::new("git").args(["add", "-A"]).current_dir(&module_dir).output().unwrap();
    std::process::Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(&module_dir)
        .envs(git_envs)
        .output()
        .unwrap();
    std::process::Command::new("git").args(["tag", "v1"]).current_dir(&module_dir).output().unwrap();

    // Copy `before/` (weaver.yaml + app/AGENTS.md) into the workspace root,
    // rewriting the module source to point at the git module copied above
    // (the fixture's own `../module` assumes a sibling `before/` layout).
    std::process::Command::new("cp")
        .args(["-r", example.join("before").join(".").to_str().unwrap(), ctx.root.to_str().unwrap()])
        .status()
        .expect("cp before failed");
    let weaver_yaml = ctx.read_file("weaver.yaml").replace("../module", "./module");
    ctx.write_file("weaver.yaml", &weaver_yaml);

    cmd().arg("apply").arg("--auto-approve").current_dir(&ctx.root).assert().success();

    let agents_md = ctx.read_file("app/AGENTS.md");
    assert!(
        agents_md.contains("rw:section id=\"recent-changes\""),
        "block-marker region destroyed by apply (heading region ate it):\n{agents_md}"
    );
    assert!(agents_md.contains("## Skills"), "heading-managed section missing after apply");
    assert!(
        agents_md.contains("## Manual Additions"),
        "hand-written content above the managed heading was destroyed"
    );

    let assert = cmd()
        .arg("plan")
        .arg("--detailed-exitcode")
        .current_dir(&ctx.root)
        .assert()
        .success(); // exit code 0

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let spurious_changes: Vec<&str> = stdout
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("+ ") || t.starts_with("~ ") || t.starts_with("- ") || t.starts_with("? ")
        })
        .collect();
    assert!(
        spurious_changes.is_empty(),
        "second plan after apply must report an empty change list, got:\n{spurious_changes:?}\nfull output:\n{stdout}"
    );
}
