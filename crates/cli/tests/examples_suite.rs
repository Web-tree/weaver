use assert_cmd::Command;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use walkdir::WalkDir;

const DEFAULT_ARGS: &[&str] = &["apply", "--auto-approve"];

#[derive(Debug, Deserialize, Default)]
struct SuiteConfig {
    #[serde(default)]
    defaults: ExampleSettings,
    #[serde(default)]
    examples: BTreeMap<String, ExampleSettings>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ExampleSettings {
    #[serde(default)]
    stage: Stage,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    ignore_paths: Vec<String>,
    #[serde(default)]
    custom_assertions: Vec<CustomAssertion>,
    /// When true, the example demonstrates a CLI failure (e.g. drift on
    /// `--strategy stop`). Non-zero exit becomes the success criterion and
    /// before/after parity is still asserted so we verify no side effects.
    #[serde(default)]
    expect_failure: bool,
    /// Optional precise exit-code assertion. When set, the CLI must exit
    /// with exactly this code (e.g. `2` for `plan --detailed-exitcode` on
    /// drift). Implies `expect_failure` for any non-zero value.
    #[serde(default)]
    expected_exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Stage {
    Implemented,
    #[default]
    Pending,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CustomAssertion {
    FileExists { path: String },
    FileContains { path: String, value: String },
    FileNonEmpty { path: String },
}

#[derive(Debug, Deserialize)]
struct WeaverConfig {
    #[serde(default)]
    modules: Vec<WeaverModule>,
}

#[derive(Debug, Deserialize)]
struct WeaverModule {
    source: String,
    #[serde(rename = "ref")]
    ref_name: String,
}

struct ExampleResult {
    name: String,
    stage: Stage,
    command: Vec<String>,
    ok: bool,
    details: Vec<String>,
}

#[test]
fn examples_apply_snapshot_suite() {
    let cfg = load_suite_config(repo_root().join("examples/test-suite.yaml"));
    let examples_root = repo_root().join("examples");

    let mut failures = Vec::new();
    let mut results = Vec::new();

    for example_dir in list_example_dirs(&examples_root) {
        let name = example_dir
            .file_name()
            .expect("example dir should have file name")
            .to_string_lossy()
            .to_string();

        let settings = merged_settings(&cfg, &name);
        let result = run_example(&example_dir, &settings);

        match (settings.stage, result.ok) {
            (Stage::Implemented, false) => {
                failures.push(format!(
                    "{name} [implemented regressed]: {}",
                    result.details.join("; ")
                ));
            }
            (Stage::Pending, true) => {
                failures.push(format!(
                    "{name} [pending but passes — promote to `stage: implemented`]"
                ));
            }
            _ => {}
        }
        results.push(result);
    }

    write_report(&results);

    if !failures.is_empty() {
        panic!(
            "Stage mismatches detected:\n{}",
            failures
                .into_iter()
                .map(|line| format!("- {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

fn run_example(example_dir: &Path, settings: &ExampleSettings) -> ExampleResult {
    let name = example_dir
        .file_name()
        .expect("example dir should have file name")
        .to_string_lossy()
        .to_string();

    let before_dir = example_dir.join("before");
    let after_dir = example_dir.join("after");

    if !before_dir.exists() || !after_dir.exists() {
        return ExampleResult {
            name,
            stage: settings.stage,
            command: command_args(settings),
            ok: false,
            details: vec!["missing before/after folders".to_string()],
        };
    }

    let workspace = TempDir::new().expect("failed to create temp dir");
    let example_workspace = workspace.path().join("example");
    copy_dir(example_dir, &example_workspace);

    let before_workspace = example_workspace.join("before");
    prepare_module_sources(&before_workspace);

    let command = command_args(settings);
    let output = Command::cargo_bin("wvr")
        .expect("weaver binary should build")
        .current_dir(&before_workspace)
        .env("HOME", workspace.path())
        .args(&command)
        .output()
        .expect("failed to execute wvr");

    let expect_failure = settings.expect_failure
        || settings
            .expected_exit_code
            .is_some_and(|code| code != 0);

    if output.status.success() == expect_failure {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if expect_failure {
            "cli command unexpectedly succeeded (expected failure)".to_string()
        } else {
            format!("cli command failed: {stderr}")
        };
        return ExampleResult {
            name,
            stage: settings.stage,
            command,
            ok: false,
            details: vec![detail],
        };
    }

    if let Some(expected) = settings.expected_exit_code {
        let actual = output.status.code().unwrap_or(-1);
        if actual != expected {
            return ExampleResult {
                name,
                stage: settings.stage,
                command,
                ok: false,
                details: vec![format!(
                    "exit code mismatch: expected {expected}, got {actual}"
                )],
            };
        }
    }

    let mut details = compare_dirs(&before_workspace, &after_dir, &settings.ignore_paths);

    for assertion in &settings.custom_assertions {
        if let Err(err) = apply_custom_assertion(&before_workspace, assertion) {
            details.push(err);
        }
    }

    ExampleResult {
        name,
        stage: settings.stage,
        command,
        ok: details.is_empty(),
        details,
    }
}

fn load_suite_config(path: PathBuf) -> SuiteConfig {
    if !path.exists() {
        return SuiteConfig::default();
    }
    let raw = fs::read_to_string(path).expect("failed to read examples suite config");
    serde_yml::from_str::<SuiteConfig>(&raw).expect("invalid examples suite config")
}

fn list_example_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = fs::read_dir(root)
        .expect("examples dir should exist")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .filter(|path| path.join("before").exists() || path.join("after").exists())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs
}

fn command_args(settings: &ExampleSettings) -> Vec<String> {
    if settings.args.is_empty() {
        DEFAULT_ARGS.iter().map(|v| (*v).to_string()).collect()
    } else {
        settings.args.clone()
    }
}

fn merged_settings(cfg: &SuiteConfig, name: &str) -> ExampleSettings {
    let mut merged = cfg.defaults.clone();
    if let Some(override_cfg) = cfg.examples.get(name) {
        if override_cfg.stage != Stage::Pending {
            merged.stage = override_cfg.stage;
        }
        if !override_cfg.args.is_empty() {
            merged.args = override_cfg.args.clone();
        }
        if !override_cfg.ignore_paths.is_empty() {
            merged.ignore_paths = override_cfg.ignore_paths.clone();
        }
        if !override_cfg.custom_assertions.is_empty() {
            merged.custom_assertions = override_cfg.custom_assertions.clone();
        }
        if override_cfg.expect_failure {
            merged.expect_failure = true;
        }
        if override_cfg.expected_exit_code.is_some() {
            merged.expected_exit_code = override_cfg.expected_exit_code;
        }
    }
    merged
}

fn compare_dirs(actual_root: &Path, expected_root: &Path, ignored: &[String]) -> Vec<String> {
    let ignored: BTreeSet<String> = ignored.iter().cloned().collect();
    let actual_files = collect_files(actual_root, &ignored);
    let expected_files = collect_files(expected_root, &ignored);

    let mut details = Vec::new();

    for path in expected_files.keys() {
        if !actual_files.contains_key(path) {
            details.push(format!("missing file: {path}"));
        }
    }

    for path in actual_files.keys() {
        if !expected_files.contains_key(path) {
            details.push(format!("unexpected file: {path}"));
        }
    }

    for (path, expected_content) in &expected_files {
        if let Some(actual_content) = actual_files.get(path) {
            if actual_content != expected_content {
                details.push(format!("content mismatch: {path}"));
            }
        }
    }

    details
}

fn collect_files(root: &Path, ignored: &BTreeSet<String>) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry
            .path()
            .strip_prefix(root)
            .expect("path should be relative")
            .to_string_lossy()
            .replace('\\', "/");

        if ignored.contains(&rel) {
            continue;
        }

        let bytes = fs::read(entry.path()).expect("failed to read file");
        files.insert(rel, bytes);
    }
    files
}

fn apply_custom_assertion(root: &Path, assertion: &CustomAssertion) -> Result<(), String> {
    match assertion {
        CustomAssertion::FileExists { path } => {
            if root.join(path).exists() {
                Ok(())
            } else {
                Err(format!("custom assertion failed: file missing ({path})"))
            }
        }
        CustomAssertion::FileContains { path, value } => {
            let content = fs::read_to_string(root.join(path))
                .map_err(|_| format!("custom assertion failed: cannot read ({path})"))?;
            if content.contains(value) {
                Ok(())
            } else {
                Err(format!(
                    "custom assertion failed: file does not contain expected value ({path})"
                ))
            }
        }
        CustomAssertion::FileNonEmpty { path } => {
            let metadata = fs::metadata(root.join(path))
                .map_err(|_| format!("custom assertion failed: cannot stat ({path})"))?;
            if metadata.len() > 0 {
                Ok(())
            } else {
                Err(format!("custom assertion failed: file is empty ({path})"))
            }
        }
    }
}


fn prepare_module_sources(before_workspace: &Path) {
    let config_path = before_workspace.join("weaver.yaml");
    if !config_path.exists() {
        return;
    }

    let raw = match fs::read_to_string(&config_path) {
        Ok(v) => v,
        Err(_) => return,
    };

    let config = match serde_yml::from_str::<WeaverConfig>(&raw) {
        Ok(v) => v,
        Err(_) => return,
    };

    for module in config.modules {
        let source = module.source;
        if source.contains("://") {
            continue;
        }

        let module_path = before_workspace.join(source);
        if !module_path.exists() {
            continue;
        }

        init_local_git_repo(&module_path, &module.ref_name);
    }
}

fn init_local_git_repo(repo_path: &Path, tag: &str) {
    let git_dir = repo_path.join(".git");
    if !git_dir.exists() {
        run_git(repo_path, &["init"]);
        run_git(repo_path, &["add", "."]);
        run_git_with_identity(repo_path, &["commit", "-m", "initial"]);
    }

    let has_tag = std::process::Command::new("git")
        .args(["tag", "--list", tag])
        .current_dir(repo_path)
        .output()
        .ok()
        .map(|out| !String::from_utf8_lossy(&out.stdout).trim().is_empty())
        .unwrap_or(false);

    if !has_tag {
        run_git_with_identity(repo_path, &["tag", tag]);
    }
}

fn run_git(repo_path: &Path, args: &[&str]) {
    let _ = std::process::Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output();
}

fn run_git_with_identity(repo_path: &Path, args: &[&str]) {
    let _ = std::process::Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .env("GIT_AUTHOR_NAME", "Example Test")
        .env("GIT_AUTHOR_EMAIL", "example@test.local")
        .env("GIT_COMMITTER_NAME", "Example Test")
        .env("GIT_COMMITTER_EMAIL", "example@test.local")
        .output();
}

fn write_report(results: &[ExampleResult]) {
    let mut markdown = String::new();
    markdown.push_str("# Example Compatibility Report\n\n");
    markdown.push_str("| Example | Stage | Command | Result | Notes |\n");
    markdown.push_str("|---|---|---|---|---|\n");

    for result in results {
        let stage = match result.stage {
            Stage::Implemented => "implemented",
            Stage::Pending => "pending",
        };
        let status = if result.ok { "✅ pass" } else { "❌ fail" };
        let notes = if result.details.is_empty() {
            "-".to_string()
        } else {
            result.details.join("; ")
        };
        markdown.push_str(&format!(
            "| {} | {} | `{}` | {} | {} |\n",
            result.name,
            stage,
            result.command.join(" "),
            status,
            notes
        ));
    }

    let report_path = repo_root().join("target/examples-compat-report.md");
    if let Some(parent) = report_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(report_path, markdown).expect("failed to write examples report");
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cli parent should exist")
        .parent()
        .expect("repo root should exist")
        .to_path_buf()
}

fn copy_dir(src: &Path, dst: &Path) {
    for entry in WalkDir::new(src).into_iter().filter_map(Result::ok) {
        let rel = entry.path().strip_prefix(src).expect("relative path");
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).expect("failed to create target dir");
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("failed to create target parent dir");
        }
        fs::copy(entry.path(), &target).expect("failed to copy file");
    }
}
