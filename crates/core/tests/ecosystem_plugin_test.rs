use weaver_core::plugin::ensure_wasm::EnsurePluginEngine;
use std::path::Path;

/// Proves a generic (non-built-in) ecosystem plugin loads and runs `plan`
/// through the `ensure-provider` world. cargo-dep's plan is pure string
/// construction (no cargo invocation), so it is deterministic.
///
/// The component is an optional build artifact produced by
/// `cargo component build -p cargo-dep-plugin --target wasm32-unknown-unknown`.
#[test]
fn test_cargo_dep_plugin_plan() {
    let engine = EnsurePluginEngine::new().expect("failed to create engine");

    let wasm_path = Path::new("../../target/wasm32-unknown-unknown/debug/cargo_dep_plugin.wasm");
    if !wasm_path.exists() {
        eprintln!("skipping: cargo-dep plugin WASM not built (run scripts/build-plugins.sh)");
        return;
    }

    let plugin = engine.load_plugin(wasm_path).expect("failed to load plugin");

    let config_json = r#"{"type": "cargo.dep", "name": "serde", "version": "1.0"}"#;
    let plan = plugin
        .plan("/tmp", false, config_json)
        .expect("plan should succeed");

    assert!(
        plan.description.contains("serde@1.0"),
        "unexpected plan: {}",
        plan.description
    );
    assert!(
        plan.actions.iter().any(|a| a.contains("cargo add serde@1.0")),
        "expected cargo add action, got: {:?}",
        plan.actions
    );
}
