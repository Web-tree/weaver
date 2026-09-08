# Plugin Development & Release Guide

> **Bundled plugins.** `plugins/aws-ssm` (secrets provider) and `plugins/ai-patch`
> (the `ai.patch` ensure) are wired into the engine. `plugins/npm-script` is a
> **reference example** of the plugin SDK: the built-in `npm.script` /
> `ensure.npm.*` ensures are handled natively (deterministic package.json edits,
> no `npm` required), so npm-script is here to demonstrate authoring a plugin,
> not as the production npm handler.

## Creating a New Plugin

This guide walks you through creating a new plugin for `wvr`.

### Prerequisites

- **Rust**: Ensure you have the latest stable Rust installed.
- **cargo-component**: Tool for building WebAssembly components.
  ```bash
  cargo install cargo-component --locked
  ```

### 1. Scaffolding

Create a new library component in the `plugins/` directory:

```bash
cargo component new --lib plugins/my-plugin
```

### 2. Configure `Cargo.toml`

Edit `plugins/my-plugin/Cargo.toml` to configure the WIT bindings and dependencies:

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[package.metadata.component]
package = "weaver:plugin"  # Required package name

[package.metadata.component.target]
world = "ensure-provider"  # The interface this plugin implements
path = "../../wit"         # Path to WIT definitions

[lib]
crate-type = ["cdylib"]    # Must be cdylib for WASM

[dependencies]
wit-bindgen = "0.50.0"     # Required for generating bindings
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### 3. Implement the Interface

Edit `plugins/my-plugin/src/lib.rs` to implement the plugin logic.

**Boilerplate:**

```rust
use serde::Deserialize;
use wit_bindgen::generate;

// Generate bindings from WIT
generate!({
    world: "ensure-provider",
    path: "../../wit",
});

use exports::weaver::plugin::ensures::{EnsureError, EnsurePlan, EnsureRequest, Guest};
use weaver::plugin::process::{exec, ExecRequest};

struct Component;

// Define your config struct
#[derive(Deserialize)]
struct MyPluginConfig {
    name: String,
    // Add other fields matching the 'config' passed from weaver.yaml
}

impl Guest for Component {
    /// Plan acts like a "check" or "diff".
    /// It should return:
    /// - description: What needs to be done (or "Already set")
    /// - actions: List of commands to execute (empty if already correct)
    fn plan(req: EnsureRequest) -> Result<EnsurePlan, EnsureError> {
        let config: MyPluginConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;

        // Logic to check current state...
        // ...

        if already_correct {
             Ok(EnsurePlan {
                description: format!("Resource '{}' already exists", config.name),
                actions: vec![], // No actions needed
            })
        } else {
             Ok(EnsurePlan {
                description: format!("Create resource '{}'", config.name),
                actions: vec![format!("echo 'creating {}'", config.name)],
            })
        }
    }

    /// Execute performs the actual changes (or dry-run).
    fn execute(req: EnsureRequest) -> Result<String, EnsureError> {
        let config: MyPluginConfig = serde_json::from_str(&req.config)
            .map_err(|e| EnsureError::ConfigError(format!("Invalid config: {}", e)))?;

        if req.dry_run {
            return Ok(format!("Would execute action for '{}'", config.name));
        }

        // Perform side effects...
        // e.g., run a command
        // let result = exec(&ExecRequest { ... })?;

        Ok(format!("Successfully ensured '{}'", config.name))
    }
}

export!(Component);
```

### 4. Build and Test

Build your new plugin locally:

```bash
./scripts/build-plugins.sh my-plugin
```

Then test it by creating an example project that uses it (see `examples/` for inspiration).

---

## Development Workflow

### Building Plugins Locally

To build all plugins for local development:

```bash
./scripts/build-plugins.sh
```

To build a specific plugin:

```bash
./scripts/build-plugins.sh npm-script
```

This will:
1. Build the WASM component
2. Copy it to `plugins/<name>/plugin.wasm`
3. Make it available for local development testing

**Note:** Built `plugin.wasm` files are gitignored and should not be committed.

