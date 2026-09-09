# Issue catalog: plugin-first standards engine

Source of truth for the GitHub issues that implement
`docs/superpowers/specs/2026-09-09-plugin-first-standards-engine-design.md`.
Each `<!-- issue ... -->` marker opens one issue; `{{key}}` inside a body is replaced
with the created issue number. Handoff refs (`R*`, `C*`) point at
`webtree-dev-standards-k8s/docs/handoffs/weaver-dev-standards-module-requirements.md`;
`FR-*` at `specs/004-generic-standardization-engine/spec.md`.

Priorities: **P0** blocks everything after it · **P1** needed for the first gate ·
**P2** needed for declarative, fixable rules · **P3** adoption/fleet polish.

## Created issues (2026-09-09)

| key | issue |
|---|---|
| `epic-plugins` | [Web-tree/weaver#57](https://github.com/Web-tree/weaver/issues/57) |
| `plugins-01` | [Web-tree/weaver#58](https://github.com/Web-tree/weaver/issues/58) |
| `plugins-02` | [Web-tree/weaver#59](https://github.com/Web-tree/weaver/issues/59) |
| `plugins-03` | [Web-tree/weaver#60](https://github.com/Web-tree/weaver/issues/60) |
| `plugins-04` | [Web-tree/weaver#61](https://github.com/Web-tree/weaver/issues/61) |
| `epic-prims` | [Web-tree/weaver#62](https://github.com/Web-tree/weaver/issues/62) |
| `prims-01` | [Web-tree/weaver#63](https://github.com/Web-tree/weaver/issues/63) |
| `prims-02` | [Web-tree/weaver#64](https://github.com/Web-tree/weaver/issues/64) |
| `prims-03` | [Web-tree/weaver#65](https://github.com/Web-tree/weaver/issues/65) |
| `prims-04` | [Web-tree/weaver#66](https://github.com/Web-tree/weaver/issues/66) |
| `prims-05` | [Web-tree/weaver#67](https://github.com/Web-tree/weaver/issues/67) |
| `prims-06` | [Web-tree/weaver#68](https://github.com/Web-tree/weaver/issues/68) |
| `prims-07` | [Web-tree/weaver#69](https://github.com/Web-tree/weaver/issues/69) |
| `prims-08` | [Web-tree/weaver#70](https://github.com/Web-tree/weaver/issues/70) |
| `epic-engine` | [Web-tree/weaver#71](https://github.com/Web-tree/weaver/issues/71) |
| `engine-05` | [Web-tree/weaver#72](https://github.com/Web-tree/weaver/issues/72) |
| `engine-07` | [Web-tree/weaver#73](https://github.com/Web-tree/weaver/issues/73) |
| `engine-09` | [Web-tree/weaver#74](https://github.com/Web-tree/weaver/issues/74) |
| `engine-01` | [Web-tree/weaver#75](https://github.com/Web-tree/weaver/issues/75) |
| `engine-02` | [Web-tree/weaver#76](https://github.com/Web-tree/weaver/issues/76) |
| `engine-03` | [Web-tree/weaver#77](https://github.com/Web-tree/weaver/issues/77) |
| `engine-06` | [Web-tree/weaver#78](https://github.com/Web-tree/weaver/issues/78) |
| `engine-10` | [Web-tree/weaver#79](https://github.com/Web-tree/weaver/issues/79) |
| `engine-04` | [Web-tree/weaver#80](https://github.com/Web-tree/weaver/issues/80) |
| `engine-08` | [Web-tree/weaver#81](https://github.com/Web-tree/weaver/issues/81) |
| `epic-modules` | [Web-tree/weaver#82](https://github.com/Web-tree/weaver/issues/82) |
| `modules-01` | [Web-tree/weaver#83](https://github.com/Web-tree/weaver/issues/83) |
| `modules-02` | [Web-tree/weaver#84](https://github.com/Web-tree/weaver/issues/84) |
| `ds-01` | [Web-tree/webtree-dev-standards-k8s#82](https://github.com/Web-tree/webtree-dev-standards-k8s/issues/82) |
| `ds-02` | [Web-tree/webtree-dev-standards-k8s#83](https://github.com/Web-tree/webtree-dev-standards-k8s/issues/83) |
| `ds-03` | [Web-tree/webtree-dev-standards-k8s#84](https://github.com/Web-tree/webtree-dev-standards-k8s/issues/84) |

---

<!-- issue key=epic-plugins repo=Web-tree/weaver labels=epic,plugins -->
# Epic: plugin platform — universal WASM workers with check/plan/execute faces

**Priority:** P0–P1 · **Design:** `docs/superpowers/specs/2026-09-09-plugin-first-standards-engine-design.md`

Plugins are the unopinionated tier: each exposes a `type` (`json.key`, `text.section`)
and a JSON config contract, does its work inside a WASM sandbox, and is composed by
modules. Today the WIT world only exports `plan`/`execute`, the only host capability is
`process.exec` (plugins cannot read a file except by shelling out), and plugin dispatch
ignores the `plugins:` declarations in `weaver.yaml`, so a plugin from another repo is
not actually loadable. This epic closes those gaps.

## Issues

- [ ] {{plugins-01}} — WIT `weaver:plugin@0.2.0`: `check` face + structured plan/check results
- [ ] {{plugins-02}} — Scoped workspace filesystem for plugins via WASI preopens (read-only for check/plan)
- [ ] {{plugins-03}} — Resolve plugins from declared sources (consumer `plugins:`, module `plugins:`, bundled table)
- [ ] {{plugins-04}} — Versioned WIT package, out-of-tree plugin template, reusable release workflow

## Later (not scheduled)

- `describe` export: a plugin reports its name, provided types, JSON schema for its config,
  and required capabilities, so `wvr` validates config before running and
  `wvr plugins list` shows what each plugin provides.
- Capability manifest per plugin (network, exec allowlist) enforced by the host.

## Done when

An out-of-tree plugin repo built from the template is declared under `plugins:` in a
consumer `weaver.yaml`, resolved from its GitHub release, and its `check` face runs under
`wvr check` with the app root mounted read-only.

---

<!-- issue key=plugins-01 repo=Web-tree/weaver labels=plugins,enhancement -->
# WIT `weaver:plugin@0.2.0`: add `check` face and structured plan/check results

**Priority:** P0 · **Epic:** {{epic-plugins}} · **Refs:** R20 (second half), FR-031, handoff open decision 1

## Why

A rule must serve validation and convergence from one declaration. The `ensures`
interface exports only `plan`/`execute` (`wit/plugin.wit:59-60`), and `ensure-plan` is a
description plus a list of free-text actions, so the engine cannot tell "converged" from
"one change" without parsing prose, and a plugin has no way to say *what* is wrong.

## What

1. Bump the package to `weaver:plugin@0.2.0` and add a `types` interface:
   - `check-status { pass, fail, skip, error }`
   - `finding { path, location, message, expected, observed, remediation, fixable }`
   - `check-result { status, message, findings }`
   - `planned-change { action: create|update|remove, path, preview }`
   - `plan-result { converged, description, changes }`
   - `ensure-request` gains `workspace` (guest mount path, see {{plugins-02}}) and renames
     `app-path` to `host-app-path`
   - `ensure-error` gains `not-fixable(string)`
2. `ensures` exports `check: func(req) -> result<check-result, ensure-error>` next to
   `plan`/`execute`; `plan` returns `plan-result`.
3. Host side (`crates/core/src/plugin/ensure_wasm.rs`): `LoadedEnsurePlugin::check`, and
   the `Ensure` trait (`crates/core/src/ensure/mod.rs`) gains `fn check(&self, ctx) ->
   Result<CheckResult>`. `EnsurePluginWrapper` forwards all three faces.
4. Built-in ensures (`file.*`, `git.*`, `npm.*`) get a default `check()` derived from
   `plan()`: converged ⇒ `pass`, otherwise `fail` with one finding per planned change.
5. Update the eight in-tree plugins to the new bindings (mechanical); `taskfile-task`
   implements a real `check` (task missing ⇒ `fail`, `fixable: false`).
6. `docs/PLUGIN_DEVELOPMENT.md` documents the three faces and when each is called.

## Acceptance

- `cargo test --workspace` passes; `./scripts/build-plugins.sh` builds every plugin.
- An integration test loads `taskfile-task` and asserts `check` returns `fail` with a
  finding naming the task when the Taskfile lacks it, `pass` when present.
- `plan` on a converged `file.exists` returns `converged: true, changes: []`.
- No plugin or host code parses free-text actions to decide convergence.

## Notes

- Keep `description` on `plan-result` for the human plan output.
- `skip` is for "rule not applicable here" (e.g. no `package.json`); it must never be
  used to hide an evaluation error — that is `error`.

---

<!-- issue key=plugins-02 repo=Web-tree/weaver labels=plugins,enhancement -->
# Scoped workspace filesystem for plugins via WASI preopens (read-only for check/plan)

**Priority:** P0 · **Epic:** {{epic-plugins}} · **Refs:** R8, FR-031, FR-032

## Why

Plugins have no filesystem access. Every shipped plugin shells out
(`test -d .beads`, `task --list-all`) through `process.exec` to learn anything about the
repo. A `json.key` or `text.section` plugin cannot exist this way, and the handoff's
"check must be strictly read-only" (R8) is unenforceable.

## What

Use `wasmtime_wasi`'s preopens rather than a custom WIT `fs` interface — `std::fs` in
the plugin then works against the mount, and the permission split is enforced by the
runtime.

1. `EnsureHost::new(app_root: &Path, mode: AccessMode)` builds the WASI context with
   `preopened_dir(app_root, "/workspace", dir_perms, file_perms)`:
   - `AccessMode::ReadOnly` (used by `plan` and `check`): `DirPerms::READ`, `FilePerms::READ`
   - `AccessMode::ReadWrite` (used by `execute`): `READ | MUTATE`, `READ | WRITE`
2. `ensure-request.workspace = "/workspace"`; `host-app-path` keeps the host path for
   `process.exec` `cwd` (the child process is outside the sandbox — document this).
3. Stop `inherit_env` by default for plugins; pass an explicit allowlist (`PATH`, `HOME`,
   and anything the consumer lists under `plugins.<name>.env`). Secrets stay in the
   `secrets` provider path.
4. `process.exec` gains `timeout-ms: option<u32>`; host kills the child on expiry and
   returns `execution-error`.
5. Reference plugin update: `taskfile-task` reads `Taskfile.yml` presence via `std::fs`
   before deciding to exec `task`.

## Acceptance

- Test: a plugin that writes a file under `/workspace` succeeds in `execute` and returns
  a WASI permission error in `check`/`plan`; the host surfaces it as `ensure-error` and
  the rule status is `error`, not `pass`.
- Test: `../` and absolute host paths from the guest fail (path escape rejected).
- Test: `process.exec` with `timeout-ms: 100` on `sleep 5` returns within ~1s.
- `docs/PLUGIN_DEVELOPMENT.md` has a "Reading and writing files" section with the
  `/workspace` convention.

## Notes

- `wasmtime_wasi::p2::add_to_linker_sync` already links `wasi:filesystem`; only the
  context builder changes.
- Pair with {{plugins-01}} (they touch the same request record); either can land first.

---

<!-- issue key=plugins-03 repo=Web-tree/weaver labels=plugins,enhancement -->
# Resolve plugins from declared sources: consumer `plugins:`, module `plugins:`, bundled table

**Priority:** P1 · **Epic:** {{epic-plugins}} · **Refs:** FR-014, FR-030, marketplace design G6

## Why

`build_plugin_ensure` calls `resolve_ensure_type(type)`, which always builds a
`Registry` source pointing at `https://plugins.repo-weaver.dev` — a host nobody serves
(`crates/core/src/plugin/resolver.rs`). The `plugins:` map in `weaver.yaml` is read only
by `wvr plugins update`. Modules cannot say which plugins they need. Net effect: a
plugin from another repository cannot be used for an ensure today, and bundled plugins
only work through the local-development fallback.

## What

1. `PluginConfig` gains `provides: Vec<String>` (ensure types). Validation: a type may be
   provided by at most one declared plugin.
2. `ModuleManifest` gains `plugins: HashMap<String, PluginConfig>` (FR-014). `path:` in a
   module manifest is relative to the module root, so a module may vendor a plugin.
3. A `PluginTable` is built per run: module-declared → consumer-declared → bundled.
   Dispatch looks the type up in the table; the `Registry` variant and its URL
   templating are removed.
4. Bundled table: `plugins/bundled.yaml` (name, provides, release tag, sha256) compiled
   into `wvr` with `include_str!`; a `just`/script target regenerates it at plugin
   release time. Source = `Web-tree/weaver` GitHub release asset (existing fetcher).
5. Unknown type ⇒ exit `4` with: the type, the plugins that were consulted, and the YAML
   to declare one.
6. Lockfile: `sha256` recorded for every fetched plugin and **verified on load**;
   mismatch ⇒ exit 4 with remediation. `--offline` refuses fetches (already exists for
   apply; extend to `check`).
7. Local dev fallback (`plugins/<name>/plugin.wasm`, up to two directories up) stays but
   only when `WEAVER_DEV_PLUGINS=1`, and logs which file was used.

## Acceptance

- Example `32-plugin-from-declared-source`: consumer `weaver.yaml` declares
  `plugins.hello: { path: ./plugins/hello, provides: [hello.check] }` and an app-level
  rule `type: hello.check`; `wvr check` runs it.
- Test: a module manifest declaring `plugins.x: { git, ref, provides }` resolves and
  pins `x` in `weaver.lock` on `wvr apply`.
- Test: undeclared, non-bundled type ⇒ exit 4 and the error names the type.
- Test: tampered cached `plugin.wasm` ⇒ checksum mismatch, exit 4.

## Depends on

{{plugins-01}} for the request shape; can be developed in parallel.

---

<!-- issue key=plugins-04 repo=Web-tree/weaver labels=plugins,documentation -->
# Versioned WIT package, out-of-tree plugin template, reusable release workflow

**Priority:** P1 · **Epic:** {{epic-plugins}} · **Refs:** "possible to load a custom plugin from another repository"

## Why

A third-party plugin author needs the WIT files at a known version, a working
`Cargo.toml`, and a way to publish `plugin.wasm` so the existing fetcher can find it.
Today `wit/` is only reachable by relative path inside this repo and the release
workflow assumes `plugins/<name>` in-tree.

## What

1. Tag WIT versions (`wit-v0.2.0`) and document how to depend on them from an external
   crate: `cargo component` target `path` pointing at a vendored copy fetched by a small
   `scripts/vendor-wit.sh <version>` (git archive of `wit/` at the tag). Mention
   `wit-deps` as the alternative.
2. `templates/plugin-rust/` in this repo: `Cargo.toml`, `src/lib.rs` implementing all
   three faces with a `hello.check` type, a unit test, `README.md`, and
   `.github/workflows/release.yml` that calls the reusable workflow below.
3. `wvr plugin new <name> [--dir <path>]` copies the template with names substituted.
4. Reusable workflow `.github/workflows/plugin-release.yml` (`workflow_call`) extracted
   from `release-plugin.yml`: builds with `cargo component`, verifies the tag matches the
   crate version, uploads `plugin.wasm` + `.sha256` to the release. In-tree plugins call
   the same workflow.
5. `docs/PLUGIN_DEVELOPMENT.md`: "Your plugin in another repository" walkthrough ending
   with the consumer `plugins:` declaration ({{plugins-03}}).

## Acceptance

- CI job: scaffold a plugin from the template into a temp dir, build it, load it with
  `wvr` via `plugins.<name>.path`, run `wvr check` — green.
- The reusable workflow is exercised by at least one in-tree plugin release.

## Depends on

{{plugins-01}} (WIT shape must be stable before tagging `wit-v0.2.0`).

---

<!-- issue key=epic-prims repo=Web-tree/weaver labels=epic,plugins -->
# Epic: generic primitive plugins (json/yaml/section/lines/file/scan) with check + fix faces

**Priority:** P1–P2 · **Design:** `docs/superpowers/specs/2026-09-09-plugin-first-standards-engine-design.md`

The handoff's case catalog (C1–C16) shows most standards are "a fragment inside a file
someone else owns". These plugins are the vocabulary: each is unopinionated, takes a
JSON config, and implements `check` (read-only), `plan`, and `execute`. Every plugin ships
with a registered example under `examples/` and unit tests in its crate.

## Issues

- [ ] {{prims-01}} — `json.key` (key-path get/set/assert, array identity addressing)
- [ ] {{prims-02}} — `yaml.key` (same for YAML, format-preserving where possible)
- [ ] {{prims-03}} — `text.section` (managed region with configurable markers, content hash check)
- [ ] {{prims-04}} — `text.lines` (line set present in an unstructured file)
- [ ] {{prims-05}} — `file` (exists/absent/directory/size/content/name pattern, over a path or glob)
- [ ] {{prims-06}} — `content.scan` (forbidden patterns, secrets presets, hostile Unicode)
- [ ] {{prims-07}} — `command` check: `expect`, `stdout_contains|matches`, `timeout`, `cwd`; fix examples 20/21
- [ ] {{prims-08}} — `weaver-plugin-sdk` crate: bindings re-export, config helpers, predicates (regex, semver, pinned, enum)

## Later

- `values.equal` — assert two selectors (possibly different files/formats) hold the same
  value (R19, C12). Until then C12 is a `command` rule.
- `ensure.npm.*` re-expressed as presets over `json.key` (FR-022).

## Done when

Every case C1–C16 in the handoff is expressible as a rule using only these types and
`taskfile.task`, and each plugin has a passing example in `examples/test-suite.yaml`.

---

<!-- issue key=prims-01 repo=Web-tree/weaver labels=plugins,enhancement -->
# `json.key` plugin: key-path get/set/assert in JSON with array identity addressing

**Priority:** P1 · **Epic:** {{epic-prims}} · **Refs:** R12, R13, R14, FR-022, C3, C4, C5

## Why

`json_merge::ensure_json_key` exists but is reachable only through `ensure.npm.*`. The
org's most standards-critical file (`.claude-plugin/marketplace.json`) is an **array of
objects** keyed by `name`; key-path-only support does not cover it.

## Config

```yaml
type: json.key
file: .claude-plugin/marketplace.json
path: plugins[name=crossplane-claim-generator].category   # dotted path; [k=v] selects an array element
op: set            # set | merge | absent          (fix face; default set)
value: quality     # any JSON value (scalar, object, array)
match: equals      # equals | contains | matches | exists | absent   (check face; default equals)
create: true       # create the file as {} when missing (default false ⇒ finding "file missing")
```

## Behaviour

- **check:** parse (`serde_json` with `preserve_order`), resolve `path`, compare with
  `match`. Findings carry `location` = the resolved key path, `expected`, `observed`
  (JSON-encoded), `fixable: true` when `op` can produce the expected state.
- **plan/execute:** upsert without reordering siblings, without duplicating the matched
  array element, preserving the file's indentation (detect 2/4/tab) and trailing newline.
  `merge` deep-merges objects; `absent` removes the key or element.
- Array addressing: `items[name=x]` selects the element whose `name == "x"`; on `set`
  with no match the element is appended as `{ "name": "x", ...value }`.

## Acceptance

- Examples: `33-json-key-object` (`.claude/settings.json` marketplace registration, C3)
  and `34-json-key-array` (marketplace `plugins[]` upsert with sibling fields, C4), both
  `stage: implemented` with byte-exact `after/` snapshots.
- Unit tests cover: nested create, sibling order preserved, array upsert idempotent,
  indentation preserved, `contains` on arrays, `absent`.
- `check` on the `after/` tree returns `pass`; on `before/` returns `fail` with one
  finding per missing/mismatched key.

## Depends on

{{plugins-01}}, {{plugins-02}}, {{plugins-03}} (bundled table entry).

---

<!-- issue key=prims-02 repo=Web-tree/weaver labels=plugins,enhancement -->
# `yaml.key` plugin: key-path get/set/assert in YAML, format-preserving where possible

**Priority:** P1 · **Epic:** {{epic-prims}} · **Refs:** R12, R13, R14, R18, FR-023, C9, C10, C11

## Why

`renovate.json`, `.yamllint.yaml`, every `.github/workflows/*.y*ml`, `Taskfile.yml` —
the YAML side of C3–C11. Comment and layout preservation is the hard part; spec 004
locked "warn-on-reformat fallback".

## Config

Same shape as `json.key` ({{prims-01}}), plus:

```yaml
predicate:          # optional, check face; from the sdk ({{prims-08}})
  pinned: true      # value is not a floating ref (no @main/@master/latest)
  semver: ">=7"     # value satisfies a range
  regex: "^actions/checkout@v[0-9]+$"
  one_of: [a, b]
```

## Behaviour

- **check:** parse with `serde_yml` (comments irrelevant for reading); multi-document
  files evaluate every document unless `document: <index>` is set.
- **fix:** two strategies, chosen automatically and reported in the plan preview:
  1. *targeted edit* — for scalar set/replace and sequence append at an existing mapping,
     edit lines in place using the parsed node's line/column (`serde_yml` spans or
     `yaml-rust2` marks), keeping comments and formatting;
  2. *re-serialize* — anything else; emits a `warn` finding "file will be reformatted"
     in `plan` and the report.
- Never touch a file whose check already passes.

## Acceptance

- Examples: `35-yaml-key-scalar` (renovate `extends`, C9), `36-yaml-key-workflow`
  (`concurrency.cancel-in-progress` across a workflow file, C10) — `stage: implemented`.
- Unit tests: comment preserved on targeted edit; reformat path emits the warn finding;
  `pinned` predicate rejects `@main`, accepts `@v7` and a 40-char SHA.
- `check` with `predicate.pinned` over `.github/workflows/*.yml` `jobs.*.steps[*].uses`
  is demonstrated in the example (glob fan-out arrives with {{engine-06}}; the example may
  list files explicitly until then).

## Depends on

{{plugins-01}}, {{plugins-02}}, {{plugins-03}}; predicates from {{prims-08}} (can start with
`equals`/`exists`).

---

<!-- issue key=prims-03 repo=Web-tree/weaver labels=plugins,enhancement -->
# `text.section` plugin: managed region with configurable markers and content-hash check

**Priority:** P1 · **Epic:** {{epic-prims}} · **Refs:** R11, R9 (partial), FR-021, C1, C2, example 30

## Why

`ensure.file.md_section` is built in, HTML-comment-only, always reports a change in
`plan`, requires the target file to exist, and has no read-only face. Plain-text targets
(`.tmux.conf` with `#` markers — example 30) are blocked on a configurable comment
leader. C1 additionally needs "is the managed block current", i.e. a content hash.

## Config

```yaml
type: text.section
file: AGENTS.md
selector:
  type: block_marker            # block_marker | heading
  id: beads-integration
marker_style: html              # html (<!-- -->) | hash (#) | slash (//) | custom
markers: { start: "<!-- BEGIN BEADS INTEGRATION {attrs} -->", end: "<!-- END BEADS INTEGRATION -->" }  # custom only
content: |                      # literal, or:
content_from_template: sections/beads.md.j2    # engine renders it before dispatch (module-relative)
on_exists: update               # update | append_only | skip
create_file: true               # create the file if missing (default true)
hash_attr: hash                 # optional: write/verify `hash:<sha256[:8]>` in the start marker
```

## Behaviour

- **check:** `fail` if the region is missing; `fail` if `on_exists: update` and the body
  (or `hash_attr`) differs; `pass` otherwise. Findings: `location: "block:<id>"` or
  `"heading:<path>"`, `expected`/`observed` as short diffs.
- **plan:** `converged` iff the would-be bytes equal the current bytes (fixes R5 for this
  primitive).
- **execute:** current `upsert_block_marker`/`upsert_heading` logic moved into the plugin;
  markers built from `marker_style`; heading selector unchanged (v1 limitations stay
  documented).
- Engine: `content_from_template` on any plugin rule is rendered with the app's Tera
  context and passed as `content` — generic for all plugins.

## Acceptance

- Example 30 (`tmux-claude-window-rename`) flips to `stage: implemented`.
- Example 23 keeps passing with `ensure.file.md_section` mapped to this plugin (keep the
  old type name as an alias in the dispatcher).
- New example `37-text-section-hash` shows a `hash:` marker attribute going stale and
  `wvr check` failing until `wvr apply` refreshes it.
- Unit tests: hash markers, `#` and `//` styles, `append_only`, `skip`.

## Depends on

{{plugins-01}}, {{plugins-02}}, {{plugins-03}}.

---

<!-- issue key=prims-04 repo=Web-tree/weaver labels=plugins,enhancement -->
# `text.lines` plugin: a set of lines is present in an unstructured file

**Priority:** P2 · **Epic:** {{epic-prims}} · **Refs:** R16, C8

## Config

```yaml
type: text.lines
file: .gitignore
lines: [".env*", ".rw/state.yaml", ".rw/answers.yaml"]
create_file: true
position: end                # end | after: "<regex>" | before: "<regex>"
comment_leader: "#"          # lines matching `^\s*#` are ignored when matching
header: "# managed by weaver" # optional line written once before the block
```

## Behaviour

- **check:** each line in `lines` must appear verbatim (trimmed) on its own non-comment
  line; one finding per missing line, `fixable: true`.
- **execute:** append missing lines (in order) at `position`, never duplicate, never
  reorder existing content, keep the file's newline convention.

## Acceptance

- Example `38-text-lines-gitignore`, `stage: implemented`.
- Unit tests: idempotent append, `after:` insertion, CRLF file preserved, comment lines
  not counted as matches.

## Depends on

{{plugins-01}}, {{plugins-02}}, {{plugins-03}}.

---

<!-- issue key=prims-05 repo=Web-tree/weaver labels=plugins,enhancement -->
# `file` plugin: exists/absent/directory/size/content/name-pattern over a path or glob

**Priority:** P1 · **Epic:** {{epic-prims}} · **Refs:** R10, R15 (paths), R21, C6, C7, C14, C15

## Why

C6 (required non-trivial files), C7 (forbidden files), C14 (naming conventions), C15
(directory layout) are all "assert something about paths". `ensure.file.exists` covers
one of these and has no read-only face.

## Config

```yaml
type: file
path: LICENSE                          # or
targets: "docs/superpowers/{plans,specs}/*.md"   # glob; engine fans out ({{engine-06}}) or plugin globs itself
state: present                          # present | absent | directory
min_size: 64                            # bytes; check only
contains: "MIT License"                 # substring; check only
matches: "^# .+"                        # regex; check only
name_matches: "^\\d{4}-\\d{2}-\\d{2}-[a-z0-9-]+\\.md$"   # basename regex; check only
template: templates/LICENSE.j2          # fix face: seed content when creating (engine-rendered)
mode: "0755"                            # optional
```

## Behaviour

- **check:** one finding per violated predicate per path; `absent` violations are
  `fixable: false` (we never delete user files); `directory`/`present` are fixable.
- **execute:** create the file (empty or from `template`) or directory; never truncate
  an existing file; set `mode` when given.

## Acceptance

- Examples: `39-file-required` (C6/C15) and `40-file-forbidden-and-naming` (C7/C14) —
  `stage: implemented`; the naming example flags a `.yml` file among `.yaml` siblings.
- `ensure.file.exists` remains as an alias to `type: file, state: present`.

## Depends on

{{plugins-01}}, {{plugins-02}}, {{plugins-03}}.

---

<!-- issue key=prims-06 repo=Web-tree/weaver labels=plugins,enhancement -->
# `content.scan` plugin: forbidden patterns, secret presets, hostile Unicode across globs

**Priority:** P2 · **Epic:** {{epic-prims}} · **Refs:** R15, C7, example 29

## Why

Example 29 designed `check.content_scan` (Trojan Source, invisible characters,
homoglyphs) but nothing implements it. The handoff adds committed credential files and
hardcoded tokens (C7). Check-only: there is no safe automatic fix.

## Config

```yaml
type: content.scan
targets: ["**/*", "!node_modules/**", "!.git/**"]
rules:
  - preset: secrets           # aws_access_key, github_token, private_key_block, generic_api_key
  - preset: unicode_bidi
  - preset: unicode_invisible
  - preset: homoglyphs
  - regex: "BEADS_DOLT_PASSWORD=\\S+"
    message: "credential in file"
allow:
  paths: ["fixtures/**"]
  codepoints: ["U+200B"]
  regex: ["EXAMPLE_KEY_[A-Z0-9]+"]
max_file_size: 1048576        # skip larger files with a `skip` finding
```

## Behaviour

- **check:** findings carry `path`, `location: "<line>:<col>"`, the rule/preset name,
  and a redacted `observed` (never echo a secret in full). Binary files skipped.
- **plan/execute:** `not-applicable`.

## Acceptance

- Example 29 promoted to `stage: implemented` with the documented stderr shape.
- New example `41-content-scan-secrets`.
- Unit tests per preset with true/false positives from example fixtures.

## Depends on

{{plugins-01}}, {{plugins-02}}, {{plugins-03}}.

---

<!-- issue key=prims-07 repo=Web-tree/weaver labels=engine,enhancement -->
# `command` check: `expect`, `stdout_contains`/`stdout_matches`, `timeout`, `cwd`; register examples 20 and 21

**Priority:** P1 · **Epic:** {{epic-prims}} · **Refs:** R20 (first half), FR-024, C13, C16

## Why

`CheckDef { name, command, description }` runs `sh -c` and treats exit 0 as pass
(`crates/cli/src/commands/check.rs`). No expected exit code, no output matching, no
timeout. Examples 20 and 21 use a `run:` key that does not deserialize and are not in
`examples/test-suite.yaml`.

## What

Built into core (the simplest rule must not require a plugin download):

```yaml
type: command                     # `checks:` entries without a type default to it
command: "task -l --json"
expect: 0                         # exit code (default 0)
stdout_contains: '"name":"lint"'
stdout_matches: '"lint"|"render"'
timeout: 30s
cwd: "."                          # relative to app path
remediation: "add a `lint` task to Taskfile.yml"
shell: sh                         # sh | none (argv split, no shell)
```

- Missing binary ⇒ status `error` (never `pass`), finding says which program.
- Timeout ⇒ `error`.
- `observed` carries the exit code and the first 512 bytes of stdout/stderr.
- Fix face: `not-applicable` (check-only type).

## Acceptance

- Examples 20 and 21 rewritten to `command:` and registered `stage: implemented` with
  `args: ["check"]` and `expected_exit_code: 2` on `before/`.
- Tests for `expect`, `stdout_matches`, timeout, missing binary ⇒ `error`.

## Depends on

None. Integrates with the report from {{engine-03}} when it lands.

---

<!-- issue key=prims-08 repo=Web-tree/weaver labels=plugins,enhancement -->
# `weaver-plugin-sdk` crate: bindings re-export, config helpers, findings builders, predicates

**Priority:** P2 · **Epic:** {{epic-prims}} · **Refs:** R18, C11

## Why

Eight plugins each hand-roll `generate!`, config parsing, and error mapping. The new
primitives share predicates (regex, semver range, "is pinned", enum membership) and
finding construction. Out-of-tree authors ({{plugins-04}}) need the same helpers.

## What

`crates/plugin-sdk` (published to crates.io as `weaver-plugin-sdk` once the WIT is
tagged):

- `weaver_plugin_sdk::prelude::*` — generated bindings for `weaver:plugin@0.2.0`, the
  `Guest` trait, request/result types.
- `Config::<T>::parse(&req)` → typed config with `config-error` mapping.
- `Finding::builder().path(..).location(..).expected(..).observed(..).fixable(..)`.
- `predicates`: `equals`, `contains`, `regex`, `semver(range)`, `pinned` (rejects
  `main`/`master`/`latest`/floating tags without a version), `one_of`; a `Predicate`
  enum deserializable from the `predicate:` YAML shape used by `json.key`/`yaml.key`.
- `workspace::path(req, rel)` — join under `/workspace`, reject escapes.
- Test helpers: `fake_request(config_json, workspace_dir)` for plugin unit tests.

## Acceptance

- Two in-tree plugins (`taskfile-task`, `json.key`) migrated to the sdk; behaviour
  unchanged.
- Predicate tests: semver ranges, `pinned` matrix (`@v7`, `@v7.1.0`, 40-char SHA pass;
  `@main`, `@latest`, missing `@` fail).

## Depends on

{{plugins-01}}.

---

<!-- issue key=epic-engine repo=Web-tree/weaver labels=epic,engine -->
# Epic: rules engine — module-shipped rules, metadata, report, exit codes, targeting

**Priority:** P0–P2 · **Design:** `docs/superpowers/specs/2026-09-09-plugin-first-standards-engine-design.md`

The engine is the opinion-free orchestrator. Today `wvr check` runs only workspace- and
app-level shell commands, prints a table, and bails; `plan` can never say "converged"
for section/template ensures; every command hardcodes `weaver.yaml` in cwd; a module
cannot live in a subdirectory. This epic turns `wvr check` into a trustworthy gate and
report, fed by rules that modules own.

## Issues

- [ ] {{engine-05}} — Bug: `plan()` reports changes on converged `md_section`/`from_template` (R5)
- [ ] {{engine-07}} — Global `-C`/`--repo` and one `Workspace` path resolver (R27)
- [ ] {{engine-09}} — Module `path:` subdirectory support (handoff open decision 3)
- [ ] {{engine-01}} — Module-shipped rules: `rules:` in manifests, inherited by `wvr check`, attributed to module@commit (R1–R3)
- [ ] {{engine-02}} — Rule metadata: `id`, `severity`, `profiles`, `when:`, consumer waivers (R6, R7, R23, R31)
- [ ] {{engine-03}} — `wvr check --json` report, typed exit codes, severity-derived gate (R22–R24, R32)
- [ ] {{engine-06}} — Glob targets with `every`/`any` quantifiers (R17)
- [ ] {{engine-10}} — Unified rule dispatch: any `type:` usable at app and module level (FR-026)
- [ ] {{engine-04}} — Foreign-repo validation: `wvr check --module <src> --ref <r> -C <path>` (R28)
- [ ] {{engine-08}} — `wvr check --fix`: apply exactly the auto-fixable failures (R25)

## Later

- Populate `FileState.owned_regions` and the brownfield conflict classifier (R9, FR-040/041).
- Fleet mode: one module against N repos, one aggregated report (R30).
- SARIF output for GitHub code scanning (R26).
- `deprecated`/`since` rule metadata with scheduled severity promotion (R34).

## Done when

A module declaring one rule, adopted by a repo whose `weaver.yaml` declares none, makes
`wvr check` exit 2 with a JSON report naming `module@<sha>/rule-id`, and exit 0 after
`wvr apply`.

---

<!-- issue key=engine-05 repo=Web-tree/weaver labels=engine,bug -->
# Bug: `plan()` reports changes on already-converged `md_section` and `from_template` ensures

**Priority:** P0 · **Epic:** {{epic-engine}} · **Refs:** R5, spec 004 SC-002, `FOLLOWUPS.md` Phase 2

## Why

`EnsureFileMdSection::plan` and `EnsureFileFromTemplate::plan` return a non-empty
`actions` list unconditionally (`crates/core/src/ensure/file.rs`). So `wvr plan
--detailed-exitcode` exits 2 on a perfectly conformant repo — there is no working gate
at all until this lands.

## What

- In both `plan()` implementations: read the target, compute the would-be bytes (reuse
  `upsert_*` / `render`), and report a change only when bytes differ. Missing target ⇒
  `create`.
- Also treat `files/` and `templates/` walking in `apply.rs` the same way: a file whose
  rendered bytes equal the current bytes is not a `create`.

## Acceptance

- Example `42-plan-converged-exit-0`: `apply` then `plan --detailed-exitcode` exits 0
  with an empty change list; edit the managed section, exit 2.
- Example 23's `after/` tree yields exit 0 on `plan --detailed-exitcode`.

## Notes

Once {{prims-03}} moves `md_section` into the `text.section` plugin, this logic moves
with it; fix it here first because it blocks every gate.

---

<!-- issue key=engine-07 repo=Web-tree/weaver labels=engine,enhancement -->
# Global `-C <path>` / `--repo` and a single `Workspace` path resolver

**Priority:** P0 · **Epic:** {{epic-engine}} · **Refs:** R27

## Why

Every command hardcodes `Path::new("weaver.yaml")`, `weaver.lock`, `.rw/state.yaml`,
`.rw/answers.yaml` relative to cwd (`apply.rs:54,70,113,334`, `check.rs:15`,
`describe.rs:24`, `list.rs:24`, `run.rs:28`, `module.rs:65,107,140,165`,
`plugins.rs:46,90,145,213`). CI, fleet runs, and foreign-repo validation all need to
point `wvr` at another directory.

## What

- `weaver_core::workspace::Workspace { root, config_path, lock_path, state_path,
  answers_path, plan_dir }` with `Workspace::discover(root: Option<PathBuf>)`.
- Global clap args `-C/--repo <path>` (also `WEAVER_ROOT`); relative `--plan`/`--out`
  paths resolve against the invocation cwd, everything else against `root`.
- All commands take a `&Workspace`; `PluginResolver::new(root)` and
  `ModuleResolver` receive it instead of `PathBuf::from(".")`.

## Acceptance

- Test: `wvr -C /tmp/x plan` on a workspace in `/tmp/x` from another cwd behaves
  identically to running inside it, for `plan`, `apply`, `check`, `describe`, `list`,
  `run`, `module list`.
- `grep -rn 'Path::new("weaver.yaml")' crates/cli` returns nothing.

---

<!-- issue key=engine-09 repo=Web-tree/weaver labels=engine,enhancement -->
# Module `path:` subdirectory support (`ModuleConfig.path`, `wvr module add --path`)

**Priority:** P0 · **Epic:** {{epic-engine}} · **Refs:** handoff open decision 3, marketplace design G4, beads design C1

## Why

`ModuleConfig.path` is parsed but never read (`config.rs`, `module.rs`); every module
must be a git repo **root**. That forces a standards module into its own repository
even when it belongs next to the standard it encodes.

## What

- `ModuleResolver::resolve` returns `<cache>/<commit>/<path>` when `path` is set;
  validates the directory exists and contains `weaver.module.yaml` or `files/` or
  `templates/`; error names the available candidates otherwise.
- `weaver.lock` `ModuleLock` records `path`.
- `wvr module add <src> --path <sub>` writes it; `describe --json` shows it.
- The examples harness (`prepare_module_sources`) supports `path:` so an example can
  keep several modules in one local repo.

## Acceptance

- Example `43-module-in-subdirectory`: one local git repo with `modules/a` and
  `modules/b`; two apps consume them via `path:`; `stage: implemented`.
- Lock round-trip test with `path`.

---

<!-- issue key=engine-01 repo=Web-tree/weaver labels=engine,enhancement -->
# Module-shipped rules: `rules:` in manifests, inherited by `wvr check`, attributed to module@commit

**Priority:** P1 · **Epic:** {{epic-engine}} · **Refs:** R1, R2, R3, FR-006, beads design C5

## Why

`ModuleManifest` has `ensures` but no `checks` (`config.rs`); `wvr check` reads only
`config.checks` + `app.checks` (`check.rs`), with a comment conceding the omission. So
every consuming repo copies the rule list and the standard has no single source of
truth. The handoff calls this the highest-value item.

## What

1. Manifest: canonical `rules: Vec<RuleEntry>`; `ensures:` and `checks:` are still
   accepted and merged into `rules` (entries from `checks:` default to
   `type: command`). App-level `rules:`/`ensures:`/`checks:` likewise.
2. `RuleEntry { meta: RuleMeta (see {{engine-02}}), type_name, config }` — a single
   representation for built-in and plugin types.
3. `wvr check [app]` evaluates, per app: module rules (in manifest order), then app
   rules, then workspace rules. For each rule it calls the `check` face
   ({{plugins-01}}); built-in types use their derived `check`.
4. Every result records `module: { name, source, resolved_commit, path }` and the
   rule's `id` (defaulting to `<type>#<index>` when not set, with a `warn` that ids
   should be explicit in modules).
5. `wvr describe --json` lists effective rules with their origin.

## Acceptance

- Example `44-module-shipped-rules`: module declares a `command` rule and a `file`
  rule; consumer `weaver.yaml` declares no checks; `wvr check` exits 2 on `before/` and
  the table shows `dev-module@<sha7>` as the source; `after/` exits 0.
- Test: workspace-, app-, and module-level rules all appear once each.

## Depends on

{{plugins-01}} (check face) for plugin rules; `command`/`file` rules work without it.

---

<!-- issue key=engine-02 repo=Web-tree/weaver labels=engine,enhancement -->
# Rule metadata: `id`, `severity`, `profiles`, `when:`, and consumer waivers

**Priority:** P1 · **Epic:** {{epic-engine}} · **Refs:** R3, R6, R7, R23, R31, R34, FR-012, FR-013, C18

## What

`RuleMeta` flattened onto every rule entry:

```yaml
id: workflow-actions-pinned        # required in modules; module-scoped
severity: error                    # error | warn | info (default error)
profiles: [infra-k8s, api]         # empty = all
when: "{{ features.ci }}"          # Tera expression → bool
description: "..."
remediation: "..."                 # overrides the plugin's finding remediation
since: "1.3.0"                     # optional, informational
```

Consumer side (`weaver.yaml`):

```yaml
apps:
  - name: repo
    module: dev-standards
    profiles: [infra-k8s]          # explicit selection (no inference)
    waivers:
      - rule: dev-standards/workflow-actions-pinned
        reason: "renovate migrates these by 2026-12"
        expires: 2026-12-31
```

- `when:` evaluated by rendering `{{ … }}` with the app Tera context plus `profiles`
  (list) and `profile` helpers; anything but `true` ⇒ `skip`. Render errors ⇒ `error`.
- Profile mismatch ⇒ `skip`. A waiver ⇒ status `waived` (reason and expiry in the
  report); an expired waiver is ignored with a `warn` line. `reason` is required.
- `enabled: false` on a consumer override of a module rule is sugar for a waiver with
  `reason: "disabled"`.
- `wvr apply`/`plan` honour `when:`, profiles, and waivers identically (a waived rule is
  neither checked nor fixed).

## Acceptance

- Example `45-rule-profiles-when-waivers`: three rules, two profiles, one waiver;
  report shows `pass`/`skip`/`waived` correctly and the summary counts exclude
  `skip`/`waived` from the score.
- Test: `skip` never inflates the score; `waived` is never printed as `pass`.
- Test: expired waiver ⇒ rule evaluated, warning printed.

## Depends on

{{engine-01}}.

---

<!-- issue key=engine-03 repo=Web-tree/weaver labels=engine,enhancement -->
# `wvr check --json` report, typed exit codes, severity-derived gate

**Priority:** P1 · **Epic:** {{epic-engine}} · **Refs:** R22, R23, R24, R32, FR-005

## Why

`wvr check` prints a `comfy_table` and bails with a count; the global `--json` only
switches log format. There is no way for CI, a dashboard, or an agent to consume
results, and exit codes are whatever `anyhow` produces.

## What

1. `--format table|json` on `check` (and `plan`); `--json` global flag implies
   `--format json` for result output. Schema in the design doc: `format_version`,
   `target`, `rules[]` (id, type, module, resolved_commit, profile, severity, status,
   fixable, findings[] with path/location/message/expected/observed/remediation),
   `summary` (counts by status and severity, `score` = pass / (pass + fail) over
   `error`+`warn` rules).
2. Exit codes everywhere: `0` conformant/converged, `1` user error (bad config, unknown
   app), `2` violations or changes (`check`; `plan --detailed-exitcode`), `3` system
   error (IO, git, network), `4` plugin error (load, checksum, trap). Implemented as a
   `WeaverError` enum with `exit_code()`; `main` maps it.
3. Gate rule: exit 2 only when at least one `fail` has `severity: error`; `warn`/`info`
   failures never change the exit code. `--fail-on warn` lowers the threshold.
4. Table output gains columns: severity, status, source (`module@sha7`), and prints
   remediation under each failing row.
5. `--output <file>` writes the JSON report.

## Acceptance

- Example `46-check-json-report` with `custom_assertions` on the JSON (status per rule,
  summary counts) and `expected_exit_code: 2`.
- Tests for each exit code path, including "missing tool ⇒ status error ⇒ exit 2"
  (an unevaluable `error`-severity rule fails the gate; it never passes).
- JSON output is deterministic (rules ordered by module, then declaration order; no
  timestamps unless `--timestamps`).

## Depends on

{{engine-01}}, {{engine-02}}.

---

<!-- issue key=engine-06 repo=Web-tree/weaver labels=engine,enhancement -->
# Glob targets with `every`/`any` quantifiers, one finding per violating path

**Priority:** P2 · **Epic:** {{epic-engine}} · **Refs:** R17, C10, C11, C14

## Why

"Every workflow must pin its actions" is a file-set rule, not a file rule. Plugins take
one `file`; the engine should fan out.

## What

- Any rule may set `targets: <glob | [globs]>` (negations with `!`) instead of `file:`.
- Engine expands globs under the app path (deterministic sort, `.git` and `.rw`
  excluded by default), calls the plugin once per match with `file` set, and
  aggregates:
  - `quantifier: every` (default) — all matches must pass; findings list each failing
    path.
  - `quantifier: any` — at least one must pass; on failure the findings list every
    candidate with its own reason.
  - zero matches ⇒ `fail` with "no files matched" unless `allow_empty: true` ⇒ `skip`.
- `plan`/`execute` fan out the same way (fix every matching file).
- Plugins that glob internally (`content.scan`, `file`) declare it and are not fanned
  out twice (a `targets_mode: plugin` hint in the bundled table).

## Acceptance

- Example `47-glob-every-any`: `yaml.key` with `predicate.pinned` over
  `.github/workflows/*.y*ml` (`every`) and a `file` rule with `any`;
  `stage: implemented`.
- Test: a 3-file glob with one violation ⇒ one finding naming that file.

## Depends on

{{engine-01}}, {{engine-03}}.

---

<!-- issue key=engine-10 repo=Web-tree/weaver labels=engine,enhancement -->
# Unified rule dispatch: any `type:` usable at app level and module level

**Priority:** P2 · **Epic:** {{epic-engine}} · **Refs:** FR-026, spec 004 US-6

## Why

App-level `ensures:` deserialize into `EnsureSpec` (only `ensure.npm.*`,
`ensure.file.*`), module-level into `EnsureEntry` (`Known | Plugin`). A consumer repo
cannot use a plugin type (or `git.submodule`) without wrapping it in a module, and
`apply.rs` runs two different loops. "Modules are opinionated, plugins are universal"
only holds if plugins are reachable without a module.

## What

- One `RuleEntry` (from {{engine-01}}) for both levels; `EnsureSpec` and `EnsureEntry`
  removed. Legacy type names (`ensure.file.exists`, `ensure.npm.script`, …) kept as
  aliases in a single alias table.
- One dispatcher: `build_rule(entry, ctx, plugin_table) -> Box<dyn Rule>` with
  `plan`/`execute`/`check`.
- `apply.rs` runs a single loop over `module.rules ++ app.rules`, and the `npm.*`
  native path becomes ordinary built-in rules.
- `describe --json` includes app-level rules (closes the `FOLLOWUPS.md` item).

## Acceptance

- Example `48-plugin-type-at-app-level`: consumer `weaver.yaml` with no module uses
  `type: taskfile.task` and `type: json.key` directly; `stage: implemented`.
- All existing examples stay green (aliases work).

## Depends on

{{engine-01}}, {{plugins-03}}.

---

<!-- issue key=engine-04 repo=Web-tree/weaver labels=engine,enhancement -->
# Foreign-repo validation: `wvr check --module <src> --ref <r> [--path p] [--profile x] -C <dir>`

**Priority:** P2 · **Epic:** {{epic-engine}} · **Refs:** R28, R8

## Why

Most repos we want to grade have not adopted Weaver. Requiring `weaver.yaml` and `.rw/`
first inverts the funnel. `check` must also be provably read-only (R8).

## What

- When `--module` is given, synthesize an in-memory `WeaverConfig` with one module and
  one app at `.` (`--app-path` to override), `profiles` from `--profile`, inputs from
  `--input k=v` (repeatable) or defaults; error on missing required inputs listing them.
- No file is written under the target: no `weaver.lock`, no `.rw/`. Module and plugin
  caches live under `~/.rw` as today; `--offline` respected.
- `check` (with or without `--module`) opens the target read-only; any write attempt
  by core is a bug caught by a test that runs against a read-only checkout.

## Acceptance

- Test: run `check --module <local git> -C <clean clone>` against a read-only
  directory; the command exits 0/2 on rule merits and the tree is byte-identical
  afterwards.
- Test: `check` with `--module` never creates `.rw/` or `weaver.lock`.

## Depends on

{{engine-07}}, {{engine-01}}, {{engine-03}}.

---

<!-- issue key=engine-08 repo=Web-tree/weaver labels=engine,enhancement -->
# `wvr check --fix`: apply exactly the auto-fixable failing rules from the same declarations

**Priority:** P2 · **Epic:** {{epic-engine}} · **Refs:** R25

## What

- `wvr check --fix [--dry-run]` runs `check`; for every `fail` whose findings are all
  `fixable: true` and whose type has a fix face, calls `plan` then `execute`; re-checks;
  reports `fixed`/`still-failing`/`not-fixable` per rule in table and JSON.
- Never touches rules that pass, are skipped, or are waived; never runs `files/` and
  `templates/` walking (that is `apply`).
- Exit code after `--fix`: 0 if everything now passes, 2 if anything still fails.

## Acceptance

- Example `49-check-fix`: two fixable failures and one `command` failure; after
  `check --fix` the JSON shows two `fixed`, one `not-fixable`, exit 2.

## Depends on

{{engine-01}}, {{engine-02}}, {{engine-03}}, {{plugins-01}}.

---

<!-- issue key=epic-modules repo=Web-tree/weaver labels=epic,modules -->
# Epic: module authoring in separate repositories — last-mile delivery kit

**Priority:** P2 · **Design:** `docs/superpowers/specs/2026-09-09-plugin-first-standards-engine-design.md`

Modules are the opinionated tier and live outside this repo. Authoring one today means
reverse-engineering the engine (see `docs/beads-module-handoff.md` §2), committing every
iteration so `git ls-remote` can see it, and hand-writing checks in every consumer. This
epic gives module authors a scaffold, a test harness, and CI they can call from their
own repo — generic for any module, not only dev-standards.

## Issues

- [ ] {{modules-01}} — `wvr module new` scaffold + module authoring guide + `.wt/weaver/manifest.yaml`
- [ ] {{modules-02}} — `wvr module test`: before/after fixture harness for external module repos + reusable CI workflow

## First consumer (tracked in `Web-tree/webtree-dev-standards-k8s`)

- {{ds-01}} — rule catalog and decisions for the dev-standards module
- {{ds-02}} — `weaver-module/` v0 built only from plugin types
- {{ds-03}} — self-dogfood: `weaver.yaml` + CI gate in the standards repo

## Done when

A new module repo (or subdirectory) scaffolded with `wvr module new` has a passing
`wvr module test` in its own CI, and a consumer adopts it with `wvr module add`.

---

<!-- issue key=modules-01 repo=Web-tree/weaver labels=modules,documentation -->
# `wvr module new` scaffold, module authoring guide, and self-describing manifest

**Priority:** P2 · **Epic:** {{epic-modules}} · **Refs:** marketplace design G7, spec 004 US-6

## What

1. `wvr module new <name> [--dir <path>] [--profiles a,b]` writes:
   ```
   weaver.module.yaml        # inputs, one example rule per face type, profiles comment
   templates/README.md.j2
   sections/example.md.j2    # for text.section rules
   tests/cases/basic/{before,after,weaver.yaml}   # for {{modules-02}}
   .wt/weaver/manifest.yaml  # discovery metadata (marketplace design §A)
   README.md                 # adoption steps for consumers
   ```
2. `docs/MODULE_AUTHORING.md`: the engine-accurate model (what is templated and what is
   not, module-relative paths, `path:` subdirectories, how rules map to plugin types,
   profiles/`when:`/waivers, versioning with tags, how a consumer pins), replacing the
   folklore currently spread across `beads-module-handoff.md` §2.
3. `wvr module validate [<dir>]`: parses the manifest, checks every rule `type` resolves
   to a plugin (declared or bundled), every `content_from_template`/`template` path
   exists, ids are unique, profiles referenced by `when:` are declared.

## Acceptance

- Scaffolded module passes `wvr module validate` and `wvr module test` out of the box.
- The guide is used to author the dev-standards module without reading engine source
  (feedback loop: gaps found there become follow-up issues).

## Depends on

{{engine-01}}, {{engine-02}}, {{engine-09}}.

---

<!-- issue key=modules-02 repo=Web-tree/weaver labels=modules,enhancement -->
# `wvr module test`: before/after fixture harness for external module repos + reusable CI workflow

**Priority:** P2 · **Epic:** {{epic-modules}}

## Why

`crates/cli/tests/examples_suite.rs` proves primitives inside this repo, but a module in
another repository has no way to run the same before/after snapshot discipline against
itself. Module authors need "apply on `before/` equals `after/`, check on `after/`
exits 0, check on `before/` exits 2" as a one-command test.

## What

1. `wvr module test [<module-dir>] [--case <name>] [--update]`: for each
   `tests/cases/<name>/` with `before/`, `after/`, and `weaver.yaml` (consumer config
   whose module `source:` is the module under test), copy `before/` to a temp dir, run
   `check` (expect exit 2 unless `expect_pass: true` in `case.yaml`), run `apply`, diff
   against `after/` (ignore `weaver.lock`, `.rw/`), run `check` again (expect 0), run
   `plan --detailed-exitcode` (expect 0). `--update` rewrites `after/`.
2. The examples harness in this repo reuses the same core so behaviour cannot drift.
3. Reusable workflow `.github/workflows/module-ci.yml` (`workflow_call`): installs `wvr`
   (pinned), runs `wvr module validate` and `wvr module test`, uploads the JSON
   reports. Module repos call it with three lines.

## Acceptance

- The scaffold from {{modules-01}} passes `wvr module test` in a fresh clone.
- A test in this repo runs `wvr module test` against a fixture module with one failing
  case and asserts the diff output names the file.

## Depends on

{{modules-01}}, {{engine-03}}, {{engine-07}}.

---

<!-- issue key=ds-01 repo=Web-tree/webtree-dev-standards-k8s labels=enhancement,documentation -->
# Dev-standards module: rule catalog, profiles, and adoption decisions

**Priority:** P2 · **Refs:** handoff `docs/handoffs/weaver-dev-standards-module-requirements.md` §3, §8

## Why

Before authoring the module, the standard itself needs to be written as rules: which
case (C1–C18), which plugin type, which profile, which severity, and what the
remediation says. This is the module's spec and the place to close the handoff's open
decisions 4, 5, 6.

## What

`docs/superpowers/specs/<date>-dev-standards-module-rules.md` containing:

1. **Profiles**: `infra-k8s`, `api`, `web`, `worker-go`, `worker-ts`, `library` — what each
   means and which repos in the registry map to which.
2. **Rule table**: one row per rule — `id`, case, plugin type + config sketch, profiles,
   severity, fixable?, remediation. Every case C1–C16 covered; C17 explicitly deferred.
3. **Decisions**: profile selection is explicit per repo; baseline strategy (`warn`-first
   promotion vs generated waivers); where AGENTS.md prose lives (module templates).
4. **Versioning**: tag scheme, how consumers pin, how a new rule ships (`warn` first,
   promotion date).

## Acceptance

- Reviewed and merged before {{ds-02}} starts.
- Each rule row names a plugin type that exists or is scheduled in `Web-tree/weaver`
  (`json.key`, `yaml.key`, `text.section`, `text.lines`, `file`, `content.scan`,
  `command`, `taskfile.task` — see {{epic-prims}}).

---

<!-- issue key=ds-02 repo=Web-tree/webtree-dev-standards-k8s labels=enhancement -->
# Dev-standards module v0 at `weaver-module/`, built only from plugin types

**Priority:** P2 · **Refs:** {{ds-01}}; weaver: {{engine-09}} (module `path:`), {{engine-01}} (module-shipped rules), {{engine-02}} (rule metadata)

## What

Author `weaver-module/` in this repo (a subdirectory; the engine resolves it via
`path: weaver-module`):

```
weaver-module/
  weaver.module.yaml      # inputs, profiles, rules (from the catalog), plugins (if any non-bundled)
  sections/*.md.j2        # AGENTS.md managed sections (C1, C2)
  templates/*.j2          # required files seeded on first apply (LICENSE, renovate.json, actionlint.yaml)
  tests/cases/*/          # before/after fixtures per profile
  .wt/weaver/manifest.yaml
  README.md
```

Ship in two steps so value lands early:

- **v0.1 — generation with today's engine**: `text.section` for C1/C2, `file` for
  C6/C15, `json.key` for C3/C4/C5, `text.lines` for C8, `taskfile.task` for C13. Works as
  soon as the primitive plugins and `path:` support exist.
- **v0.2 — validation**: add `severity`, `profiles`, `when:`, `command` rules for C13/C16,
  `yaml.key` + globs for C9–C11, `file` naming rules for C14, `content.scan` for C7.

No org-specific code goes into `Web-tree/weaver`; anything the module needs that the
engine lacks becomes a generic issue there.

## Acceptance

- `wvr module validate weaver-module` and `wvr module test weaver-module` pass in CI.
- Every rule in the catalog is present with its `id`; ids are stable across v0.1 → v0.2.
- A clean clone of another Web-tree repo graded with
  `wvr check --module https://github.com/Web-tree/webtree-dev-standards-k8s.git --ref v0.2.0 --path weaver-module --profile api -C <clone>`
  produces a JSON report and writes nothing.

## Depends on

{{ds-01}}; weaver {{engine-09}}, {{engine-01}}, {{engine-02}}, and the primitive plugins
{{prims-01}}, {{prims-02}}, {{prims-03}}, {{prims-04}}, {{prims-05}}, {{prims-06}} (see {{epic-prims}}).

---

<!-- issue key=ds-03 repo=Web-tree/webtree-dev-standards-k8s labels=enhancement,devops -->
# Self-dogfood: this repo consumes its own module, `wvr check` gates PRs

**Priority:** P2 · **Refs:** {{ds-02}}; weaver: {{engine-03}} (JSON report + exit codes), {{engine-07}} (`-C`)

## What

1. `weaver.yaml` at the repo root: `modules: [{ name: dev-standards, source: ".",
   ref: HEAD, path: weaver-module }]`, one app at `.` with `profiles: [infra-k8s]`, and a
   waivers block for anything intentionally non-conformant (with reasons).
2. `.github/workflows/standards.yaml`: installs a pinned `wvr`, runs
   `wvr check --format json --output report.json` and `wvr plan --detailed-exitcode`;
   uploads the report; fails the PR on exit 2. Triggers: `pull_request`, `push: main`;
   `concurrency.cancel-in-progress: true` (the workflow itself must satisfy C10).
3. Fix or waive every current violation so `main` is green on day one; the waiver list
   is the initial baseline and each entry has an expiry.
4. `Taskfile.yml` gains `standards:check` and `standards:fix` (`wvr check --fix`).

## Acceptance

- `main` is green; a PR that removes a required `.gitignore` line or unpins an action
  fails with a readable remediation in the job log.
- Report artifact is attached to every run.

## Depends on

{{ds-02}}.
