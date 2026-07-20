# Handoff: `beads` repo-weaver module

**Goal.** A repo-weaver module that onboards any repo to the team's shared **bd
(beads)** issue tracker — which runs on **Dolt exposed over Tailscale**. Running
the module produces a working `.envrc` (the DB password is pulled from 1Password,
never set by hand), points `bd` at the shared server, and declares the
prerequisites. Onboarding a new repo becomes `rw apply` + one task — no
`.envrc.example` to copy, nothing pasted by hand.

**Audience.** Whoever implements the module (human or agent). Assumes access to
the repo-weaver codebase. Engine citations are `file:line` into this repo.

**Why this exists.** The current onboarding (see the `argocd-apps` repo,
`apps/dolt/README.md`) ships an `.envrc.example` that each repo copies. That's
fine for one repo; across many repos it's boilerplate. This module makes the
`.envrc` (and the `bd init`) a first-class, versioned, re-appliable artifact.

---

## 1. Target end-state (what "onboarded" means)

After the module is applied to a repo, that repo has:

1. **`.envrc`** that exports `BEADS_DOLT_PASSWORD`, resolved from **1Password**
   (`op://WebTreeShared/dolt-beads/password`) with a **kubectl fallback** for
   cluster admins. `bd` has **no on-disk password store** — the password *must*
   be in the environment, so this is the whole mechanism.
2. **`bd` pointed at the shared server** — `dolt.stoat-pain.ts.net:3306`, user
   `beads`, on its board (per-repo DB or the shared `beads` DB — see §5).
3. **Prerequisites** expressed as checks: `direnv` installed + shell-hooked,
   1Password access to the `WebTreeShared` vault (or kubectl), on the tailnet,
   `bd` installed, `.beads/` initialized.
4. **`direnv allow`** run, so the password loads on every `cd`.

### Org constants (current, as of this handoff)

| Thing | Value |
|---|---|
| Dolt SQL endpoint (tailnet) | `dolt.stoat-pain.ts.net:3306` |
| MySQL user | `beads` |
| Shared DB password (1Password) | `op://WebTreeShared/dolt-beads/password` |
| Shared DB password (kubectl fallback) | `kubectl get secret dolt-secret -n dolt -o jsonpath='{.data.beads-password}' \| base64 -d` |
| bd env var | `BEADS_DOLT_PASSWORD` |
| Tailscale proxy tag (ACL target) | `tag:k8s` |

The infra behind these lives in `argocd-apps` (`apps/dolt`, `apps/tailscale-operator`).

---

## 2. repo-weaver module model (engine-accurate — read before designing)

Verified against the working engine, not the aspirational docs. Where `specs/`,
`PRD.md`, or example READMEs disagree with the code, the code wins.

### 2.1 A module is a directory

```
<module-root>/
  weaver.module.yaml     # optional manifest — inputs, tasks, ensures
  files/                 # optional — copied verbatim into the app
  templates/             # optional — Tera (.j2) rendered into the app
```

- Apply walks `files/` and `templates/` for every app that references the module
  (`crates/cli/src/commands/apply.rs:141,194`). `.j2` suffix is stripped on
  render; non-`.j2` files under `templates/` are copied through as-is.
- A missing `weaver.module.yaml` is treated as empty (`crates/core/src/config.rs:304-317`).
- Directory nesting under `files/`/`templates/` is preserved 1:1 into the app root.

### 2.2 Manifest schema (`weaver.module.yaml`) — `ModuleManifest`

`crates/core/src/config.rs:239-333`:

```yaml
inputs:                       # map<name, InputDef>
  <name>:
    type: string|number|bool|list|list(<T>)|map
    default: <value>          # optional
    required: false           # optional
    description: "..."        # optional
outputs: {}                   # declared but not consumed anywhere yet — ignore
tasks:                        # map<name, {command, description}>
  <name>:
    command: "<program> <args>"
    description: "..."
ensures:                      # list — built-in types OR plugin dispatch (see §4)
  - type: "<type>"
    ...
```

### 2.3 Templates are Tera, and **only files are templated**

- `templates/*.j2` are rendered with Tera; the app's resolved `inputs` become the
  context, plus a top-level `secrets.*` object if the consumer declares `secrets:`
  (`crates/core/src/template.rs`, `apply.rs:194-260`). Conditionals/loops work
  (`{% if %}`, `{% for %}`, `{{ x.field }}`).
- **Critical constraint:** `tasks[].command` and manifest-level `ensures[].*`
  config are **NOT** Tera-rendered — they're literal YAML strings
  (`crates/cli/src/commands/run.rs:56-97`; `crates/core/src/ensure/mod.rs:50-143`).
  So a task command **cannot** interpolate `{{ inputs.database }}`. Plan around
  this (see §4).

### 2.4 Running a command

Two options, no third:

