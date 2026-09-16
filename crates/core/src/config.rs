use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaverConfig {
    pub version: String,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub modules: Vec<ModuleConfig>,
    #[serde(default)]
    pub apps: Vec<AppConfig>,
    #[serde(default)]
    pub checks: Vec<CheckDef>,
    #[serde(default)]
    pub secrets: HashMap<String, SecretConfig>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
}

/// A config fragment loaded from includes or weaver.d/ — version is optional.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WeaverConfigFragment {
    #[serde(default)]
    pub modules: Vec<ModuleConfig>,
    #[serde(default)]
    pub apps: Vec<AppConfig>,
    #[serde(default)]
    pub checks: Vec<CheckDef>,
    #[serde(default)]
    pub secrets: HashMap<String, SecretConfig>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
}

/// Declarative plugin source. A plugin provides ensure types via a WASM
/// component. Exactly one of `git` or `path` must be set.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginConfig {
    #[serde(default)]
    pub git: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, rename = "ref")]
    pub git_ref: Option<String>,
}

impl PluginConfig {
    pub fn validate(&self, name: &str) -> anyhow::Result<()> {
        if self.git.is_some() && self.path.is_some() {
            anyhow::bail!(
                "Plugin '{}' cannot have both 'git' and 'path' properties. Please choose one.",
                name
            );
        }
        if self.git.is_none() && self.path.is_none() {
            anyhow::bail!("Plugin '{}' must have either 'git' or 'path' property.", name);
        }
        Ok(())
    }
}

impl WeaverConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_yml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_with_includes(path: &Path) -> anyhow::Result<Self> {
        let mut config = Self::load(path)?;
        let base_dir = path.parent().unwrap_or(Path::new("."));

        // Collect include paths from explicit includes field
        let mut include_paths = Vec::new();
        for pattern in &config.includes {
            let full_pattern = base_dir.join(pattern);
            let pattern_str = full_pattern.to_string_lossy().to_string();
            for entry in glob::glob(&pattern_str)? {
                let p = entry?;
                if p.is_file() {
                    include_paths.push(p);
                }
            }
        }

        // Auto-discover weaver.d/*.yaml
        let weaver_d = base_dir.join("weaver.d");
        if weaver_d.is_dir() {
            let pattern = weaver_d.join("*.yaml");
            let pattern_str = pattern.to_string_lossy().to_string();
            for entry in glob::glob(&pattern_str)? {
                let p = entry?;
                if p.is_file() && !include_paths.contains(&p) {
                    include_paths.push(p);
                }
            }
        }

        // Sort for deterministic ordering
        include_paths.sort();

        // Merge fragments
        for inc_path in &include_paths {
            let content = std::fs::read_to_string(inc_path)?;
            let fragment: WeaverConfigFragment = serde_yml::from_str(&content)?;
            config.merge_fragment(fragment);
        }

        config.validate_plugins()?;

