//! # Rust Basic WASM Plugin Example
//!
//! This example demonstrates how to create a simple WASM plugin for Weaver
//! using Rust and the WASI Component Model.
//!
//! ## Building
//!
//! ```bash
//! cargo component build --release
//! ```
//!
//! ## Output
//!
//! The compiled component will be at `target/wasm32-wasip1/release/rust_basic_plugin.wasm`

use wit_bindgen::generate;

generate!({
    world: "provider",
    path: "../../../wit",
});

use exports::weaver::plugin::secrets::{Guest, SecretError, SecretRequest};

/// The main component struct that implements the plugin interface.
struct Component;

impl Guest for Component {
    /// Retrieves a secret value based on the request.
    ///
    /// This example implementation demonstrates the interface but returns
    /// a mock value. In a real plugin, this would:
    /// - Connect to a secrets backend (Vault, AWS SSM, etc.)
    /// - Handle authentication
    /// - Return the actual secret value
    ///
    /// # Arguments
    /// * `req` - The secret request containing the key to look up
    ///
    /// # Returns
    /// * `Ok(String)` - The secret value if found
    /// * `Err(SecretError)` - An error describing what went wrong
    fn get_secret(req: SecretRequest) -> Result<String, SecretError> {
        // Example: simple key-based lookup with mock values
        match req.key.as_str() {
            "example/api-key" => Ok("mock-api-key-12345".to_string()),
            "example/db-password" => Ok("mock-db-password".to_string()),
            key if key.starts_with("env/") => {
                // Demonstrate the not-found error variant
                Err(SecretError::NotFound(format!(
                    "Secret '{}' not found in example store",
                    key
                )))
            }
            _ => {
                // Demonstrate access denied for unknown paths
                Err(SecretError::AccessDenied(format!(
                    "Access denied to secret path: {}",
                    req.key
                )))
            }
        }
    }
}

export!(Component);