- **Module task + `rw run`** — `tasks.<name>.command` runs via
  `rw run <app> <task> [trailing args...]` in the app's directory, forwarding
  trailing args (`run.rs:56-97`). Naive `split_whitespace` — **no quoted args
  with spaces**. `rw apply` does **not** run tasks; they're explicit. rw does
  **not** check idempotency — the command must be safe to re-run.
- **WASM ensure plugin** — the only way to get `plan`/`execute` idempotency wired
  into `rw apply`. `plan()` checks current state via the `process.exec` host
  import and returns pending actions; `execute()` re-checks then mutates. Model on
  `plugins/taskfile-task/src/lib.rs`. See §6 and `docs/PLUGIN_DEVELOPMENT.md`.

### 2.5 Checks (prerequisites)

`CheckDef { name, command, description }` (`config.rs:227-231`) — run
`command` via `sh -c`, exit 0 = pass (`crates/cli/src/commands/check.rs:98-116`).
Declared in the **consumer's** `weaver.yaml` at workspace or per-app level.
**The module manifest has no `checks:` field** — a module cannot ship its own
checks today, so the handoff must tell consumers to add them (or generate them
into `weaver.yaml`; there's no automation for that yet). `rw check` runs them;
**`rw apply` does not**.

### 2.6 Sourcing a module

`ModuleConfig { name, source, ref, path }` (`config.rs:140-159`):

```yaml
modules:
  - name: "beads"
    source: "https://github.com/<org>/beads-module.git"   # or a local path for dev
    ref: "v1.0.0"                                          # branch, tag, or 40-char SHA
```

- `source` with no `://` is a **local path** (`canonicalize`d); otherwise a git
  URL (`crates/core/src/module.rs:30-46`). **Use `https://` (or `ssh://`), never
  the `git@host:` SCP shorthand** — that has no `://` and is misread as a local
  path.
- **`path:` (subdir) is declared but never read** — ship the module at the **repo
  root** of its own repo, or as its own directory referenced by `source:`.
- `rw module add <source> --ref <r>` pins the commit into `weaver.lock`;
  `rw module update <name> --ref <r>` re-pins.

### 2.7 Gotchas / do-not-rely-on (stale or unimplemented)

- `when:` conditional ensures — **not implemented**. No conditional ensures.
- `ensure.file.md_section` requires the **target file to already exist**
  (`ensure/file.rs:147-161`) — it does not create it.
- Examples `16`, `17` (`ensure.cargo.*`, `ensure.go.*`), `20`, `21` (`run:`
  instead of `command:`), `28`, `29` are **aspirational/stale** — not wired to the
  engine. Don't copy their YAML shapes. Trust the integration tests
  (`crates/cli/tests/integration/`) and `examples/test-suite.yaml` `stage: implemented`.

---

## 3. Module design

Recommended: **pure module config** (no plugin) for v1. A `.envrc` is a rendered
template; `bd init` and `direnv allow` are tasks. Add the plugin later (§6) only
if you need `bd init` to fire automatically and idempotently during `rw apply`.

### 3.1 Layout

```
beads-module/                 # own git repo (or a dev directory)
  weaver.module.yaml
  templates/
    .envrc.j2
  README.md
```

### 3.2 `weaver.module.yaml`

```yaml
inputs:
  vault_ref:
    type: string
    default: "op://WebTreeShared/dolt-beads/password"
    description: "1Password secret reference for the shared beads DB password."

tasks:
  # Point bd at the shared Dolt over Tailscale. Idempotent-ish: bd init is safe to
  # re-run; it will report an existing project rather than clobber it.
  # Board choice is made at run time via the trailing --database arg (see §5):
  #   rw run <app> init                       -> per-repo DB (needs the *.* grant)
  #   rw run <app> init --database beads       -> shared board (works today)
  init:
    description: "Initialize/point bd at the shared Dolt server over Tailscale"
    command: "bd init --server-host dolt.stoat-pain.ts.net --server-port 3306 --server-user beads"

  allow:
    description: "Trust the generated .envrc so direnv loads the DB password"
    command: "direnv allow"
```

### 3.3 `templates/.envrc.j2`

This is the whole password mechanism. Note it **does not gate on `op whoami`** —
that errors on multi-account 1Password setups and would wrongly skip the op path.
`op read` with a full `op://vault/item/field` reference resolves across accounts.

```bash
# Managed by repo-weaver (module: beads). bd → shared Dolt over Tailscale.
# Resolves the shared DB password: 1Password (all teammates) → kubectl (admins).
# No manual key-setting; nothing sensitive is stored on disk.
if command -v op >/dev/null 2>&1; then
  BEADS_DOLT_PASSWORD="$(op read '{{ vault_ref }}' 2>/dev/null)"
fi
if [ -z "${BEADS_DOLT_PASSWORD:-}" ] && command -v kubectl >/dev/null 2>&1; then
  BEADS_DOLT_PASSWORD="$(kubectl get secret dolt-secret -n dolt -o jsonpath='{.data.beads-password}' 2>/dev/null | base64 -d)"
fi
export BEADS_DOLT_PASSWORD
[ -z "${BEADS_DOLT_PASSWORD:-}" ] && \
  echo "beads: BEADS_DOLT_PASSWORD unresolved — run 'op signin' (needs {{ vault_ref }}) or ensure kubectl access" >&2
```

Because `templates/` is auto-walked, `rw apply` renders this to `.envrc` at the
app root with no per-app `ensures:` needed. `.envrc` is typically gitignored in
the target repo — that's fine: repo-weaver regenerates it on `apply` and
drift-tracks it in `.rw/state.yaml`; a fresh clone just runs `rw apply` first.

### 3.4 Consumer `weaver.yaml`

```yaml
version: "1"
modules:
  - name: "beads"
    source: "https://github.com/<org>/beads-module.git"   # or "../beads-module" for dev
    ref: "v1.0.0"

apps:
  - name: "repo-weaver"          # the repo being onboarded
    module: "beads"
    path: "."
    inputs: {}                   # vault_ref default is fine
    checks:
      - name: "direnv-installed"
        command: "command -v direnv"
      - name: "tailnet-reachable"
        command: "nc -z -G 5 dolt.stoat-pain.ts.net 3306"
      - name: "bd-installed"
        command: "command -v bd"
      - name: "beads-initialized"
        command: "test -d .beads"
```

### 3.5 Onboarding workflow (what a dev runs)

```sh
op signin                                  # once per session (company SSO)
rw apply                                   # renders .envrc
rw run repo-weaver allow                   # direnv allow
rw run repo-weaver init --database beads    # or omit --database for a per-repo board (§5)
rw check repo-weaver                        # verify prerequisites
bd ready                                    # see the board
```

---

## 4. The `bd init` templating constraint (important)

`tasks[].command` is **not** templated (§2.3), so the server host/port/user are
**hardcoded** in the task command, and the **database is chosen at run time** via
the forwarded trailing arg:

- `rw run <app> init` → `bd init … --server-user beads` (no `--database`) → bd
  derives the DB name from the repo/prefix → **per-repo board** (needs the grant
  in §5).
- `rw run <app> init --database beads` → **shared board** (works today).

Do **not** bake `--database` into the task command and then also pass one at run
time — you'd emit `--database X --database Y`. Keep it out of the task; choose per
run.

If you want the database to be a real module `input` (rendered, not a run-time
arg), you must render a wrapper into the repo (e.g. `templates/.beads/connect.sh.j2`
using `{{ database }}`) and point the task at it (`command: "sh .beads/connect.sh"`).
That trades a hidden helper file for input-driven parameterization. For v1, the
trailing-arg approach is simpler and ships no extra files.

---

## 5. Board model + the one infra prerequisite

Two ways to place a repo's issues; this is a team decision, documented here so the
module supports both.

| | **Shared board** | **Per-repo board** (recommended for many repos) |
|---|---|---|
| `bd init` | `--database beads` | omit `--database` (bd uses the repo/prefix name) |
| Infra change | none — works today | **required** (see below) |
| Issues | all repos intermixed, one prefix | each repo own DB + own prefix |
| Cross-repo view | native | `bd repo add` to aggregate |

**Per-repo prerequisite:** the `beads` MySQL user is currently granted **only** on
the `beads` (+ `dolt_workbench`) database
(`argocd-apps: apps/dolt/templates/init-configmap.yaml`), so it cannot create or
use a new per-repo database. To enable per-repo boards, broaden the grant:

```sql
GRANT ALL PRIVILEGES ON *.* TO 'beads'@'%';
```

That's a one-line change to `init-configmap.yaml` + an ArgoCD sync (the init Job
re-runs the grant on every sync). It is **not** part of this module — it's an
`argocd-apps` PR that must land first if per-repo boards are chosen. The module
itself is board-model-agnostic (§4).

