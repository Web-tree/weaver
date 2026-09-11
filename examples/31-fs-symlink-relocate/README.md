# 31 - fs.symlink Cache Relocation

Demonstrates relocating a real directory (e.g. a Rust toolchain cache) from
a small internal disk (`src`) onto a bigger external volume (`dst`), leaving
a symlink at `src` so tools that hardcode the original path keep working
transparently.

## What this covers

- `fs.symlink` (WASM plugin, `plugins/fs-symlink`) for idempotent
  src -> symlink -> dst convergence
- `enabled: false` as the on/off switch a downstream module uses to turn a
  relocation target on/off (not exercised by this fixture — see "Other
  cases" below)
- The conflict case, where real data exists at *both* `src` and `dst` (not
  exercised by this fixture — see "Other cases" below)

## Before state

- `src` (`/tmp/wvr-examples/31-fs-symlink-relocate/rustup`) is a real
  directory with real data (a fake toolchain file), simulating an existing,
  never-relocated `~/.rustup`.
- `dst` (`/tmp/wvr-examples/31-fs-symlink-relocate/vault/caches/rustup`)
  does not exist yet.

## Setup

Unlike the other numbered examples, `fs.symlink` operates on absolute
paths *outside* the app directory by design (that's the point — relocating
`~/.rustup`, not a file inside the repo being converged), so the "before"
state can't be represented purely as checked-in files under `before/`. Create
it once before running `wvr apply`:

```sh
mkdir -p /tmp/wvr-examples/31-fs-symlink-relocate/rustup
echo "fake toolchain data" > /tmp/wvr-examples/31-fs-symlink-relocate/rustup/marker
rm -rf /tmp/wvr-examples/31-fs-symlink-relocate/vault
```

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply:
- `/tmp/wvr-examples/31-fs-symlink-relocate/vault/caches/rustup/marker`
  exists (the real data moved to `dst`).
- `/tmp/wvr-examples/31-fs-symlink-relocate/rustup` is now a symlink
  pointing at `/tmp/wvr-examples/31-fs-symlink-relocate/vault/caches/rustup`.
- Re-running `wvr apply` is a no-op ("already linked") — idempotent.

## Other cases (config-only, not exercised by this fixture)

- **`enabled: false`** — set on a per-target basis so a downstream module
  can turn a relocation on/off without any other special-casing:
  ```yaml
  - type: fs.symlink
    src: /Users/max/.rustup
    dst: /Volumes/Vault/caches/rustup
    enabled: false
  ```
  `plan` reports a no-op ("skip {src} (disabled)"); `execute` performs no
  filesystem operations and returns `"Skipped {src} (disabled by config)"`.

- **Conflict** — if both `src` and `dst` already contain real (non-symlink)
  data, `fs.symlink` never deletes or merges anything. `plan` surfaces it as
  a warning; `execute` fails with `EnsureError::ExecutionError` asking the
  user to resolve manually:
  ```
  both /Users/max/.rustup and /Volumes/Vault/caches/rustup contain data;
  resolve manually before running apply again
  ```

## Known limitations of this example (read before relying on it)

This example was originally authored against the real source without being
able to run it end-to-end. It has since been run for real, and the plugin
resolution gap noted below has been fixed. One limitation remains, in the
test harness rather than in the plugin or `wvr apply` itself:

1. **Fixed:** `PluginResolver::resolve_ensure_type` (in
   `crates/core/src/plugin/resolver.rs`) now checks `WeaverConfig.plugins`
   (via `PluginResolver::set_plugins_config`, called from
   `crates/cli/src/commands/apply.rs`) before falling back to
   `PluginSource::Registry` auto-discovery. A `plugins: fs-symlink: path:
   ...` entry like the one above is honored by `wvr apply`'s generic `type:`
   ensure dispatch, not just by `wvr plugins update/list/verify`. Manually
   verified: with `plugins/fs-symlink/plugin.wasm` built
   (`./scripts/build-plugins.sh fs-symlink`), running `wvr apply` from
   `before/` genuinely relocates `src` to `dst` and symlinks it back, and a
   second run reports "already linked" (idempotent). This same fix applies
   to any plugin with a `plugins:` override, and — for the identical
   underlying reason — to `examples/16-go-module-management` and
   `examples/17-rust-cargo-management` *if* they declared a `plugins:`
   override too; as shipped, they don't (they rely on registry
   auto-discovery), and they are blocked by a separate, unrelated issue (see
   their entries in `examples/test-suite.yaml`), so they remain `pending`.
2. **Generic plugin ensures only work at module scope.** A `type:` that
   isn't one of the built-in `EnsureConfig` variants must be declared under
   a module's `weaver.module.yaml` `ensures:` list (dispatched via
   `manifest.ensures` in `apply.rs`) — `apps: [...].ensures` in the root
   `weaver.yaml` is the separate, closed `EnsureSpec` enum and does not fall
   through to plugin dispatch. That's why this example's `fs.symlink` entry
   lives in `module/weaver.module.yaml`, not in `before/weaver.yaml`
   directly. This part of the original design was correct.
3. **Still pending, but only for the automated suite
   (`crates/cli/tests/examples_suite.rs`):** that harness copies each
   example directory into an isolated `TempDir` before running `wvr apply`
   (see `run_example`/`copy_dir`), so a `plugins:` `path:` that reaches
   outside the example's own subtree — `"../../../plugins/fs-symlink"`,
   3 levels up to the repo's `plugins/` — can't be found there; only
   `examples/31-fs-symlink-relocate/` itself gets copied, not
   `plugins/fs-symlink/`. That's a limitation of how the harness isolates
   examples, not a bug in `wvr apply` or this plugin — hence `stage:
   pending` in `examples/test-suite.yaml` for now, with a comment pointing
   back here. Flip it to `implemented` if/when the harness (or this example)
   is changed so the local plugin path resolves inside the copied tree too.
