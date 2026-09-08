# Rust Basic WASM Plugin Example

This example demonstrates how to create a simple WASM plugin for `Weaver` using Rust and the WASI Component Model.

## Overview

The plugin implements the `weaver:plugin/secrets` interface, providing a mock secrets provider that demonstrates:

- The `get-secret` function signature
- Error handling with `SecretError` variants (`NotFound`, `AccessDenied`, `Internal`)
- WIT-to-Rust code generation via `wit-bindgen`

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.92+
- [cargo-component](https://github.com/bytecodealliance/cargo-component):

  ```bash
  cargo install cargo-component
  ```

## Building

```bash
cd examples/wasm/rust-basic
cargo component build --release
```

The compiled WASM component will be output to:

```text
target/wasm32-wasip1/release/rust_basic_plugin.wasm
```

## Interface

This plugin implements the `secrets` interface defined in [`wit/plugin.wit`](../../../wit/plugin.wit):

```wit
interface secrets {
    record secret-request {
        key: string,
    }

    variant secret-error {
        access-denied(string),
        not-found(string),
        internal(string),
    }

    get-secret: func(req: secret-request) -> result<string, secret-error>;
}
```

## Usage

Once compiled, the `.wasm` file can be loaded by the `Weaver` host runtime for secret resolution.

## Mock Behavior

This example returns mock values for demonstration:

| Key Pattern           | Result                       |
|-----------------------|------------------------------|
| `example/api-key`     | Returns mock API key         |
| `example/db-password` | Returns mock password        |
| `env/*`               | Returns `NotFound` error     |
| Other                 | Returns `AccessDenied` error |

## Creating a Real Plugin

To create a production plugin:

1. Copy this example as a starting point
2. Replace the mock implementation with actual secrets backend calls
3. Use the `process.exec` interface if you need to call external CLIs (see `aws-ssm` plugin)
4. Build and distribute the `.wasm` file
