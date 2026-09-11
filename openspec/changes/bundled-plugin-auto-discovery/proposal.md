## Why

Every ensure plugin that ships in this repo's own `plugins/` directory (`cargo-dep`, `fs-symlink`, etc.) is unusable from a `wvr apply` run until it's either published to the plugin registry (a tagged release) or explicitly pointed at via a `plugins: <name>: {path: ...}` override in the consuming `weaver.yaml`. That's fine for a plugin developed by and for one project, but weaver's own bundled plugins are meant to be broadly reusable across any machine that has this repo cloned — today every such machine needs its own hand-written `path:` override, hardcoded to wherever that person happened to clone the repo, before any of them work. There's no way to say "use the plugins from wherever I have weaver checked out" once and have it apply everywhere.

## What Changes

- Add a new plugin resolution fallback: when a generic ensure `type:` has no explicit `plugins:` override, and no cached/registry copy is found, check a small set of well-known local locations for a weaver checkout (starting with a new `WEAVER_HOME` env var) and, if found, load `plugins/<name>/plugin.wasm` from it directly — the same file `scripts/build-plugins.sh` already produces today.
- This sits below explicit `plugins:` config (still wins) and is attempted before the network `Registry` fetch, so a local dev/bundled checkout is preferred over downloading a possibly-stale published version.
- No change to any existing explicit-config resolution path (`Local`, `Git`) — fully additive, opt-in via `WEAVER_HOME`.

## Capabilities

### New Capabilities
- `bundled-plugin-discovery`: resolves an ensure plugin's `.wasm` from a locally-checked-out weaver repo (located via `WEAVER_HOME` or equivalent) when no explicit `plugins:` override and no cached/registry copy exists.

### Modified Capabilities
(none — `openspec/specs/` has no existing captured spec for plugin resolution yet; this repo's prior plugin-resolution spec lives outside openspec, in `specs/003-plugin-management-system/`, out of scope here)

## Impact

- `crates/core/src/plugin/resolver.rs`: new resolution tier in `resolve_ensure_type`'s `Registry` branch (today it has no dev-checkout fallback at all — the existing `try_load_dev_plugin` helper is unrelated: it only looks at the *consuming app's own* `project_dir` and two parent levels, for weaver's own example tests, and is only wired into the `Git` branch).
- New env var: `WEAVER_HOME` (name TBD in design.md).
- `docs/PLUGIN_DEVELOPMENT.md`: document the new resolution tier.
- No breaking changes — purely additive fallback, opt-in, behind an unset-by-default env var.
