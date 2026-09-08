# 17 - Rust Cargo Dependency and Formatting Ensures

Demonstrates Rust workspace convergence with Cargo-native operations.

## What this covers

- `ensure.cargo.dep` for dependency pinning in `Cargo.toml` (PRD §6)
- `ensure.cargo.fmt` for deterministic formatting (PRD §6)
- Native-tool flow with `cargo add` / `cargo fmt` (PRD §2)

## Before state

- Tool crate has minimal dependencies
- Needed CLI and serialization crates are missing
- Source formatting does not match `rustfmt` output

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `Cargo.toml` includes required dependencies and features
- `src/main.rs` is rustfmt-formatted
- Team conventions from the module are copied into the crate directory
