# Handoff: Weaver requirements for a Webtree dev-standards module

**Goal.** Define everything the Weaver engine (`Web-tree/weaver`, binary `wvr`) must
be able to do before we can author a **dev-standards module** that answers, for any
Webtree repo: *does this repo conform to the Webtree dev standards, and if not,
exactly which rules fail and how do I fix them?*

**Audience.** Whoever implements the engine work (human or agent), and whoever
authors the module afterwards. Engine citations are `file:line` into this repo,
authored against `6bf1c3f` and re-verified against `2faf1d8` (the intervening
commits are dependency bumps only). Case citations marked *(corpus)* point at the
Webtree Crossplane/k8s repo — the repo the standards are being derived from — not
at this one.

**Status.** Requirements only — no implementation decisions locked beyond what the
existing Weaver specs already lock. Supersedes nothing; complements
`specs/004-generic-standardization-engine/spec.md`, which covers the *convergence*
half of this problem and leaves the *validation* half largely unbuilt.

**Framing.** Weaver today can **generate** a conformant repo. It cannot yet
**judge** one. Generation and judgement must come from one rule definition — two
lists that drift apart are worse than no automation. Every requirement below exists
to make one authored rule serve both directions.

---

## 1. Verified engine baseline

What exists today, so the requirements below are a delta and not a rewrite.

**Works:**

- Module resolution with git ref → resolved-commit pinning; lockfile wired; cache
  keyed by resolved commit (`crates/core/src/module.rs`, `lockfile.rs`).
- `wvr module add <git-url> --ref <r>` (`crates/cli/src/commands/module.rs:16-35`).
- Region-scoped file primitives — `ensure.file.exists`,
  `ensure.file.from_template`, `ensure.file.md_section` (`heading` and
  `block_marker` selectors), preserving non-owned bytes
  (`crates/core/src/ensure/file.rs`). Example 23 passes end-to-end.
- WASM plugin tier: `ensure-provider` world exporting `plan`/`execute`, with a
  `process.exec` host import (`wit/plugin.wit:42-67`,
  `crates/core/src/plugin/ensure_wasm.rs`). Eight ensure plugins shipped, plus one
  `provider`-world secrets plugin (`aws-ssm`).
- Tera templating, typed module inputs, multi-app path scoping, config includes.

**Does not work / does not exist** (each is the reason for a requirement below):

| Gap | Evidence |
|---|---|
| A module cannot ship checks | `ModuleManifest` has `inputs`/`outputs`/`tasks`/`ensures`, no `checks` (`config.rs:240-249`); `wvr check` reads only `config.checks` + `app.checks` (`check.rs:15-47`) |
| `plan` can never report "converged" | `md_section.plan()` and `from_template.plan()` return actions unconditionally (`ensure/file.rs:136-145`, `221-226`) |
| Checks are one shell string | `CheckDef { name, command, description }` (`config.rs:227-231`) — no expected exit code, no output matching, no severity, no remediation |
| No conditionals | no `when:`, no feature toggles, no per-ensure `enabled:` anywhere in `config.rs` |
| No structured config primitives at the consumer surface | `EnsureSpec` exposes only `ensure.npm.*` + `ensure.file.*` (`config.rs:165-206`); `json_merge::ensure_json_key` exists but only backs `npm.*` (`crates/core/src/ensures.rs:12-33`) |
| Sub-file ownership not tracked | `FileState.owned_regions` is schema-only, never populated (`state.rs:52`; `specs/004-generic-standardization-engine/FOLLOWUPS.md:13`) |
| Config path is hardcoded | `Path::new("weaver.yaml")` in cwd in every command (`apply.rs:54`, `check.rs:15`, `describe.rs:24`, `list.rs:24`, `module.rs:65,107,165`) — no `-C` / `--repo` |
| No machine-readable check output | `wvr check` prints a `comfy_table` and bails with a count (`check.rs:63-96`); global `--json` only switches log format |
| Plugins cannot supply checks | the `ensures` WIT interface exports `plan`/`execute` only (`wit/plugin.wit:60-61`) |