### Testing Plugins Locally

After building, test from an example project:

```bash
cd examples/npm-script
cargo r -- apply
# or
cargo r -- plugins update --all
```

The resolver will automatically use the local `plugin.wasm` file.

## Releasing Plugins

### Prerequisites

- You must be on the `main` branch
- All changes must be committed
- You must have push access to the repository

### Release Process

Use the release script for simplified releases:

```bash
./scripts/release-plugin.sh <plugin-name> <version>
```

Example:

```bash
./scripts/release-plugin.sh npm-script 1.0.0
```

This script will:
1. ✅ Update `version` in `plugins/<name>/Cargo.toml`
2. ✅ Show you the diff for review
3. ✅ Commit the version bump
4. ✅ Create a git tag: `<plugin-name>-v<version>`
5. ✅ Provide push instructions

### Manual Release (Alternative)

If you prefer manual control:

1. **Update version:**
   ```bash
   # Edit plugins/npm-script/Cargo.toml
   version = "1.0.1"
   ```

2. **Commit:**
   ```bash
   git add plugins/npm-script/Cargo.toml
   git commit -m "chore(npm-script): bump version to 1.0.1"
   ```

3. **Tag:**
   ```bash
   git tag npm-script-v1.0.1
   ```

4. **Push:**
   ```bash
   git push origin main --tags
   ```

### What Happens After Pushing

GitHub Actions will automatically:
1. ✅ Parse the tag to extract plugin name  and version
2. ✅ Verify version matches `Cargo.toml`
3. ✅ Build the WASM component
4. ✅ Generate SHA256 checksum
5. ✅ Create GitHub release (NOT marked as "latest")
6. ✅ Upload `plugin.wasm` and `plugin.wasm.sha256`

### Release URLs

After release, the plugin will be available at:
```
https://github.com/web-tree/repo-weaver/releases/download/<plugin-name>-v<version>/plugin.wasm
```

Example:
```
https://github.com/web-tree/repo-weaver/releases/download/npm-script-v1.0.0/plugin.wasm
```

## Semantic Versioning

Follow semver for plugin versions:

- **MAJOR** (1.0.0 → 2.0.0): Breaking changes
- **MINOR** (1.0.0 → 1.1.0): New features, backward compatible
- **PATCH** (1.0.0 → 1.0.1): Bug fixes, backward compatible

Example version progression:
- `0.1.0` - Initial development
- `0.2.0` - Add new feature
- `0.2.1` - Fix bug
- `1.0.0` - Stable API, production ready
- `1.1.0` - Add optional feature
- `2.0.0` - Breaking API change

## Tag Naming Convention

### Plugin Tags
Format: `<plugin-name>-v<semver>`

Examples:
- `npm-script-v1.0.0`
- `npm-script-v1.0.1`
- `aws-ssm-v0.1.0`

### CLI Tags (for comparison)
Format: `v<semver>` (no prefix)

Examples:
- `v0.1.0` ← Marked as "latest"
- `v0.2.0` ← Marked as "latest"

**Important:** Only CLI releases are marked as "latest" in GitHub. Plugin releases are standalone.

## Troubleshooting

### "WASM file not found" during build

Make sure you have `cargo-component` installed:
```bash
cargo install cargo-component --locked
```

### Version mismatch error in CI

The tag version must match the version in `Cargo.toml`. Use the release script to avoid this.

### Can't find plugin locally

Make sure you've built the plugin:
```bash
./scripts/build-plugins.sh <plugin-name>
ls -lh plugins/<plugin-name>/plugin.wasm
```

## FAQ

**Q: Do I need to build plugins to use wvr?**
A: No! End users never build plugins. They're automatically downloaded from GitHub releases.

**Q: When should I build plugins locally?**  
A: Only when developing or testing plugin code changes.

**Q: Can I have multiple versions of a plugin?**  
A: Yes! Each tag creates a separate release. Users can pin to specific versions in their `weaver.yaml`.

**Q: What if I make a mistake in a release?**  
A: You can delete the tag and release on GitHub, fix the issue, and re-release with a patch version.
