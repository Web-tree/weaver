# Design: plugin-first standards engine (validate + converge from one rule)

**Date:** 2026-09-09
**Status:** proposed — issue breakdown in
`docs/superpowers/plans/2026-09-09-standards-engine-issues.md`
**Source:** `webtree-dev-standards-k8s` handoff
`docs/handoffs/weaver-dev-standards-module-requirements.md` (R1–R34, cases C1–C18)
**Complements:** `specs/004-generic-standardization-engine/spec.md` (FR-005/006/012–014/
022–026/030–032/040), `docs/superpowers/specs/2026-07-23-weaver-marketplace-design.md`

## Goal

Make Weaver able to **judge** a repo, not only generate one, with every rule authored
once and serving three consumers: a CI gate (yes/no), a human report (which rules fail,
how badly, how to fix), and a fixer (`wvr apply` / `wvr check --fix`). Do it so that the
low-level work lives in **universal WASM plugins** that any module in any repo can
compose, and the opinions live in **modules** shipped from separate repositories.

## Layering contract (locked)

| Tier | Where it lives | Opinionated? | Examples |
|---|---|---|---|
| **Plugins** — universal workers | mostly `Web-tree/weaver/plugins/*`, released per tag as `plugin.wasm`; also any third-party repo | never | `json.key`, `yaml.key`, `text.section`, `file`, `content.scan`, `taskfile.task` |
| **Engine** — `wvr` | `crates/core`, `crates/cli` | never | resolve modules/plugins, run plan/apply/check, report, exit codes |
| **Modules** — last-mile delivery | separate repos (or a subdirectory of the repo whose standard they encode) | yes | `webtree-dev-standards-k8s/weaver-module/` |

A plugin exposes a **type** (`json.key`) and a JSON config contract. A module is data:
a list of rules, each `type:` + config + metadata. Consumers adopt modules; they can also
use plugin types directly in `weaver.yaml`.

## Decisions

| # | Decision | Choice | Why |
|---|---|---|---|
| D1 | Where primitives live | **WASM plugins** with filesystem access, in `plugins/` | Hub architecture doc already promises this; makes third-party primitives possible; keeps core opinion-free |
| D2 | Filesystem access for plugins | **WASI preopens**, not a custom WIT `fs` interface | `wasmtime-wasi` already supports `preopened_dir(host, guest, DirPerms, FilePerms)`; plugins use `std::fs`; host grants **read-only** for `plan`/`check` and read+mutate for `execute` — R8 (check is read-only) is enforced by the sandbox, not by convention |
| D3 | Rule surface | **One declaration, two faces**: every ensure plugin exports `plan`/`execute`/`check` | Prevents the check-list and fix-list from drifting (handoff open decision 1) |
| D4 | Rule list in manifests | New canonical `rules:`; `ensures:` and `checks:` stay accepted and are merged into it | Backward compatible; `checks:` entries are check-only by definition |
| D5 | Conditionals | `when:` is a Tera expression over inputs + `profile` rendered to a bool | Reuses the templating engine that already exists; no new expression language |
| D6 | Structural queries | Done **inside plugins** with serde (`json.key`, `yaml.key`), not by shelling to `jq`/`yq` | Resolves the "native tool first" vs "no false green" tension: plugins are the sanctioned place for format parsers; no runner tool dependency |
| D7 | Bundled plugin resolution | Built-in table of bundled plugin names → `Web-tree/weaver` release tags, pinned per `wvr` version | Replaces the nonexistent registry default; keeps "plugins are mostly part of weaver" true |
| D8 | Third-party plugins | Declared in `plugins:` (consumer `weaver.yaml` **or** module manifest) with `git`+`ref` (release asset) or `path`; `provides:` lists the types | FR-014/FR-030; dispatch must consult declarations, not only the type name |
| D9 | Module location | Subdirectory support via `ModuleConfig.path` | Lets the standards module live next to the standard it encodes (handoff open decision 3) |
| D10 | Exit codes | `0` conformant, `1` user error, `2` violations/changes, `3` system error, `4` plugin error | FR-005; only `error`-severity failures set `2` |
| D11 | Baseline strategy | Not decided here — module-level concern; the engine supports both `warn`-first and waiver baselines | Handoff open decision 5 stays with the module author |

