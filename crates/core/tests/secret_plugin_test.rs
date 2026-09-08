use weaver_core::plugin::wasm::WasmPluginEngine;
use std::path::Path;

/// Loads the aws-ssm secrets provider WASM component and calls `get-secret`
/// through the `provider` world. Proves the secrets plugin runtime works.
///
/// The aws-ssm component is an optional build artifact produced by
/// `cargo component build -p aws-ssm-provider --target wasm32-unknown-unknown`.
/// Skip rather than fail when absent so the default `cargo test` run does not
/// require the wasm toolchain.
#[test]
fn test_load_aws_ssm_provider() {
    let engine = WasmPluginEngine::new().expect("failed to create engine");

    let wasm_path = Path::new("../../target/wasm32-unknown-unknown/debug/aws_ssm_provider.wasm");
    if !wasm_path.exists() {
        eprintln!("skipping: aws-ssm provider WASM not built (run scripts/build-plugins.sh)");
        return;
    }

    let provider = engine.load_provider(wasm_path).expect("failed to load provider");

    // Calling get-secret will shell out to `aws`, which is typically absent in
    // CI. Either outcome proves the WASM component loaded and executed: a
    // success (aws present + param exists) or a provider error mentioning aws.
    match provider.get_secret("/some/test/key") {
        Ok(_) => {}
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("aws") || msg.contains("spawn") || msg.contains("provider error"),
                "unexpected error: {msg}"
            );
        }
    }
}