---

## 6. Optional: automatic, idempotent `bd init` via a WASM plugin

Use only if `rw apply` must run `bd init` itself (with a real "already
initialized" check) rather than a separate `rw run … init`. Scaffold per
`docs/PLUGIN_DEVELOPMENT.md`: `cargo component new --lib plugins/beads-init`,
`world = "ensure-provider"`, `path = "../../wit"`. Model on
`plugins/taskfile-task/src/lib.rs`.

```rust
// plugins/beads-init/src/lib.rs — sketch
generate!({ world: "ensure-provider", path: "../../wit" });
use exports::weaver::plugin::ensures::{EnsureError, EnsurePlan, EnsureRequest, Guest};
use weaver::plugin::process::{exec, ExecRequest};

struct Component;

fn initialized(cwd: &str) -> Result<bool, String> {
    // WASM has no direct fs — probe via the process host import.
    let r = exec(&ExecRequest {
        program: "test".into(), args: vec!["-d".into(), ".beads".into()],
        cwd: Some(cwd.into()), env: vec![], inherit_env: true, stdin: None,
    })?;
    Ok(r.status == 0)
}

impl Guest for Component {
    fn plan(req: EnsureRequest) -> Result<EnsurePlan, EnsureError> {
        if initialized(&req.app_path).map_err(EnsureError::ExecutionError)? {
            return Ok(EnsurePlan { description: "beads already initialized".into(), actions: vec![] });
        }
        Ok(EnsurePlan { description: "beads not initialized".into(), actions: vec!["bd init".into()] })
    }
    fn execute(req: EnsureRequest) -> Result<String, EnsureError> {
        if req.dry_run { return Ok("Would run `bd init`".into()); }
        if initialized(&req.app_path).map_err(EnsureError::ExecutionError)? {
            return Ok("already initialized".into());
        }
        let r = exec(&ExecRequest {
            program: "bd".into(),
            args: vec!["init".into(),
                       "--server-host".into(), "dolt.stoat-pain.ts.net".into(),
                       "--server-port".into(), "3306".into(),
                       "--server-user".into(), "beads".into()],
            cwd: Some(req.app_path), env: vec![], inherit_env: true, stdin: None,
        }).map_err(EnsureError::ExecutionError)?;
        if r.status != 0 { return Err(EnsureError::ExecutionError("bd init failed".into())); }
        Ok("bd init completed".into())
    }
}
export!(Component);
```

Then reference it in `weaver.module.yaml` (config is static — §2.3, so the
database can't be per-app here without a run-time arg you no longer have; hardcode
the board model in the plugin or keep it on the shared `beads` DB):

```yaml
ensures:
  - type: "beads.init"     # resolver maps "beads.init" -> plugin "beads-init"
```

Build/release: `./scripts/build-plugins.sh beads-init` (dev),
`./scripts/release-plugin.sh beads-init 1.0.0` (publish). Note the plugin needs
`BEADS_DOLT_PASSWORD` present in the inherited env when `bd init` connects — which
the `.envrc` provides only after `direnv allow`, so ordering still matters. The
task approach (§3) sidesteps this by being explicit.

---

## 7. Testing (TDD — per `AGENTS.md`)

- **Module render test** — apply the module into a temp dir; assert `.envrc`
  exists and contains the `op read` + kubectl fallback and the resolved
  `vault_ref`. Follow the pattern in `crates/cli/tests/integration/`.
- **No-`op-whoami`-gate regression** — assert the rendered `.envrc` does *not*
  contain `op whoami` (the multi-account bug this module fixes).
- **Task shape** — `rw run <app> init` invokes `bd` with the expected args
  (can stub `bd` on `PATH` with a recorder script; avoid mocks per house style).
- **Check semantics** — `rw check` fails when `.beads` is absent, passes when present.
- If you build the plugin: `plan()` returns empty actions when `.beads/` exists,
  non-empty otherwise; `execute()` is a no-op when already initialized
  (mirror `plugins/*/` test style).

---

## 8. Open decisions & prerequisites (resolve before/while implementing)

1. **Board model** — shared `beads` vs per-repo DBs. Per-repo needs the `*.*`
   grant PR in `argocd-apps` (§5) merged + synced first. The module works either
   way; only the `rw run … init` invocation differs.
2. **Plugin or task** — ship v1 as tasks (§3); add the `beads-init` plugin (§6)
   only if auto-apply idempotency is required.
3. **Where the module lives** — its own git repo (recommended; clean `source:`
   URL + tag pinning) vs a directory inside an existing repo. Root-of-repo only
   (`path:` subdir is unsupported, §2.6).
4. **Checks in the consumer** — the module can't ship checks (§2.5); document the
   `weaver.yaml` `checks:` block (§3.4) as part of onboarding, or add tooling to
   inject it.
5. **`.envrc.example` cleanup** — this module supersedes the committed
   `.envrc.example` in `argocd-apps` (which still has the `op whoami` multi-account
   bug). Once repos move to the module, retire that file.

---

## Reference

- Manual flow this automates: `argocd-apps` → `apps/dolt/README.md`
  ("Developer workflow", "Onboarding a teammate").
- Infra: `argocd-apps` → `apps/dolt/` (Dolt + Service `tailscale.com/expose`),
  `apps/tailscale-operator/`.
- repo-weaver engine: `crates/core/src/config.rs` (schema),
  `crates/cli/src/commands/apply.rs` (apply loop), `run.rs`, `check.rs`,
  `crates/core/src/module.rs` (sourcing), `wit/plugin.wit`,
  `docs/PLUGIN_DEVELOPMENT.md`.
