use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Saved plan artifact produced by `wvr plan --out` and consumed by
/// `wvr apply --plan`. Inspired by Terraform plan-file workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFile {
    pub format_version: String,
    #[serde(default)]
    pub created_at: Option<String>,
    /// SHA-256 fingerprint of the workspace inputs at plan time. Apply
    /// recomputes this; mismatch means the workspace drifted since plan.
    pub workspace_fingerprint: String,
    /// Inputs captured at plan time, keyed by `<app-name>.<input-name>`.
    #[serde(default)]
    pub planned_inputs: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub changes: Vec<PlannedChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedChange {
    pub action: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<serde_json::Value>,
}

impl PlanFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = fs::read_to_string(path)?;
        let plan: Self = serde_json::from_str(&raw)?;
        Ok(plan)
    }

    /// Verify the plan against freshly resolved app inputs.
    ///
    /// `current_inputs` maps `<app>.<input>` to the value resolved from
    /// `weaver.yaml` + defaults. Returns `Err` listing mismatches when the
    /// plan is stale.
    pub fn verify_against_inputs(
        &self,
        current_inputs: &BTreeMap<String, serde_json::Value>,
    ) -> Result<(), StalePlan> {
        let mut diffs = Vec::new();

        for (key, planned) in &self.planned_inputs {
            match current_inputs.get(key) {
                Some(actual) if actual == planned => {}
                Some(actual) => diffs.push(InputDiff {
                    key: key.clone(),
                    planned: planned.clone(),
                    actual: Some(actual.clone()),
                }),
                None => diffs.push(InputDiff {
                    key: key.clone(),
                    planned: planned.clone(),
                    actual: None,
                }),
            }
        }

        if diffs.is_empty() {
            Ok(())
        } else {
            Err(StalePlan { diffs })
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputDiff {
    pub key: String,
    pub planned: serde_json::Value,
    pub actual: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct StalePlan {
    pub diffs: Vec<InputDiff>,
}

impl std::fmt::Display for StalePlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "stale plan: workspace inputs changed since plan was saved"
        )?;
        for diff in &self.diffs {
            match &diff.actual {
                Some(actual) => writeln!(
                    f,
                    "  - {}: planned={} actual={}",
                    diff.key, diff.planned, actual
                )?,
                None => writeln!(
                    f,
                    "  - {}: planned={} actual=<missing>",
                    diff.key, diff.planned
                )?,
            }
        }
        Ok(())
    }
}

impl std::error::Error for StalePlan {}

/// Render planned changes as a human-readable list. Glyph by action:
/// `+` create/add, `~` update/change, `-` remove, `?` drift, else ` `.
pub fn render_changes(changes: &[PlannedChange]) -> String {
    let mut out = String::new();
    for c in changes {
        let glyph = match c.action.as_str() {
            "create" | "add" => '+',
            "update" | "change" => '~',
            "remove" | "delete" => '-',
            "drift" => '?',
            _ => ' ',
        };
        out.push_str(&format!("{glyph} {} {}\n", c.action, c.path));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn fixture_plan() -> PlanFile {
        PlanFile {
            format_version: "1".into(),
            created_at: None,
            workspace_fingerprint: "sha256:abc".into(),
            planned_inputs: BTreeMap::from([
                ("org-policy.retention_days".into(), json!(30)),
                ("org-policy.name".into(), json!("policy")),
            ]),
            changes: vec![],
        }
    }

    #[test]
    fn load_parses_json() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("plan.json");
        fs::write(
            &p,
            r#"{
              "format_version": "1",
              "workspace_fingerprint": "sha256:x",
              "planned_inputs": {"app.k": 1},
              "changes": []
            }"#,
        )
        .unwrap();

        let plan = PlanFile::load(&p).unwrap();
        assert_eq!(plan.format_version, "1");
        assert_eq!(plan.workspace_fingerprint, "sha256:x");
        assert_eq!(plan.planned_inputs.get("app.k"), Some(&json!(1)));
    }

    #[test]
    fn verify_passes_when_inputs_match() {
        let plan = fixture_plan();
        let current = BTreeMap::from([
            ("org-policy.retention_days".into(), json!(30)),
            ("org-policy.name".into(), json!("policy")),
        ]);
        assert!(plan.verify_against_inputs(&current).is_ok());
    }

    #[test]
    fn verify_reports_value_mismatch() {
        let plan = fixture_plan();
        let current = BTreeMap::from([
            ("org-policy.retention_days".into(), json!(90)),
            ("org-policy.name".into(), json!("policy")),
        ]);
        let err = plan.verify_against_inputs(&current).unwrap_err();
        assert_eq!(err.diffs.len(), 1);
        assert_eq!(err.diffs[0].key, "org-policy.retention_days");
        assert_eq!(err.diffs[0].planned, json!(30));
        assert_eq!(err.diffs[0].actual, Some(json!(90)));
    }

    #[test]
    fn verify_reports_missing_input() {
        let plan = fixture_plan();
        let current = BTreeMap::from([("org-policy.retention_days".into(), json!(30))]);
        let err = plan.verify_against_inputs(&current).unwrap_err();
        assert_eq!(err.diffs.len(), 1);
        assert_eq!(err.diffs[0].key, "org-policy.name");
        assert!(err.diffs[0].actual.is_none());
    }

    #[test]
    fn verify_ignores_extra_current_inputs() {
        let plan = PlanFile {
            format_version: "1".into(),
            created_at: None,
            workspace_fingerprint: "f".into(),
            planned_inputs: BTreeMap::from([("a.k".into(), json!(1))]),
            changes: vec![],
        };
        let current = BTreeMap::from([
            ("a.k".into(), json!(1)),
            ("b.unrelated".into(), json!("x")),
        ]);
        assert!(plan.verify_against_inputs(&current).is_ok());
    }

    #[test]
    fn display_lists_all_diffs() {
        let plan = fixture_plan();
        let current = BTreeMap::from([
            ("org-policy.retention_days".into(), json!(90)),
            ("org-policy.name".into(), json!("renamed")),
        ]);
        let err = plan.verify_against_inputs(&current).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("stale plan"));
        assert!(msg.contains("retention_days"));
        assert!(msg.contains("planned=30"));
        assert!(msg.contains("actual=90"));
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[test]
    fn renders_change_lines_with_action_prefix() {
        let changes = vec![
            PlannedChange {
                action: "create".into(),
                path: "AGENTS.md#Skills".into(),
                preview: None,
            },
            PlannedChange {
                action: "update".into(),
                path: "package.json".into(),
                preview: None,
            },
            PlannedChange {
                action: "drift".into(),
                path: "app/settings.yaml".into(),
                preview: None,
            },
            PlannedChange {
                action: "remove".into(),
                path: "stale.txt".into(),
                preview: None,
            },
        ];
        let rendered = render_changes(&changes);
        assert!(rendered.contains("+ create AGENTS.md#Skills"));
        assert!(rendered.contains("~ update package.json"));
        assert!(rendered.contains("? drift app/settings.yaml"));
        assert!(rendered.contains("- remove stale.txt"));
    }
}
