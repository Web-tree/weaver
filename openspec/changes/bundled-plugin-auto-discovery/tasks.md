## 1. Resolver: WEAVER_REPO fallback

- [ ] 1.1 In `crates/core/src/plugin/resolver.rs`, add a `try_load_weaver_repo_plugin(name: &str) -> Result<Option<Vec<u8>>, PluginError>` helper: read `WEAVER_REPO` from the environment, return `None` if unset; else check `$WEAVER_REPO/plugins/<name>/plugin.wasm` and return its bytes if it exists, `None` otherwise (mirror `try_load_dev_plugin`'s read-and-log-on-hit pattern, but with a single fixed path, not the 3-level parent search).
- [ ] 1.2 In the `PluginSource::Registry` branch of `resolve_ensure_type` (currently ~line 260), call `try_load_weaver_repo_plugin` after the `~/.rw/plugins` cache-miss check and before the network `fetch_from_url` call. On a hit, store it in the cache (same as the `Git` branch's dev-plugin hit does) and return a `ResolvedPlugin` with `build_method: BuildMethod::Local` and a `source_url` like `format!("weaver-repo:plugins/{}", name)`.
- [ ] 1.3 Confirm (by reading, not assuming) that `set_plugins_config`'s explicit-override check in `resolve_ensure_type` (line ~102) still runs first and returns early before ever reaching the `Registry` branch — no change needed there, just verify.

## 2. Tests

- [ ] 2.1 Unit test: `WEAVER_REPO` unset → resolution reaches the Registry network path unchanged (existing behavior, no regression).
- [ ] 2.2 Unit test: `WEAVER_REPO` set, `plugins/<name>/plugin.wasm` present → resolved from there, no network call, cached afterward.
- [ ] 2.3 Unit test: `WEAVER_REPO` set, `plugins/<name>/plugin.wasm` absent → falls through to Registry (use a temp dir fixture, same pattern as the existing `resolve_ensure_type_*` tests added for the `plugins:`-override fix).
- [ ] 2.4 Unit test: explicit `plugins:` override present AND `WEAVER_REPO` set → override wins, `WEAVER_REPO` path never touched (assert via a `WEAVER_REPO` pointed at a directory with no `plugins/` dir at all, so any attempt to read it would error/panic if reached).
- [ ] 2.5 Unit test: `~/.rw/plugins` cache already has the plugin, `WEAVER_REPO` also set → cache wins, `WEAVER_REPO` not consulted (same non-existent-directory trick as 2.4).

## 3. Docs

- [ ] 3.1 Document `WEAVER_REPO` in `docs/PLUGIN_DEVELOPMENT.md`: what it does, the precedence order (explicit config > WEAVER_REPO > cache... — get the exact final order from 1.2/2.x right, cache actually comes before WEAVER_REPO per design.md D2), and the "point it at your weaver checkout once, works for every `weaver.yaml` you write" use case.
- [ ] 3.2 Add a one-line mention in `--help` output for whichever CLI command surfaces plugin resolution (check `crates/cli` for where `RW_REGISTRY_URL` is currently documented, if anywhere, and mirror it).

## 4. Verification

- [ ] 4.1 `cargo test --workspace` passes.
- [ ] 4.2 Manual end-to-end: unset any `plugins:` override in a real `weaver.yaml` using `fs.symlink`, set `WEAVER_REPO=/Users/max/git/webtree/weaver`, run `wvr plan`, confirm it resolves without the override and without a network call (verifiable by running offline or checking logs for "Loaded plugin from" style output).