---

## 2. What "validate" has to mean

Three distinct questions the standards module must answer, because we have three
distinct consumers:

| Mode | Consumer | Question | Must not |
|---|---|---|---|
| **Gate** | CI on every PR | "Is this repo conformant? yes/no" | mutate the working tree; require network |
| **Report** | a human, a dashboard | "Which rules fail, how badly, and how do I fix each?" | reduce to a single boolean |
| **Fix** | a developer, an agent | "Make it conformant" | touch anything the rule doesn't own |

One rule definition must serve all three. A rule that can only fix (today's
`ensure.*`) or only gate (today's `check.command`) is half a rule.

---

## 3. Case catalog — what the standards actually contain

Grounded in the real files of the corpus repo. Each case is a class of rule the
module must express; the right-hand column is the engine capability it needs. `R*`
refs point at §4.

| # | Case | Concrete instance *(corpus)* | Needs |
|---|---|---|---|
| **C1** | Managed section in a doc, current with the module version | `AGENTS.md` beads block, `<!-- BEGIN BEADS INTEGRATION profile:full hash:d4f96305 -->` | R11 (section assert), R5 (converged detection) |
| **C2** | House-rules prose section present | `AGENTS.md` "Non-Interactive Shell Commands", "Landing the Plane" | R11 |
| **C3** | Key registered inside a JSON object we don't own wholly | `.claude/settings.json` → `extraKnownMarketplaces.webtree-k8s-plugins` | R12 (key-path assert), R13 (key-path converge) |
| **C4** | Entry in a JSON **array of objects**, identified by a field, with required sibling fields | `.claude-plugin/marketplace.json` → `plugins[]` entries keyed by `name`, each needing `source`/`description`/`version`/`author`/`keywords`/`category` | R14 (identity-keyed array upsert + per-element assertions) |
| **C5** | MCP server registered | `.mcp.json` (absent today; target state) | R12, R13 |
| **C6** | Required files exist and are non-trivial | `LICENSE`, `README.md`, `AGENTS.md`, `renovate.json`, `.github/actionlint.yaml` | R10 (file assert) |
| **C7** | Forbidden content / files | committed `.beads/.beads-credential-key`; any `.env`; hardcoded tokens | R15 (negative + scan rules) |
| **C8** | Required lines in an unstructured file | `.gitignore` must carry `.env*`, `.rw/state.yaml`, `.rw/answers.yaml` | R16 (line-set, configurable comment leader) |
| **C9** | Key inside a YAML config we don't own wholly | `renovate.json` `extends: [config:recommended]`; `tests/lint/.yamllint.yaml` rules | R12, R13 |
| **C10** | Required CI jobs/steps across a **glob** of files | `.github/workflows/*.y*ml` must run yamllint + actionlint + kubeconform; must trigger on `pull_request` and `push:main`; must set `concurrency.cancel-in-progress` | R17 (glob-scoped structural query with any/every semantics) |
| **C11** | Pinned third-party versions | `uses: actions/checkout@v7`, `raven-actions/actionlint@v2` — pinned, not floating | R17, R18 (value predicates) |
| **C12** | Same value in two places | `CROSSPLANE_CLI_VERSION` in `Taskfile.yml` `vars` vs `.github/workflows/crossplane-test.yaml` `env` | R19 (cross-selector equality) |
| **C13** | Required tasks exist, queried via the native tool | `Taskfile.yml` must expose `lint`, `render`, `test`, `default` with descriptions (`task -l --json`) | R20 (plugin-backed checks), existing `taskfile-task` plugin |
| **C14** | Path/naming conventions | `docs/superpowers/{plans,specs}/YYYY-MM-DD-*.md`; workflow extension is `.yaml` in one file and `.yml` in two — a real inconsistency to gate | R21 (glob + regex path rules) |
| **C15** | Directory structure | `crossplane/{xrds,compositions,claims,rbac}`, `tests/{lint,render,e2e,ci}` | R10 |
| **C16** | Tool-managed state initialized | `.beads/` present with `config.yaml` + the five git hooks | R20 |
| **C17** | Repo-external settings | branch protection, required status checks, Renovate enabled | R20 + a GitHub-API plugin (v2, see §7) |
| **C18** | Stack-specific rule sets | TS Hono API / TanStack web / Go worker / Rust / KCL-Crossplane / infra repos each get different subsets of C1–C17 | R6 (profiles), R7 (`when:`) |

Two structural observations from building this catalog:

- **Most rules are "a fragment inside a file someone else owns."** Whole-file
  ownership (`from_template`) covers a minority of cases. Region- and key-scoped
  assertions are the load-bearing primitives.
- **Several rules are file-set rules, not file rules** (C10, C11, C14). "Every
  workflow must pin its actions" cannot be expressed as a rule about one path.

---

## 4. Requirements

Each is testable. `MUST` = blocks the v1 module; `SHOULD` = blocks the standard
being trustworthy at fleet scale; `MAY` = nice to have.

### A. Rule distribution — the module owns the rules

- **R1 (MUST)** — `ModuleManifest` MUST support a `checks:` field, and `wvr check`
  MUST execute checks inherited from every module an app consumes, in addition to
  workspace- and app-level checks.
  *Why:* without this, every consuming repo copies the rule list into its own
  `weaver.yaml` and the standard has no single source of truth (`config.rs:240-249`;
  `check.rs:43-46`'s own comment concedes the omission). This is the single
  highest-value item in the document.
  *Accept:* a module declaring one check, adopted by a repo whose `weaver.yaml`
  declares none, fails `wvr check` when the repo violates it.

- **R2 (MUST)** — Inherited checks MUST be attributable: output identifies the
  module, its resolved commit, and the rule id for every result.
  *Accept:* the report for a failing rule names `module@<sha>` and a stable rule id.

- **R3 (SHOULD)** — A rule MUST have a stable **id** independent of its name, so
  waivers and history survive a rename.

- **R4 (SHOULD)** — Updating the module ref and re-running MUST surface newly
  added rules distinctly from newly broken ones ("3 new rules since v1.2, 1 of
  which you fail").

### B. Verdict correctness

- **R5 (MUST)** — Every ensure's `plan()` MUST report zero actions when the target
  is already in the desired state. Specifically `md_section` and `from_template`
  must compute the would-be bytes and diff (`ensure/file.rs:136-145`, `221-226`).
  *Why:* until this lands, `wvr plan --detailed-exitcode` returns 2 on a perfectly
  conformant repo — there is no working gate at all.
  *Accept:* apply, then plan; exit code 0 and an empty change list.

- **R6 (MUST)** — Rules MUST be groupable into named **profiles** (repo kinds:
  `api`, `web`, `worker-go`, `worker-ts`, `infra-k8s`, `library`), selectable per
  repo, with a rule belonging to zero or more profiles. (C18)

- **R7 (MUST)** — Ensures and checks MUST support a `when:` condition over module
  inputs/profile selections (spec 004 FR-012, still unimplemented). (C18)
  *Accept:* a rule gated on a disabled feature is reported as `skipped`, not
  `passed` — a skipped rule must never inflate a conformance score.

- **R8 (MUST)** — `check` MUST be strictly read-only, and MUST NOT require network
  access once the module is resolved (offline flag already exists on `apply`).
  *Accept:* a check run against a read-only filesystem checkout succeeds or fails
  on rule merits, never on write errors.

- **R9 (SHOULD)** — On a brownfield repo Weaver never wrote, a rule MUST be able to
  distinguish *absent* from *present-but-drifted*, which requires populating
  `FileState.owned_regions` (spec 004 FR-040; today schema-only).

### C. Assertion vocabulary

Each of these MUST exist as a **declarative, read-only** rule type, and each SHOULD
have a converging counterpart so `--fix` works from the same declaration.

- **R10 (MUST)** — `check.file` — path exists / does not exist / is a directory,
  optional min size, optional content regex. (C6, C15)
- **R11 (MUST)** — `check.section` — a named region (`block_marker` or heading
  path) exists in a text file, optionally matching expected content or a content
  hash. Read-only mirror of `ensure.file.md_section`. (C1, C2)
- **R12 (MUST)** — `check.key` — a key-path in JSON **or** YAML exists and equals /
  contains / matches an expected value. (C3, C5, C9)
- **R13 (MUST)** — `ensure.json.key` and `ensure.yaml.key` exposed at the
  **consumer/module surface** (spec 004 FR-022/FR-023; `json_merge` exists but only
  backs `npm.*` — `config.rs:165-206`). Siblings, key order and indentation
  preserved. (C3, C5, C9)
- **R14 (MUST)** — Array-of-objects support in both R12 and R13: address an element
  by an identity field (`plugins[name=crossplane-claim-generator]`), assert its
  fields, upsert without reordering or duplicating siblings. (C4)
  *Why called out separately:* the marketplace catalog is the single most
  standards-critical file in the org and it is an array, not a map. Key-path-only
  support does not cover it.
- **R15 (MUST)** — `check.absent` / content-scan rules: forbidden paths, forbidden
  patterns (credentials, tokens), with an allowlist. (C7)
- **R16 (SHOULD)** — `ensure.text.lines` / `check.lines` — a set of lines is
  present in an unstructured file, with a configurable comment leader (the blocker
  that keeps example 30 `pending`). (C8)
- **R17 (MUST)** — Rules MUST accept a **glob** target plus quantifier semantics —
  `any` (at least one matching file satisfies it) and `every` (all do) — with the
  violating paths listed individually in the report. (C10, C11, C14)
- **R18 (SHOULD)** — Value predicates beyond equality: regex, semver range,
  "is pinned" (not a floating ref), enum membership. (C11)
- **R19 (SHOULD)** — Cross-selector rules: assert two selectors — possibly in
  different files and different formats — hold equal values. (C12)
- **R20 (MUST)** — `check.command` MUST gain `expect` (exit code), `stdout_contains`
  / `stdout_matches`, `timeout`, and `cwd` (spec 004 FR-024). And the WIT `ensures`
  interface MUST export a `check` function so plugins can supply rules, not just
  convergence (`wit/plugin.wit:60-61`). (C13, C16, C17)
- **R21 (SHOULD)** — `check.paths` — a glob's members must match a filename regex
  (dates, extensions, kebab-case). (C14)

*Note on examples 20 and 21:* they use a `run:` key
(`examples/20-check-k8s-ingress-annotations/before/weaver.yaml:13`,
`examples/21-check-terraform-ec2-tags/before/weaver.yaml:13`) that does not
deserialize into `CheckDef`, and neither is registered in
`examples/test-suite.yaml`. They are design sketches, not a contract. Whichever key
wins, R20 should register them as executable examples so this class of rule stays
proven.

### D. Reporting and CI integration

- **R22 (MUST)** — `wvr check --json` MUST emit a machine-readable report: per rule
  — id, name, module + resolved commit, profile, severity, status
  (`pass`/`fail`/`skip`/`error`/`waived`), the target path(s), the observed vs
  expected value, and a human remediation string. Today's output is a
  `comfy_table` plus a failure count (`check.rs:63-96`).
- **R23 (MUST)** — Rules MUST carry a **severity** (`error` | `warn` | `info`), and
  the exit code MUST derive from severity — only `error` fails the gate.
  *Why:* without warn-level rules, every new standard is a breaking change to every
  repo's CI, and the standard stops being adoptable.
- **R24 (MUST)** — Typed exit codes: `0` conformant, `2` violations,
  `1` user error, `3` system error, `4` plugin error (spec 004 FR-005).
- **R25 (SHOULD)** — Each failing rule MUST state whether it is auto-fixable, and
  `wvr apply` (or `wvr check --fix`) MUST fix exactly the auto-fixable failures
  from the same rule definitions.
- **R26 (MAY)** — SARIF output, so violations surface as GitHub code-scanning
  annotations on the PR diff.

### E. Targeting

- **R27 (MUST)** — A global `-C <path>` / `--repo <path>` (or equivalent) so
  commands operate on a repo other than cwd. Every command currently hardcodes
  `Path::new("weaver.yaml")` (`apply.rs:54`, `check.rs:15`, `describe.rs:24`,
  `list.rs:24`, `module.rs:65,107,165`).
- **R28 (MUST)** — **Foreign-repo validation**: validate a repo that has *not*
  adopted Weaver — no `weaver.yaml`, no `.rw/` — by pointing `wvr` at a module plus
  a target path (e.g. `wvr check --module <git-url> --ref <r> -C <path>`).
  *Why:* the original ask is "validate if **some** repo fits the standards." Most
  repos we want to grade will not have adopted the module on the day we first grade
  them; requiring adoption first inverts the funnel.
  *Accept:* running against a clean clone of an unmodified repo produces a full
  report and writes nothing.
- **R29 (SHOULD)** — Monorepo scoping: rules apply per app path, and a repo-level
  rule and a package-level rule are distinguishable in the report.
- **R30 (SHOULD)** — Fleet mode: run one module against N repos and emit one
  aggregated report (spec 004 calls a thin batch runner the v1 ceiling — that is
  enough).

### F. Governance

- **R31 (MUST)** — **Waivers**: a repo can opt out of a specific rule id in its own
  config, with a required justification and an optional expiry; waived rules appear
  in the report as `waived`, never silently as `pass`. Spec 004 FR-013's
  `enabled: false` is the mechanism; the justification and visibility are the
  requirement.
- **R32 (SHOULD)** — Conformance summary: counts by severity and status, and a
  stable score, so adoption can be tracked over time.
- **R33 (SHOULD)** — The module MUST be versioned and consumers pinned, with the
  report naming the version it graded against (builds on the already-working
  lockfile).
- **R34 (MAY)** — `deprecated` / `since` metadata on a rule, so a rule can ship as
  `warn` and be promoted to `error` on a published date.

---

## 5. Minimum viable validator

If the engine work has to be sliced, this is the smallest set that produces a
standards module worth adopting. Everything else is a widening of the rule
vocabulary on a working spine.

**Slice 1 — a gate that can say "yes":** R1, R2, R5, R20, R22, R23, R24.
Yields: module-owned shell-command rules with severity, a JSON report and a
trustworthy exit code. Every case in §3 is expressible, if crudely, as a
`check.command`.

**Slice 2 — declarative rules:** R10, R11, R12, R13, R14, R15, R17.
Yields: rules that are data, portable across repos, and auto-fixable — and,
critically, C4 (the marketplace catalog).

**Slice 3 — real-world adoption:** R6, R7, R27, R28, R31.
Yields: profiles per repo kind, grading of repos that have not adopted Weaver, and
waivers so adoption doesn't stall on the first disagreement.

**Slice 4 — fleet:** R9, R19, R25, R29, R30, R32.

Interim option while Slice 1 is in flight: author the module now with app-level
`check.command` entries in each consuming repo, accepting the copy-paste. It proves
the rule catalog is right and gives Slice 1 a ready-made corpus to migrate — but it
is explicitly not the end state, because R1 is the whole point.

---

## 6. Non-functional requirements

- **Determinism** — two runs on the same tree and the same module commit produce
  identical reports. No wall-clock, no network, no ambient tool-version dependence
  beyond what a rule explicitly probes.
- **No false green** — a rule that cannot be evaluated (missing tool, unreadable
  file) reports `error`, never `pass`. This is worth stating because it is the
  failure mode that quietly kills trust in a linter.
- **Speed** — a full validation of a repo this size must be fast enough to sit in
  the PR gate without anyone noticing; fleet mode must parallelize across repos.
- **Diagnosability** — a failing rule tells you the file, the location within it,
  what was expected, what was found, and what to run.

---

## 7. Explicitly out of scope for v1

- **C17 / R20's GitHub-API rules** (branch protection, required checks). Needs a
  credentialed plugin and a different trust model than a filesystem probe. Name the
  rules now, implement after Slice 3.
- **AI-assisted rule evaluation.** `ensure.ai.patch` stays a last-resort fix path,
  never an assertion — a non-deterministic gate is not a gate.
- **Auto-opened per-repo remediation PRs.** A batch runner (R30) is the ceiling.
- **A registry/marketplace resolver.** Raw git URLs are fine; the marketplace design
  (`docs/superpowers/specs/2026-07-23-weaver-marketplace-design.md`) already covers
  this as a later pass and nothing here depends on it.

---

## 8. Open decisions

1. **Rule-definition surface.** Are checks a sibling list to `ensures` in the module
   manifest, or does one declaration emit both a check and an ensure (a rule with
   `assert` and `fix` faces)? The second avoids the two-lists-drift problem R25
   otherwise has to police manually. Recommendation: one declaration, two faces.
2. **Structural query language for C10/C11/C17.** JSONPath, a `yq`-style
   expression, or CEL? Weaver's "native tool first" principle (PRD §2) argues for
   shelling to `yq`/`jq` rather than embedding a query engine — but that makes rules
   depend on tools present on the runner, which collides with the no-false-green
   rule. Needs a decision before R17.
3. **Where the standards module lives.** Its own repo (as `beads-module` did, per
   the marketplace design's M2 separation), or inside this repo? Module sources must
   be a git repo root today — `ModuleConfig.path` is parsed but never read
   (`config.rs:144`) — so a subdirectory of this repo is not addressable until that
   changes.
4. **Profile detection.** Explicit selection in each repo's config, or inferred from
   repo contents (a `go.mod` implies `worker-go`)? Inference is friendlier and
   guessable-wrong; explicit is honest. Recommendation: explicit, with inference as
   a suggestion during onboarding.
5. **Baseline for existing repos.** Grading the current fleet against a complete
   standard will produce a wall of red. Do we ship rules as `warn` first and promote
   (R34), or generate a per-repo waiver baseline at adoption (R31)? Pick one before
   the first rule ships as `error`.
6. **Where the standards text lives.** `AGENTS.md` sections are both the standard's
   documentation and one of its enforced artifacts (C1). Decide whether the module
   generates the prose from templates or asserts against prose kept elsewhere.

---

## 9. Reference

**Corpus repo (the case source, not this repo):** `AGENTS.md`,
`.claude/settings.json`, `.claude-plugin/marketplace.json`, `.beads/`,
`.github/workflows/`, `.github/actionlint.yaml`, `Taskfile.yml`, `renovate.json`,
`tests/lint/`, `crossplane/`, `docs/superpowers/`.

**Weaver engine (this repo):** `crates/core/src/config.rs` (schema),
`crates/core/src/ensure/` (primitives), `crates/cli/src/commands/check.rs`,
`plan.rs`, `apply.rs`, `crates/core/src/module.rs` (sourcing),
`crates/core/src/state.rs`, `wit/plugin.wit`, `docs/PLUGIN_DEVELOPMENT.md`.

**Weaver design docs:** `specs/004-generic-standardization-engine/spec.md` and
`FOLLOWUPS.md` (the convergence half; R5, R7, R13, R23, R24, R31 all map to
already-approved FRs there), `docs/beads-module-handoff.md` (house style for this
document, and the engine constraints a module author hits in practice),
`docs/superpowers/specs/2026-07-23-weaver-marketplace-design.md`.
