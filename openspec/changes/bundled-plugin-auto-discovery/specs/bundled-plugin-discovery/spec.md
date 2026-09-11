## ADDED Requirements

### Requirement: WEAVER_REPO local checkout fallback
When resolving a generic ensure plugin `type:` that has no explicit `plugins:` override in the consuming `weaver.yaml`, and the `WEAVER_REPO` environment variable is set, the system SHALL attempt to load `$WEAVER_REPO/plugins/<plugin-name>/plugin.wasm` before falling back to a network Registry fetch.

#### Scenario: WEAVER_REPO set and plugin is built
- **WHEN** `WEAVER_REPO` points at a weaver checkout containing a built `plugins/<name>/plugin.wasm`, and no `plugins:` override exists for `<name>`, and no cached copy exists
- **THEN** the system loads that file and uses it to resolve the ensure, without any network request

#### Scenario: WEAVER_REPO set but plugin not built there
- **WHEN** `WEAVER_REPO` is set but `plugins/<name>/plugin.wasm` does not exist at that path
- **THEN** the system falls through to the existing Registry resolution behavior, with no error raised for the missing local file

#### Scenario: WEAVER_REPO unset
- **WHEN** `WEAVER_REPO` is not set in the environment
- **THEN** resolution behaves exactly as it does today — cache, then Registry — with no new code path exercised

### Requirement: Explicit configuration takes precedence over WEAVER_REPO
An explicit `plugins: <name>: {path: ...}` or `{git: ..., ref: ...}` entry in `weaver.yaml` SHALL always be used over the `WEAVER_REPO` fallback, regardless of whether `WEAVER_REPO` is set.

#### Scenario: Both an explicit override and WEAVER_REPO are present
- **WHEN** `weaver.yaml` configures `plugins: {<name>: {path: /some/other/path}}` and `WEAVER_REPO` is also set in the environment
- **THEN** the system resolves `<name>` from the explicit `path:` override; `WEAVER_REPO` is not consulted for that plugin

### Requirement: WEAVER_REPO fallback sits between cache and Registry
The `WEAVER_REPO` fallback SHALL be attempted after checking the local `~/.rw/plugins/<name>/latest/` cache and before any network Registry fetch, so a cache hit or a `WEAVER_REPO` hit both avoid network access.

#### Scenario: Cache already has the plugin
- **WHEN** `~/.rw/plugins/<name>/latest/plugin.wasm` already exists from a prior resolution
- **THEN** the cached copy is used; `WEAVER_REPO` is not consulted even if set

#### Scenario: Cache miss, WEAVER_REPO hit
- **WHEN** there is no cached copy, `WEAVER_REPO` is set, and `$WEAVER_REPO/plugins/<name>/plugin.wasm` exists
- **THEN** the system uses the `WEAVER_REPO` copy and does not perform a network Registry request
