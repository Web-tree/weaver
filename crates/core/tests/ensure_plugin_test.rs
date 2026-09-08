use weaver_core::plugin::ensure_wasm::EnsurePluginEngine;
use std::path::Path;

#[test]
fn test_load_npm_script_plugin() {
    let engine = EnsurePluginEngine::new().expect("Failed to create engine");

    // Path to the built WASM component (wasm32-unknown-unknown target)
    let wasm_path = Path::new("../../target/wasm32-unknown-unknown/debug/npm_script_plugin.wasm");

    if !wasm_path.exists() {
        // The plugin WASM is an optional build artifact produced by
        // `cargo component build -p npm-script-plugin --target wasm32-unknown-unknown`
        // (see scripts/build-plugins.sh). Skip rather than fail when it is
        // absent so the default `cargo test` run does not require the wasm
        // toolchain; the assertions below run whenever the artifact exists.
        eprintln!("skipping: npm-script plugin WASM not built (run scripts/build-plugins.sh)");
        return;
    }

    // Test that plugin loads successfully
    let plugin = engine
        .load_plugin(wasm_path)
        .expect("Failed to load plugin");

    // Test plan with sample config - this will try to run npm which may not exist
    // in the test environment. We just verify the plugin infrastructure works.
    let config_json = r#"{"name": "test", "command": "echo hello"}"#;
    let result = plugin.plan("/tmp/test-app", false, config_json);

    // The result may fail because npm is not installed in the test environment,
    // but that's okay - we're testing that the WASM plugin loads and executes.
    // A failure with "Failed to spawn npm" means the plugin loaded correctly.
    match result {
        Ok(plan) => {
            // Plugin executed successfully
            assert!(plan.description.contains("npm script"));
        }
        Err(e) => {
            let err_str = e.to_string();
            // This is expected if npm is not installed
            assert!(
                err_str.contains("npm") || err_str.contains("spawn"),
                "Unexpected error: {}",
                err_str
            );
        }
    }
}
