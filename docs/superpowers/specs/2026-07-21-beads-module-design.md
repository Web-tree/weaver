# Design: `beads` repo-weaver module (beads-first, extract-later)

**Date:** 2026-07-21
**Status:** approved, pre-implementation
**Source handoff:** `docs/beads-module-handoff.md`

## Goal

Ship a working repo-weaver module that onboards any repo to the team's shared
**bd (beads)** tracker — Dolt over Tailscale, password from 1Password. Build it
**beads-first**: hand-build the real module end-to-end, record every point of
friction, and let that evidence define a later general module-authoring workflow.
The workflow is *not* built in this pass; its backlog is a deliverable of it (§6).

## Decisions (locked)

| # | Decision | Choice |
|---|---|---|
| 1 | Primary deliverable | Beads module first, then extract the workflow from real friction |
| 2 | Where the module lives | `examples/31-beads-module/` golden fixture first; **promote to its own git repo** for the dogfood (required — see Constraint C1) |
| 3 | Board model | Shared `beads` DB for v1 (works against today's infra, no grant PR); module stays board-agnostic |
| 4 | Pass-1 scope | Full slice: golden example + integration tests + real dogfood |
| 5 | Sequencing | **Spike → TDD** — throwaway manual validation of untested handoff claims, then test-first committed work |

## Engine constraints (verified against code, not docs)

- **C1 — a module source must be a git repo root.** `rw` resolves every local
  source through `git ls-remote` + `git clone`
  (`crates/core/src/module.rs:30-54`). Verified: `git ls-remote examples/git-module`
  → *"does not appear to be a git repository."* And `ModuleConfig.path`
  (`config.rs:145`) is parsed but read nowhere. **Consequence:** the module cannot
  be sourced from a subdirectory of repo-weaver; the dogfood requires a real
  separate repo (or the module's own repo). Promotion is not optional in this pass.
- **C2 — the example harness already fakes a repo for local sources.**
  `prepare_module_sources` (`crates/cli/tests/examples_suite.rs:355-405`) `git
  init`s and tags a temp copy of each local module before applying. So the
  *golden example* needs no separate repo; only the *by-hand CLI dogfood* hits C1.
  The harness knows something the CLI doesn't — captured as friction gap G2 (§6).
- **C3 — `tasks[].command` and `ensures[].*` are NOT Tera-rendered**
  (`run.rs:56-97`; `ensure/mod.rs:50-143`). Only `templates/*.j2` are. So the
  `--database` choice is a run-time trailing arg, never baked into the task.
- **C4 — `rw run` splits args on whitespace** (`run.rs`) — no quoted args with
  spaces. None of our commands need them.
- **C5 — a module cannot ship `checks:`.** `ModuleManifest` has no such field
  (`config.rs:239-333`); checks live in the consumer's `weaver.yaml`. Captured as
  friction gap G3 (§6).

## Handoff corrections (found during design; validate in the spike)

- **`.envrc` happy-path exit bug.** The handoff's final line
  `[ -z "${VAR:-}" ] && echo ... >&2` exits **1** when the password *is* resolved
  (the `&&` short-circuits and the failed test's status becomes the script's
  status). direnv would flag a healthy `.envrc` as failed. Fix: use an `if`
  block, not a bare `&&`.
- **`.gitignore:9` is `.env*`**, which ignores
  `examples/31-beads-module/after/app/.envrc`. The golden fixture's expected
  output can't be committed without a negation rule. Fix: add
  `!examples/**/.envrc` (and confirm the fixture file is force-added).
- **Untested claims to verify in the spike, not assume:** `bd init` re-run
  safety; `op read` resolving across a multi-account 1Password setup without an
  `op whoami` gate; direnv load ordering vs. `bd init` needing the password.

## Architecture — five components

### 1. The module (pure config, no plugin)

`examples/31-beads-module/module/`:

```
weaver.module.yaml     # 1 input (vault_ref), 2 tasks (init, allow)
templates/.envrc.j2    # the whole password mechanism → renders to .envrc
README.md              # onboarding steps + board-model note
```

- **Input:** `vault_ref` (string, default `op://WebTreeShared/dolt-beads/password`).
- **Tasks:** `init` = `bd init --server-host dolt.stoat-pain.ts.net --server-port
  3306 --server-user beads` (no `--database` — chosen per run); `allow` =
  `direnv allow`.
- **Template `.envrc.j2`:** `op read` → kubectl fallback → `export`; **no
  `op whoami` gate**; happy-path-safe exit (correction above). `{{ vault_ref }}`
  is the only interpolation.

### 2. Golden example `examples/31-beads-module/`

`before/` (weaver.yaml sourcing `../module` at a ref) + `after/` (converged
tree incl. rendered `app/.envrc`). Registered in `test-suite.yaml` **`stage:
pending` first** (must fail red), flipped to `implemented` when green. The
harness's full-content diff against `after/` is stricter than any custom
assertion — it *is* the `op whoami` regression guard. Needs the `.gitignore`
negation (correction above).

### 3. Integration tests `crates/cli/tests/integration/beads.rs`

Covers what the example suite structurally can't (`rw apply` never runs tasks):

- **Task shape:** a recorder script named `bd` on `PATH` (real process, no
  mocks — house style) captures argv; assert `rw run <app> init --database beads`
  forwards trailing args as `bd init … --database beads`.
- **Check semantics:** `rw check` fails when `.beads/` absent, passes when present.

### 4. Spike (throwaway, uncommitted)

Manual dogfood in a scratch dir to validate the three untested claims + the
`.envrc` exit fix. Delete afterward. Only reconnaissance — no committed artifact
depends on it existing.

### 5. Real dogfood

Promote the validated module to its own git repo (C1), onboard **repo-weaver
itself** (clean guinea pig: no `weaver.yaml`, no `.beads/`, tailnet + `bd`/`op`/
`direnv`/`kubectl` all verified present). Real `rw apply` → real `.envrc` → real
`op read` → real `bd init --database beads` → `bd ready`.

## Data flow (onboarding, shared board)

```
op signin
rw apply                              # renders app/.envrc from template
rw run <app> allow                    # direnv allow
rw run <app> init --database beads    # bd → shared Dolt, shared board
rw check <app>                        # prerequisites (direnv, tailnet, bd, .beads)
bd ready                              # board is live
```

## Testing strategy (TDD per AGENTS.md)

1. Example 31 registered `pending` → red. Build module → flip to `implemented` → green.
2. `beads.rs` integration tests: red → implement/verify → green.
3. Spike validates claims before any fixture hardens them.
4. Full suite (`cargo test`) + `cargo clippy` green before dogfood.

## Out of scope (this pass)

- The `beads-init` WASM plugin (handoff §6) — tasks suffice for v1.
- Per-repo boards + the `argocd-apps` `*.*` grant PR (handoff §5).
- Building `rw module new` / dev-mode / module `checks:` / subpath sourcing —
  these are *recorded* here (§6) and become the extract pass, not built now.
- Retiring `argocd-apps`'s `.envrc.example`.

## §6 — Extract backlog (the actual point of going beads-first)

Friction visible before writing code; the dogfood may add more. This list is a
primary deliverable — it seeds the general module-development workflow.

| ID | Gap | Evidence |
|----|-----|----------|
| G1 | No `rw module new` scaffold | `rw init` is consumer-side only; no command emits a module skeleton |
| G2 | Local module dev requires a commit per iteration | CLI clones every source (C1); the test harness already works around this (`examples_suite.rs:355`) but the CLI has no dev-mode |
| G3 | Modules can't ship `checks:` | `ModuleManifest` has no field (C5); every consumer hand-copies the same checks |
| G4 | No subpath sourcing | `path:` ignored (C1); example 28's `paths:` unimplemented — blocks in-repo dogfood and a modules monorepo |
| G5 | Harness lacks `file_not_contains` | cheap regression-assertion primitive; would make "no `op whoami`" an explicit check |

## Open flag raised at design time

§5 promotion to a separate repo is more ceremony than "example first" implied.
Accepted: deferring the dogfood is the alternative, and the dogfood is where the
real friction (the §6 backlog's ground truth) actually surfaces.