## Architecture

### WIT contract `weaver:plugin@0.2.0`

```wit
package weaver:plugin@0.2.0;

interface types {
    enum check-status { pass, fail, skip, error }

    record finding {
        path: option<string>,        // workspace-relative
        location: option<string>,    // "line:col" | key-path | glob member
        message: string,
        expected: option<string>,
        observed: option<string>,
        remediation: option<string>,
        fixable: bool,
    }

    record check-result {
        status: check-status,
        message: string,
        findings: list<finding>,
    }

    enum change-action { create, update, remove }

    record planned-change {
        action: change-action,
        path: string,
        preview: option<string>,     // unified diff or short description
    }

    record plan-result {
        converged: bool,
        description: string,
        changes: list<planned-change>,
    }

    record ensure-request {
        workspace: string,           // guest path of the preopened app root, e.g. "/workspace"
        host-app-path: string,       // host path, only for process.exec cwd
        dry-run: bool,
        config: string,              // JSON: {"type": "...", ...rule config}
    }

    variant ensure-error {
        config-error(string),
        execution-error(string),
        not-applicable(string),
        not-fixable(string),
    }
}

interface ensures {
    use types.{ensure-request, plan-result, check-result, ensure-error};
    plan:    func(req: ensure-request) -> result<plan-result, ensure-error>;
    execute: func(req: ensure-request) -> result<string, ensure-error>;
    check:   func(req: ensure-request) -> result<check-result, ensure-error>;
}

world ensure-provider {
    import process;                  // unchanged, gains a timeout field
    export ensures;
}
```

Filesystem access is **not** a WIT import: the host preopens the app root at `workspace`
through `wasi:filesystem`, which `wasmtime_wasi::p2::add_to_linker_sync` already links.

| Face | Preopen permissions | May call `process.exec`? |
|---|---|---|
| `check` | `DirPerms::READ`, `FilePerms::READ` | yes (rule may probe a native tool) |
| `plan` | `DirPerms::READ`, `FilePerms::READ` | yes |
| `execute` | `READ \| MUTATE`, `READ \| WRITE` | yes |

Path escape (`..`, absolute host paths) is rejected by WASI; a test proves it.

### Plugin resolution

```
type "json.key"
  ├─ declared in module manifest plugins:{..., provides:[json.key]}  → that source
  ├─ declared in consumer weaver.yaml plugins:                        → that source
  ├─ bundled table (plugins/bundled.yaml compiled into wvr)          → Web-tree/weaver release
  └─ none → error, exit 4: "no plugin provides type 'json.key'; declare it under plugins:"
```

Sources: `git` + `ref` (GitHub release asset `plugin.wasm`, existing fetcher), `path`
(a directory holding `plugin.wasm`, relative to the declaring file). `weaver.lock`
records `sha256` and it is verified on every load; `--offline` refuses to fetch.

The local-development fallback (`plugins/<name>/plugin.wasm` up to two directories up)
stays, gated behind `WEAVER_DEV_PLUGINS=1`, and is logged when used.

### Rule model

```yaml
# weaver.module.yaml (module) — also valid under apps[].rules in weaver.yaml
rules:
  - id: agents-md-house-rules          # stable, module-scoped; report shows <module>/<id>
    type: text.section
    severity: error                    # error | warn | info   (default error)
    profiles: [infra-k8s, api, web]    # empty/absent = all profiles
    when: "{{ features.agents_md }}"   # Tera expression → bool; false ⇒ status skip
    description: "AGENTS.md carries the non-interactive shell rules"
    remediation: "Run `wvr apply` or paste the section from the standard"
    # --- type-specific config follows ---
    file: AGENTS.md
    selector: { type: heading, path: ["Non-Interactive Shell Commands"], depth: 2 }
    content_from_template: sections/non-interactive.md.j2

plugins:                               # plugins this module needs (FR-014)
  acme-lint:
    git: https://github.com/acme/weaver-plugin-lint.git
    ref: v1.2.0
    provides: [acme.lint]
```

