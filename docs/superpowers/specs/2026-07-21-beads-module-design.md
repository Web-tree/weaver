# Design: `beads` repo-weaver module (beads-first, extract-later)

**Date:** 2026-07-21 (updated 2026-07-23)
**Status:** approved, pre-implementation
**Source handoff:** `docs/beads-module-handoff.md`
**Related:** `2026-07-23-weaver-marketplace-design.md` — beads ships "marketplace-shaped"
(its repo carries a self-describing manifest and is indexed by the central catalog).

## Goal

Ship a working repo-weaver module that onboards any repo to the team's shared
**bd (beads)** tracker — Dolt over Tailscale, password from 1Password. Build it
**beads-first**: hand-build the real module end-to-end, record every point of
friction, and let that evidence define a later general module-authoring workflow.
The workflow is *not* built in this pass; its backlog is a deliverable of it (§6).

## Separation of concerns (the spine of this design)

repo-weaver is a **general tool**; the beads module is **one org's specifics**.
These stay in different repos, and the design keeps them from bleeding into each
other:

- **`Web-tree/beads-module`** (new repo) — the org-specific artifact: the
  `weaver.module.yaml`, the `.envrc.j2` template, and the module's *own* tests.
  Versioned and consumed via `source:`.
- **`Web-tree/repo-weaver`** (this repo) — stays infra-agnostic. It gains **no**
  beads example and **no** beads-flavored test. What it gains from this exercise
  is the §6 extract backlog and any *generic engine* fix the dogfood surfaces.
- **The dogfood** is repo-weaver being a real beads *user*: repo-weaver's own
  committed `weaver.yaml` sources the external module. Not a fixture — actual usage.

Deciding fact: `examples/` is currently **100% infra-agnostic** — no existing
example references `stoat-pain`, `WebTreeShared`, or any real endpoint. A beads
example would be the first org-specific fixture in a general tool's suite, and
the engine capabilities it would exercise (template render, task-arg forwarding)
are already covered generically by examples **02** and **14**. So it earns nothing
and costs coupling. It's out.

## Decisions (locked)

