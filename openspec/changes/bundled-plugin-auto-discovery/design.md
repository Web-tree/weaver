## Context

`plugins/` in this repo ships 9 ensure plugins (`cargo-dep`, `fs-symlink`, `go-dep`, ...), each a WASM component built by `scripts/build-plugins.sh <name>` into `plugins/<name>/plugin.wasm`. Today `PluginResolver::resolve_ensure_type` (`crates/core/src/plugin/resolver.rs`) has exactly two ways to find a plugin for a generic `type:` ensure with no explicit `plugins:` override:

1. `~/.rw/plugins/<name>/latest/` cache, populated only by a prior successful resolution.
2. A network fetch from the `Registry` URL (`RW_REGISTRY_URL` or `https://plugins.repo-weaver.dev`), which requires the plugin to have been released via the `*-v*.*.*` tag → `release-plugin.yml` CI flow.

There's a third, narrower mechanism, `try_load_dev_plugin`, but it only looks at the *consuming app's* `project_dir` (and 2 parent dirs) — meant for weaver's own `examples/` to self-test bundled plugins in CI, and only reachable from the `Git` source branch, not `Registry`. It does nothing for a `weaver.yaml` that lives anywhere else (a dotfiles repo, `$HOME`, another project) — exactly the case this proposal is for.

Motivating case: a personal weaver module uses `type: fs.symlink`. The plugin isn't released yet (no tag pushed). Every machine that wants to use it needs its own `plugins: {fs-symlink: {path: /wherever/i/cloned/weaver/plugins/fs-symlink}}`, hardcoded to that machine's clone path, in every `weaver.yaml` that uses it.

## Goals / Non-Goals

**Goals:**
- One-time, per-machine configuration (an env var) that lets any `weaver.yaml`, anywhere, resolve any bundled plugin from a local weaver checkout — without a `plugins:` override per config file, and without needing a release tag.
- Zero behavior change for existing explicit `plugins:` config, existing `Local`/`Git` sources, and existing Registry-only resolution when the env var is unset.

**Non-Goals:**
- Auto-building a missing `plugin.wasm` (still requires `scripts/build-plugins.sh <name>` first, same as today's `Local` source).
- Multi-checkout search paths (one `WEAVER_REPO` at a time — a search-list is a possible future extension, not v1).
- Replacing or changing the registry/release flow (`release-plugin.yml`) — this is a local-development-and-personal-use fallback, not a distribution mechanism.
- Auto-detecting a weaver checkout from the consuming project's own location (rejected — see Decisions).

## Decisions

**D1. New env var `WEAVER_REPO`, not project_dir-relative auto-detection.**
`try_load_dev_plugin` already does project_dir-relative detection, and it's insufficient here precisely because the consuming `weaver.yaml` usually does *not* live inside or near the weaver checkout (a dotfiles repo, `$HOME`, an unrelated project). An explicit env var makes the source unambiguous and machine-global (set once in a shell profile), independent of where any given `weaver.yaml` happens to sit. Alternative names considered: `WEAVER_HOME` (rejected — collides in spirit with `~/.rw`, weaver's own state/cache home, which is a different thing), `WVR_DEV_PLUGINS_DIR` (rejected — more accurate but `WEAVER_REPO` reads better and generalizes if this repo later needs to expose more than plugins from the same var).

**D2. Precedence: explicit `plugins:` config > `WEAVER_REPO` fallback > Registry fetch.**
`WEAVER_REPO` only comes into play inside the existing `Registry` branch, after the `~/.rw/plugins` cache miss, before the network fetch — so a local dev checkout is preferred over downloading a (possibly older or unpublished) registry copy, but never overrides an explicit override someone deliberately configured.

**D3. Require a pre-built `plugin.wasm`, same as the `Local` source today.**
No new build-on-demand step. Consistent with existing `Local`/dev-plugin behavior; keeps this change scoped to resolution, not build orchestration.

**D4. Resolved path: `$WEAVER_REPO/plugins/<name>/plugin.wasm`, single fixed layout.**
Matches the existing `plugins/<name>/plugin.wasm` convention used everywhere else in this repo (`try_load_dev_plugin`, `scripts/build-plugins.sh`, the `Local` source). No new layout to learn.

## Risks / Trade-offs

- **[Stale local wasm silently preferred over a newer registry release]** → Mitigation: this is deliberate (D2) for a dev-focused fallback, but document it clearly in `docs/PLUGIN_DEVELOPMENT.md` and in the `WEAVER_REPO` description, so it's not a surprise when a locally-built plugin shadows what `wvr plugins update` would otherwise fetch.
- **[Env var typo'd to a non-weaver directory]** → Mitigation: resolution simply misses (falls through to Registry) if `$WEAVER_REPO/plugins/<name>/plugin.wasm` doesn't exist — no error, no partial state. Same failure mode as today's Registry-miss path.
- **[Confusion with `~/.rw` naming]** → Mitigation: addressed by D1's name choice; call this out explicitly in the CLI's `--help`/docs text for `WEAVER_REPO`.

## Migration Plan

Purely additive — no existing config or behavior changes when `WEAVER_REPO` is unset. No migration steps for existing users. Document the new env var in `docs/PLUGIN_DEVELOPMENT.md` and mention it as an alternative to per-file `plugins: path:` overrides.

## Open Questions

- Should `WEAVER_REPO` also feed *module* resolution (today: git-only, per `crates/core/src/module.rs`'s `ModuleResolver::resolve`, always `git ls-remote`)? Out of scope for this change — flagged here in case a follow-up proposal wants to extend the same idea to modules, not just plugins.
