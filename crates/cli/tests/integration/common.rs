use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub fn cmd() -> Command {
    Command::cargo_bin("wvr").unwrap()
}

pub struct TestContext {
    pub root: PathBuf,
    pub temp: TempDir,
    // Keep temp alive so dir isn't deleted until drop
}

impl TestContext {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp.path().to_path_buf();
        Self { root, temp }
    }

    pub fn write_file(&self, path: &str, content: &str) {
        let p = self.root.join(path);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    pub fn read_file(&self, path: &str) -> String {
        fs::read_to_string(self.root.join(path)).unwrap()
    }

    pub fn setup_module(&self, name: &str, ref_: &str, content: &str) {
        // Mocking a remote git repo is hard in pure integration tests without a real git server.
        // For MVP tests, we can use "file://" scheme if supported, or just mock the cache directly
        // if we want to cheat, but `wvr apply` calls `git clone`.
        //
        // Actually, ModuleResolver supports `file://` if we pass a path.
        // To properly test "update", we need a "remote" repo.
        // Let's create a bare git repo in temp/remote/name

        let remote_path = self.root.join("remotes").join(name);
        fs::create_dir_all(&remote_path).unwrap();

        // Init remote repo
        std::process::Command::new("git")
            .args(&["init", "--bare"])
            .current_dir(&remote_path)
            .output()
            .unwrap();

        // Create a separate "source" dir to commit and push from
        let source_path = self.root.join("sources").join(name);
        fs::create_dir_all(&source_path).unwrap();

        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(&source_path)
            .output()
            .unwrap();

        // Write content
        let file_path = source_path.join("files/file.txt");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(file_path, content).unwrap();

        // Write manifest
        let manifest = r#"inputs: {}"#;
        fs::write(source_path.join("weaver.module.yaml"), manifest).unwrap();

        // Commit (Git user config might be needed in CI)
        let git_envs = [
            ("GIT_AUTHOR_NAME", "Test"),
            ("GIT_AUTHOR_EMAIL", "test@example.com"),
            ("GIT_COMMITTER_NAME", "Test"),
            ("GIT_COMMITTER_EMAIL", "test@example.com"),
        ];

        std::process::Command::new("git")
            .args(&["add", "."])
            .current_dir(&source_path)
            .envs(git_envs)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&source_path)
            .envs(git_envs)
            .output()
            .unwrap();

        // Tag it
        std::process::Command::new("git")
            .args(&["tag", ref_])
            .current_dir(&source_path)
            .envs(git_envs)
            .output()
            .unwrap();

        // Push to bare remote
        std::process::Command::new("git")
            .args(&["remote", "add", "origin", remote_path.to_str().unwrap()])
            .current_dir(&source_path)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(&["push", "origin", ref_])
            .current_dir(&source_path)
            .output()
            .unwrap();
    }

    pub fn setup_module_with_manifest(
        &self,
        name: &str,
        ref_: &str,
        content: &str,
        manifest: &str,
    ) {
        let remote_path = self.root.join("remotes").join(name);
        fs::create_dir_all(&remote_path).unwrap();

        // Init remote repo
        std::process::Command::new("git")
            .args(&["init", "--bare"])
            .current_dir(&remote_path)
            .output()
            .unwrap();

        // Create a separate "source" dir to commit and push from
        let source_path = self.root.join("sources").join(name);
        fs::create_dir_all(&source_path).unwrap();

        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(&source_path)
            .output()
            .expect("Failed to init git");

        // Ensure master branch (default usually, but explicit is good)
        // std::process::Command::new("git").args(&["checkout", "-b", "master"]).current_dir(&source_path).output().ok();
        // Assume master is default

        // Write content
        let file_path = source_path.join("files/file.txt");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(file_path, content).unwrap();

        // Write manifest
        fs::write(source_path.join("weaver.module.yaml"), manifest).unwrap();

        // Force branch name to master ensuring it exists
        std::process::Command::new("git")
            .args(&["symbolic-ref", "HEAD", "refs/heads/master"])
            .current_dir(&source_path)
            .output()
            .ok();

        // Commit (Git user config might be needed in CI)
        let git_envs = [
            ("GIT_AUTHOR_NAME", "Test"),
            ("GIT_AUTHOR_EMAIL", "test@example.com"),
            ("GIT_COMMITTER_NAME", "Test"),
            ("GIT_COMMITTER_EMAIL", "test@example.com"),
        ];

        let out = std::process::Command::new("git")
            .args(&["add", "."])
            .current_dir(&source_path)
            .envs(git_envs)
            .output()
            .expect("Failed to git add");
        assert!(out.status.success(), "git add failed");

        let out = std::process::Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&source_path)
            .envs(git_envs)
            .output()
            .expect("Failed to git commit");
        assert!(out.status.success(), "git commit failed");

        // Push to bare remote
        std::process::Command::new("git")
            .args(&["remote", "add", "origin", remote_path.to_str().unwrap()])
            .current_dir(&source_path)
            .output()
            .unwrap();

        let out = std::process::Command::new("git")
            .args(&["push", "origin", "master"])
            .current_dir(&source_path)
            .output()
            .expect("Failed to git push");
        if !out.status.success() {
            println!(
                "Git push failed: stdout: {}, stderr: {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            panic!("Git push failed");
        }
    }
}

pub fn weaver_config(module_name: &str, ref_: &str, root: &Path) -> String {
    let remote_path = root.join("remotes").join(module_name);
    let remote_url = format!("file://{}", remote_path.display());
    format!(
        r#"
version: "1"
modules:
  - name: "{module_name}"
    source: "{remote_url}"
    ref: "{ref_}"
apps:
  - name: "app"
    module: "{module_name}"
    path: "app"
    inputs: {{}}
"#
    )
}

pub fn module_config_v1() -> &'static str {
    "v1 content"
}
pub fn module_config_v2() -> &'static str {
    "v2 content"
}