| # | Decision | Choice |
|---|---|---|
| 1 | Primary deliverable | Beads module first, then extract the authoring workflow from real friction |
| 2 | Where the module lives | **Its own repo (`Web-tree/beads-module`) from the start.** No `examples/31`; repo-weaver's `examples/` stays infra-agnostic |
| 3 | Board model | Shared `beads` DB for v1 (works against today's infra, no grant PR); module stays board-agnostic |
| 4 | Pass-1 scope | Full slice: module + module's own render test (in its repo) + spike + real dogfood of repo-weaver. Engine tests in repo-weaver only if the dogfood exposes an engine gap, and then **generic**, not beads-flavored |
| 5 | Sequencing | **Spike → TDD** — throwaway manual validation of untested handoff claims, then test-first committed work |

## Engine constraints (verified against code, not docs)

- **C1 — a module source must be a git repo root.** `rw` resolves every local
  source through `git ls-remote` + `git clone`
  (`crates/core/src/module.rs:30-54`). Verified: `git ls-remote examples/git-module`
  → *"does not appear to be a git repository."* And `ModuleConfig.path`
  (`config.rs:145`) is parsed but read nowhere. **Consequence:** the beads module
  must be its own repo — which decision #2 already commits to. During module dev
  you therefore commit (and tag/branch) each iteration you want `rw` to see; this
  is felt directly (no harness crutch — see C2) and *is* the G2 evidence (§6).
- **C2 — the example harness fakes a repo for local sources, but we don't use
  it.** `prepare_module_sources` (`crates/cli/tests/examples_suite.rs:355-405`)
  `git init`s + tags a temp copy so in-repo examples can source local modules
  without ceremony. Since there's no beads example, we don't lean on this — the
  dogfood hits the *real* CLI's real C1 friction, which is what we want to measure.
- **C3 — `tasks[].command` and `ensures[].*` are NOT Tera-rendered**
  (`run.rs:56-97`; `ensure/mod.rs:50-143`). Only `templates/*.j2` are. So the
  `--database` choice is a run-time trailing arg, never baked into the task.
- **C4 — `rw run` splits args on whitespace** (`run.rs`) — no quoted args with
  spaces. None of our commands need them.
- **C5 — a module cannot ship `checks:`.** `ModuleManifest` has no such field
  (`config.rs:239-333`); checks live in the consumer's `weaver.yaml`. So the
  dogfood's `weaver.yaml` hand-writes the checks. Captured as friction gap G3 (§6).

## Handoff corrections (found during design; validate in the spike)

- **`.envrc` happy-path exit bug.** The handoff's final line
  `[ -z "${VAR:-}" ] && echo ... >&2` exits **1** when the password *is* resolved
  (the `&&` short-circuits and the failed test's status becomes the script's
  status). direnv would flag a healthy `.envrc` as failed. Fix: use an `if`
  block, not a bare `&&`.
- **Untested claims to verify in the spike, not assume:** `bd init` re-run
  safety; `op read` resolving across a multi-account 1Password setup without an
  `op whoami` gate; direnv load ordering vs. `bd init` needing the password.
- **Note (no longer a bug):** `.gitignore:9` is `.env*`, so a consumer's rendered
  `.envrc` is gitignored — which is *correct* (handoff §3.3: `.envrc` is
  regenerated by `rw apply`). The beads module's render test therefore asserts
  output in-test (render to a temp dir, assert content) rather than committing a
  golden `.envrc`. No repo-weaver `.gitignore` change is needed.

## Architecture — four components

### 1. The module — lives in `Web-tree/beads-module` (repo root, per C1)

```
weaver.module.yaml       # 1 input (vault_ref), 2 tasks (init, allow)
templates/.envrc.j2      # the whole password mechanism → renders to .envrc
.wt/weaver/manifest.yaml # self-describing marketplace manifest (see marketplace spec §A)
README.md                # onboarding steps + board-model note
```

The `.wt/weaver/manifest.yaml` is discovery metadata only (name, kind, root,
description) — it does **not** re-declare inputs/tasks (those stay authoritative
in `weaver.module.yaml`). It makes the repo self-describing and marketplace-ready.

- **Input:** `vault_ref` (string, default `op://WebTreeShared/dolt-beads/password`).
- **Tasks:** `init` = `bd init --server-host dolt.stoat-pain.ts.net --server-port
  3306 --server-user beads` (no `--database` — chosen per run); `allow` =
  `direnv allow`.
- **Template `.envrc.j2`:** `op read` → kubectl fallback → `export`; **no
  `op whoami` gate**; happy-path-safe exit (correction above). `{{ vault_ref }}`
  is the only interpolation.

### 2. The module's own render test — in `Web-tree/beads-module`

Proves *this module's* correctness, where the module lives. Renders the template
(via `rw apply` into a temp workspace, or a direct Tera render) and asserts the
output contains the `op read` + kubectl fallback and the resolved `vault_ref`,
and does **not** contain `op whoami`. This is the beads repo's regression guard,
not repo-weaver's. (Exact test harness is the beads repo's plan, not this spec's.)

### 3. Spike (throwaway, uncommitted)

Manual dogfood in a scratch dir to validate the untested claims + the `.envrc`
exit fix. Delete afterward. Reconnaissance only — no committed artifact depends
on it. **Machine is ready:** `bd`, `op`, `direnv`, `kubectl` all present; tailnet
reachable on `dolt.stoat-pain.ts.net:3306`; repo-weaver has no `weaver.yaml` and
no `.beads/` (clean guinea pig).

### 4. Real dogfood — repo-weaver as a beads consumer

repo-weaver gets a **committed `weaver.yaml`** sourcing the external module:

```yaml
version: "1"
modules:
  - name: beads
    source: "https://github.com/Web-tree/beads-module.git"
    ref: "v0.1.0"          # or a branch/SHA during bring-up
apps:
  - name: repo-weaver
    module: beads
    path: "."
    inputs: {}             # vault_ref default is fine
    checks:                # C5: module can't ship these; consumer declares them
      - { name: direnv-installed,  command: "command -v direnv" }
      - { name: tailnet-reachable, command: "nc -z -G 5 dolt.stoat-pain.ts.net 3306" }
      - { name: bd-installed,      command: "command -v bd" }
      - { name: beads-initialized, command: "test -d .beads" }
```

Then run it for real: `rw apply` → `app/.envrc` (well, `./.envrc`) →
`rw run repo-weaver allow` → `rw run repo-weaver init --database beads` →
`rw check repo-weaver` → `bd ready`. The rendered `.envrc` is gitignored (correct);
`weaver.yaml` and `.beads/` are committed. Any *engine* gap this surfaces gets a
**generic** fix + test in repo-weaver — never a beads-flavored one.

## Data flow (onboarding, shared board)

```
op signin
rw apply                                   # renders ./.envrc from the module template
rw run repo-weaver allow                    # direnv allow
rw run repo-weaver init --database beads     # bd → shared Dolt, shared board
rw check repo-weaver                         # prerequisites (direnv, tailnet, bd, .beads)
bd ready                                     # board is live
```

## Testing strategy (TDD per AGENTS.md)

- **beads repo:** module render test red → write template/manifest → green;
  spike validates claims before the test hardens them.
- **repo-weaver:** no new beads test. If the dogfood exposes an engine gap
  (e.g. `rw run` mis-forwards a trailing arg), reproduce it with a **generic**
  failing test in `crates/cli/tests/integration/`, fix, green.
- Full `cargo test` + `cargo clippy` green in whichever repo changed.

## Out of scope (this pass)

- The `beads-init` WASM plugin (handoff §6) — tasks suffice for v1.
- Per-repo boards + the `argocd-apps` `*.*` grant PR (handoff §5).
- Building `rw module new` / dev-mode / module `checks:` / subpath sourcing —
  *recorded* in §6, built in the extract pass, not now.
- Retiring `argocd-apps`'s `.envrc.example`.
- Any beads example or beads-flavored test inside repo-weaver.

## §6 — Extract backlog (the actual point of going beads-first)

Friction visible before writing code; the dogfood may add more. This list is a
primary deliverable — it seeds the general module-development workflow.

| ID | Gap | Evidence |
|----|-----|----------|
| G1 | No `rw module new` scaffold | `rw init` is consumer-side only; nothing emits a module skeleton |
| G2 | Local module dev requires a commit per iteration | CLI clones every source (C1); no dev-mode. Felt directly during beads bring-up since we don't use the example-harness crutch (C2) |
| G3 | Modules can't ship `checks:` | `ModuleManifest` has no field (C5); every consumer hand-copies the same checks — see the dogfood `weaver.yaml` |
| G4 | No subpath sourcing | `path:` ignored (C1); example 28's `paths:` unimplemented — blocks a modules monorepo and in-repo module hosting |
| G5 | No render assertion without a committed golden file | `.env*` gitignore means the beads test can't lean on a golden `.envrc`; a `render → assert-content` helper (or engine `--render-only`) would help module authors generally |
| G6 | No marketplace resolution engine | `marketplace:` config + `use: name@ref` expansion; catalog backs `get_registry_url()`'s TODO; `provides_ensure` replaces the fantasy plugin default. See marketplace spec |
| G7 | `rw module new` should scaffold the self-manifest | emit `.wt/weaver/manifest.yaml` alongside the module skeleton (ties to G1) |
| G8 | No `rw marketplace` verbs | list/search/validate; validate that catalog `source`/`ref` resolve and match the target repo's self-manifest |

## Resolved design-time flag

Earlier draft had the module start as an in-repo example and "promote later,"
which blurred the general tool with one org's infra. Resolved: the module lives
in its own repo from the start; repo-weaver stays infra-agnostic. Cost — eating
C1's commit-per-iteration friction immediately — is accepted, because that
friction is exactly the G2 evidence the extract pass needs.