```yaml
# weaver.yaml (consumer)
apps:
  - name: repo
    module: dev-standards
    path: "."
    profiles: [infra-k8s]
    waivers:
      - rule: dev-standards/workflow-extension
        reason: "legacy workflows migrate in Q4 2026"
        expires: 2026-12-31
```

Status vocabulary: `pass`, `fail`, `skip` (`when:` false or profile mismatch), `waived`,
`error` (could not evaluate — never counts as pass). Only `fail` with `severity: error`
turns the gate red.

Glob targets: a rule whose `file:`/`targets:` is a glob is fanned out by the engine, one
plugin call per match, aggregated with `quantifier: every | any` (default `every`);
each violating path is its own finding.

### Report and exit codes

`wvr check --json` emits:

```json
{
  "format_version": "1",
  "target": { "root": "/abs/path", "modules": [{ "name": "dev-standards", "source": "...", "resolved_commit": "…", "path": "weaver-module" }] },
  "rules": [
    { "id": "dev-standards/agents-md-house-rules", "type": "text.section", "severity": "error",
      "profile": "infra-k8s", "status": "fail", "fixable": true,
      "findings": [{ "path": "AGENTS.md", "location": "heading:Non-Interactive Shell Commands",
                     "message": "section missing", "expected": "…", "observed": null,
                     "remediation": "…" }] }
  ],
  "summary": { "pass": 41, "fail": 3, "skip": 6, "waived": 1, "error": 0,
               "by_severity": { "error": 2, "warn": 1, "info": 0 }, "score": 0.93 }
}
```

`--format table|json|sarif` (`sarif` later). Exit codes per D10 across **all** commands.

### Targeting

A single `Workspace` resolver (`root`, `weaver.yaml`, `weaver.lock`, `.rw/state.yaml`,
`.rw/answers.yaml`) replaces the hardcoded `Path::new("weaver.yaml")` in every command;
`-C <path>` / `WEAVER_ROOT` set the root. `wvr check --module <src> --ref <r> -C <path>`
synthesizes a one-app config in memory for repos that never adopted Weaver and writes
nothing.

### Modules in a subdirectory

`ModuleConfig.path` is honored after clone (`<cache>/<commit>/<path>`); `wvr module add
<src> --path <sub>` writes it; the lock records it. This unblocks
`webtree-dev-standards-k8s/weaver-module/`.

## Sequencing

Phases map to the handoff's slices; issues inside a phase are independent unless stated.

| Phase | Outcome | Issues (see catalog) |
|---|---|---|
| **0 — unblock** | plan says "converged"; `-C`; module `path:` | engine-05, engine-07, engine-09 |
| **1 — plugin platform** | `check` face, structured plan, fs sandbox, declared-source resolution, out-of-tree template | plugins-01, plugins-02, plugins-03, plugins-04 |
| **2 — a gate that can say yes** | module-shipped rules, metadata, JSON report, exit codes, command check upgrades | engine-01, engine-02, engine-03, prims-07 |
| **3 — declarative rules** | json/yaml/section/lines/file/scan plugins + sdk + glob fan-out + unified dispatch | prims-01…06, prims-08, engine-06, engine-10 |
| **4 — adoption** | foreign-repo validation, `--fix`, module authoring kit, dev-standards module v0 + dogfood | engine-04, engine-08, modules-01, modules-02, ds-01…03 |
| **later** | owned regions, fleet mode, SARIF, `describe` export, cross-selector equality | tracked in epics |

## Out of scope

- GitHub-API rules (branch protection) — needs a credentialed plugin and a trust model.
- AI-assisted evaluation as an assertion (`ai.patch` stays a fix path only).
- Marketplace resolver (`use: name@ref`) — separate design, nothing here depends on it.
- Auto-opened remediation PRs.

## Open decisions left to the module author

- Profile detection: explicit per repo (recommended) with inference as an onboarding hint.
- Baseline: ship as `warn` and promote, or generate a per-repo waiver baseline.
- Where the AGENTS.md prose lives: module templates (recommended, so C1/C2 are
  `text.section` rules with `content_from_template`).
