use weaver_core::plugin::ensure_wasm::EnsurePluginEngine;
use std::path::Path;

/// Loads the ai-patch ensure plugin and exercises `plan` through the
/// `ensure-provider` world. Uses `verify_command: "true"` so the plugin
/// reports "already satisfied" deterministically — no AI tool or network
/// required. Proves the plugin loads and its verify-gated logic runs.
///
/// The component is an optional build artifact produced by
/// `cargo component build -p ai-patch-plugin --target wasm32-unknown-unknown`.
#[test]
fn test_ai_patch_plugin_plan_already_satisfied() {
    let engine = EnsurePluginEngine::new().expect("failed to create engine");

    let wasm_path = Path::new("../../target/wasm32-unknown-unknown/debug/ai_patch_plugin.wasm");
    if !wasm_path.exists() {
        eprintln!("skipping: ai-patch plugin WASM not built (run scripts/build-plugins.sh)");
        return;
    }

    let plugin = engine.load_plugin(wasm_path).expect("failed to load plugin");

    let config_json = r#"{"prompt": "add a license", "verify_command": "true", "tool": ""}"#;
    let plan = plugin
        .plan("/tmp", false, config_json)
        .expect("plan should succeed when verify passes");

    assert!(
        plan.description.contains("already satisfied"),
        "unexpected plan: {}",
        plan.description
    );
    assert!(plan.actions.is_empty(), "expected no actions when satisfied");
}