        Ok(config)
    }

    fn merge_fragment(&mut self, fragment: WeaverConfigFragment) {
        // Arrays: concatenate
        self.modules.extend(fragment.modules);
        self.apps.extend(fragment.apps);
        self.checks.extend(fragment.checks);

        // Maps: later wins
        for (k, v) in fragment.secrets {
            self.secrets.insert(k, v);
        }
        for (k, v) in fragment.plugins {
            self.plugins.insert(k, v);
        }
    }

    fn validate_plugins(&self) -> anyhow::Result<()> {
        for (name, plugin) in &self.plugins {
            plugin.validate(name)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub name: String,
    pub source: String,
    pub r#ref: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub module: String,
    pub path: String,
    #[serde(default)]
    pub inputs: HashMap<String, serde_yml::Value>,
    #[serde(default)]
    pub ensures: Vec<EnsureSpec>,
    #[serde(default)]
    pub checks: Vec<CheckDef>,
}

/// Declarative convergence actions attached to an app. Variant names map to
/// the `type:` value in YAML (e.g. `ensure.npm.script`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EnsureSpec {
    #[serde(rename = "ensure.npm.script")]
    NpmScript {
        file: String,
        name: String,
        value: String,
    },
    #[serde(rename = "ensure.npm.dep")]
    NpmDep {
        file: String,
        name: String,
        version: String,
    },
    #[serde(rename = "ensure.npm.devDep")]
    NpmDevDep {
        file: String,
        name: String,
        version: String,
    },
    #[serde(rename = "ensure.npm.engine")]
    NpmEngine {
        file: String,
        name: String,
        version: String,
    },
    #[serde(rename = "ensure.file.exists")]
    FileExists {
        dest: String,
        #[serde(default)]
        template: Option<String>,
    },
    #[serde(rename = "ensure.file.from_template")]
    FileFromTemplate { template: String, dest: String },
    #[serde(rename = "ensure.file.md_section")]
    FileMdSection {
        file: String,
        selector: MdSelector,
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        content_from_template: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MdSelector {
    Heading {
        path: Vec<String>,
        #[serde(default = "default_heading_depth")]
        depth: usize,
    },
    BlockMarker {
        id: String,
    },
}

fn default_heading_depth() -> usize {
    2
}

/// A rule's severity (R23): only `Error` fails the gate (`wvr check`'s exit
/// code derives from severity), `Warn` and `Info` are reported but never
/// change the exit code. Without warn-level rules every new standard would be
/// a breaking CI change for every repo, so the standard stops being
/// adoptable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckDef {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Stable rule id (R3), independent of `name`, so waivers/history survive
    /// a rename. Falls back to `name` when unset (legacy configs).
    #[serde(default)]
    pub id: Option<String>,
    /// R23: `error` | `warn` | `info`, defaulting to `error` so legacy
    /// three-field checks (`name`/`command`/`description`) behave exactly as
    /// before.
    #[serde(default)]
    pub severity: Severity,
    /// Expected exit code (R20). Defaults to `0`, matching today's
    /// "any non-zero exit is a failure" behavior.
    #[serde(default)]
    pub expect: i32,
    /// Substring that must appear in stdout (R20). Evaluated against stdout
    /// with only its *trailing* newline(s) stripped (shell command
    /// substitution semantics, like `$(...)`) -- leading and interior bytes,
    /// including interior newlines, are left alone.
    #[serde(default)]
    pub stdout_contains: Option<String>,
    /// Regex that must match stdout (R20). An invalid pattern is an
    /// evaluation error (`Status::Error`), never a silent pass. Evaluated
    /// against stdout with only its *trailing* newline(s) stripped (like
    /// `stdout_contains`), so a natural end-anchored pattern (e.g.
    /// `^v\d+\.\d+\.\d+$`) matches output from commands like `echo`, which
    /// always emit a trailing newline. `$`/`^` still only anchor at the
    /// start/end of the whole (trimmed) string by default; opt into
    /// per-line anchoring with `(?m)` if stdout can span multiple lines.
    #[serde(default)]
    pub stdout_matches: Option<String>,
    /// Timeout in seconds (R20). No timeout by default.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Working directory, relative to the repo root (R20).
    #[serde(default)]
    pub cwd: Option<String>,
    /// Human-readable "how to fix this" string (spec 004 FR-024).
    #[serde(default)]
    pub remediate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretConfig {
    pub provider: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleManifest {
    #[serde(default)]
    pub inputs: HashMap<String, InputDef>,
    #[serde(default)]
    pub outputs: HashMap<String, String>,
    #[serde(default)]
    pub tasks: HashMap<String, TaskDef>,
    #[serde(default)]
    pub ensures: Vec<EnsureEntry>,
    /// Rules the module authors so every consuming repo shares one source of
    /// truth (R1, `docs/dev-standards-module-handoff.md`). Reuses `CheckDef`
    /// exactly -- a module-authored check and a repo-authored one are the
    /// same rule type, `wvr check` just runs them from different sources
    /// (see `CheckSource` in `crate::check`).
    #[serde(default)]
    pub checks: Vec<CheckDef>,
}

/// A single `ensures:` entry in a module manifest.
///
/// Built-in types deserialize into the typed [`EnsureConfig`]. Any other
/// `type:` is captured generically and dispatched to a WASM plugin named after
/// the type (e.g. `go.dep` -> `go-dep`), making the ensure surface extensible
/// without changing core.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnsureEntry {
    Known(EnsureConfig),
    Plugin(PluginEnsure),
}

/// A generic, plugin-backed ensure: a `type:` plus arbitrary config that is
/// forwarded verbatim (as JSON) to the resolved plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEnsure {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(flatten)]
    pub config: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EnsureConfig {
    #[serde(rename = "git.submodule")]
    GitSubmodule {
        url: String,
        path: String,
        r#ref: String,
    },
    #[serde(rename = "git.clone_pinned")]
    GitClonePinned {
        url: String,
        path: String,
        r#ref: String,
    },
    #[serde(rename = "npm.script")]
    NpmScript { name: String, command: String },
    #[serde(rename = "ai.patch")]
    AiPatch {
        prompt: String,
        #[serde(default)]
        verify_command: String,
        /// External AI CLI that reads the prompt on stdin and writes a unified
        /// diff to stdout (e.g. "claude -p", "gemini", "codex", or a custom
        /// command). Required to apply the patch (PRD §4.6).
        #[serde(default)]
        tool: String,
    },
}

impl ModuleManifest {
    /// Load a module manifest from disk.
    ///
    /// A missing `weaver.module.yaml` is treated as an empty manifest:
    /// modules that only ship static files under `files/` do not need
    /// to declare inputs, outputs, or tasks.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = serde_yml::from_str(&content)?;
        Ok(manifest)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDef {
    pub r#type: String,
    pub default: Option<serde_yml::Value>,
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDef {
    pub command: String,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn module_manifest_missing_file_yields_empty_manifest() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("weaver.module.yaml");
        assert!(!missing.exists());

        let manifest = ModuleManifest::load(&missing).expect("missing manifest should default");
        assert!(manifest.inputs.is_empty());
        assert!(manifest.outputs.is_empty());
        assert!(manifest.tasks.is_empty());
    }

    #[test]
    fn module_manifest_loads_when_present() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("weaver.module.yaml");
        fs::write(
            &path,
            r#"
inputs:
  region:
    type: string
    required: true
"#,
        )
        .unwrap();

        let manifest = ModuleManifest::load(&path).unwrap();
        assert_eq!(manifest.inputs.len(), 1);
        assert!(manifest.inputs.contains_key("region"));
    }

    /// A manifest with no `checks:` key still loads (backward compatibility)
    /// -- modules written before R1 must not fail to parse.
    #[test]
    fn module_manifest_without_checks_key_still_loads() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("weaver.module.yaml");
        fs::write(
            &path,
            r#"
inputs:
  region:
    type: string
    required: true
"#,
        )
        .unwrap();

        let manifest = ModuleManifest::load(&path).unwrap();
        assert!(manifest.checks.is_empty());
    }

    /// R1: a module manifest can declare `checks:`, reusing `CheckDef`
    /// exactly.
    #[test]
    fn module_manifest_loads_checks() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("weaver.module.yaml");
        fs::write(
            &path,
            r#"
inputs: {}
checks:
  - id: no-todo
    name: "No TODO markers"
    command: "! grep -r TODO ."
    severity: error
"#,
        )
        .unwrap();

        let manifest = ModuleManifest::load(&path).unwrap();
        assert_eq!(manifest.checks.len(), 1);
        assert_eq!(manifest.checks[0].id.as_deref(), Some("no-todo"));
        assert_eq!(manifest.checks[0].command, "! grep -r TODO .");
    }

    #[test]
    fn test_load_single_file_no_includes() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("weaver.yaml");
        fs::write(
            &config_path,
            r#"
version: "1.0"
modules:
  - name: base
    source: "https://example.com/mod"
    ref: "v1.0"
apps:
  - name: my-app
    module: base
    path: "."
"#,
        )
        .unwrap();

        let config = WeaverConfig::load_with_includes(&config_path).unwrap();
        assert_eq!(config.modules.len(), 1);
        assert_eq!(config.apps.len(), 1);
    }

    #[test]
    fn test_merge_modules_concatenate() {
        let dir = TempDir::new().unwrap();
        let weaver_d = dir.path().join("weaver.d");
        fs::create_dir(&weaver_d).unwrap();

        let config_path = dir.path().join("weaver.yaml");
        fs::write(
            &config_path,
            r#"
version: "1.0"
modules:
  - name: base
    source: "https://example.com/base"
    ref: "v1"
"#,
        )
        .unwrap();

        fs::write(
            weaver_d.join("extra.yaml"),
            r#"
modules:
  - name: extra
    source: "https://example.com/extra"
    ref: "v2"
apps:
  - name: extra-app
    module: extra
    path: "./extra"
"#,
        )
        .unwrap();

        let config = WeaverConfig::load_with_includes(&config_path).unwrap();
        assert_eq!(config.modules.len(), 2);
        assert_eq!(config.modules[0].name, "base");
        assert_eq!(config.modules[1].name, "extra");
        assert_eq!(config.apps.len(), 1);
        assert_eq!(config.apps[0].name, "extra-app");
    }

    #[test]
    fn test_merge_secrets_override() {
        let dir = TempDir::new().unwrap();
        let weaver_d = dir.path().join("weaver.d");
        fs::create_dir(&weaver_d).unwrap();

        let config_path = dir.path().join("weaver.yaml");
        fs::write(
            &config_path,
            r#"
version: "1.0"
secrets:
  db_password:
    provider: env
    key: DB_PASS
"#,
        )
        .unwrap();

        fs::write(
            weaver_d.join("secrets.yaml"),
            r#"
secrets:
  db_password:
    provider: aws-ssm
    key: /prod/db/password
"#,
        )
        .unwrap();

        let config = WeaverConfig::load_with_includes(&config_path).unwrap();
        let secret = config.secrets.get("db_password").unwrap();
        assert_eq!(secret.provider, "aws-ssm");
        assert_eq!(secret.key, "/prod/db/password");
    }

    #[test]
    fn test_include_glob_pattern() {
        let dir = TempDir::new().unwrap();
        let conf_dir = dir.path().join("conf");
        fs::create_dir(&conf_dir).unwrap();

        let config_path = dir.path().join("weaver.yaml");
        fs::write(
            &config_path,
            r#"
version: "1.0"
includes:
  - "conf/*.yaml"
"#,
        )
        .unwrap();

        fs::write(
            conf_dir.join("apps.yaml"),
            r#"
apps:
  - name: from-include
    module: base
    path: "./inc"
"#,
        )
        .unwrap();

        let config = WeaverConfig::load_with_includes(&config_path).unwrap();
        assert_eq!(config.apps.len(), 1);
        assert_eq!(config.apps[0].name, "from-include");
    }

    #[test]
    fn test_deterministic_ordering() {
        let dir = TempDir::new().unwrap();
        let weaver_d = dir.path().join("weaver.d");
        fs::create_dir(&weaver_d).unwrap();

        let config_path = dir.path().join("weaver.yaml");
        fs::write(&config_path, "version: \"1.0\"\n").unwrap();

        fs::write(
            weaver_d.join("b.yaml"),
            "apps:\n  - name: b-app\n    module: m\n    path: b\n",
        )
        .unwrap();
        fs::write(
            weaver_d.join("a.yaml"),
            "apps:\n  - name: a-app\n    module: m\n    path: a\n",
        )
        .unwrap();

        let config = WeaverConfig::load_with_includes(&config_path).unwrap();
        assert_eq!(config.apps.len(), 2);
        // a.yaml sorts before b.yaml
        assert_eq!(config.apps[0].name, "a-app");
        assert_eq!(config.apps[1].name, "b-app");
    }

    #[test]
    fn ensure_entry_known_type_parses_typed() {
        let yaml = "type: git.submodule\nurl: https://example.com/x.git\npath: vendor/x\nref: main\n";
        let entry: EnsureEntry = serde_yml::from_str(yaml).unwrap();
        match entry {
            EnsureEntry::Known(EnsureConfig::GitSubmodule { url, path, r#ref }) => {
                assert_eq!(url, "https://example.com/x.git");
                assert_eq!(path, "vendor/x");
                assert_eq!(r#ref, "main");
            }
            other => panic!("expected typed GitSubmodule, got {other:?}"),
        }
    }

    #[test]
    fn check_def_legacy_three_fields_default_severity_and_expect() {
        let yaml = "name: lint\ncommand: cargo clippy\ndescription: run clippy\n";
        let check: CheckDef = serde_yml::from_str(yaml).unwrap();
        assert_eq!(check.name, "lint");
        assert_eq!(check.command, "cargo clippy");
        assert_eq!(check.description.as_deref(), Some("run clippy"));
        assert_eq!(check.id, None);
        assert_eq!(check.severity, Severity::Error);
        assert_eq!(check.expect, 0);
        assert_eq!(check.stdout_contains, None);
        assert_eq!(check.stdout_matches, None);
        assert_eq!(check.timeout, None);
        assert_eq!(check.cwd, None);
        assert_eq!(check.remediate, None);
    }

    #[test]
    fn check_def_two_field_legacy_still_parses() {
        // Some existing fixtures omit `description` entirely.
        let yaml = "name: lint\ncommand: cargo clippy\n";
        let check: CheckDef = serde_yml::from_str(yaml).unwrap();
        assert_eq!(check.description, None);
        assert_eq!(check.severity, Severity::Error);
    }

    #[test]
    fn check_def_new_fields_parse() {
        let yaml = r#"
name: lint
id: rule.lint
command: cargo clippy
severity: warn
expect: 1
stdout_contains: "warning"
stdout_matches: "^warning:"
timeout: 30
cwd: "subdir"
remediate: "run `cargo clippy --fix`"
"#;
        let check: CheckDef = serde_yml::from_str(yaml).unwrap();
        assert_eq!(check.id.as_deref(), Some("rule.lint"));
        assert_eq!(check.severity, Severity::Warn);
        assert_eq!(check.expect, 1);
        assert_eq!(check.stdout_contains.as_deref(), Some("warning"));
        assert_eq!(check.stdout_matches.as_deref(), Some("^warning:"));
        assert_eq!(check.timeout, Some(30));
        assert_eq!(check.cwd.as_deref(), Some("subdir"));
        assert_eq!(check.remediate.as_deref(), Some("run `cargo clippy --fix`"));
    }

    #[test]
    fn ensure_entry_unknown_type_becomes_generic_plugin() {
        let yaml = "type: go.dep\nmodule: github.com/pkg/errors\nversion: v0.9.1\n";
        let entry: EnsureEntry = serde_yml::from_str(yaml).unwrap();
        match entry {
            EnsureEntry::Plugin(p) => {
                assert_eq!(p.type_name, "go.dep");
                // The non-`type` fields are captured verbatim for the plugin.
                assert_eq!(p.config.get("module").unwrap(), "github.com/pkg/errors");
                assert_eq!(p.config.get("version").unwrap(), "v0.9.1");
                assert!(!p.config.contains_key("type"), "type must not leak into config");
            }
            other => panic!("expected generic Plugin entry, got {other:?}"),
        }
    }
}
