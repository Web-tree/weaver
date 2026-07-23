# Design: weaver marketplace (module + plugin catalog)

**Date:** 2026-07-23
**Status:** approved, pre-implementation
**Related:** `2026-07-21-beads-module-design.md` (beads is the marketplace's first entry)

## Goal

Give repo-weaver a **named catalog** of modules and plugins so consumers resolve
by name instead of hardcoding a git URL + ref in every `weaver.yaml`. Beads is
the forcing function and first citizen. Designed now; the **resolution engine is
a later pass** — this pass ships only the authorable-today pieces (§ "What ships").

## Why this exists

Two concrete pains:
- Every `modules:` entry hardcodes `source` + `ref` (`config.rs`); no discovery,
  no single source of truth, N repos referenced by N copies of a URL.
- The plugin subsystem *already* has a `Registry` source but it points at a
  fantasy default (`https://plugins.repo-weaver.dev`, `resolver.rs:391`) that
  nobody serves, with a literal `// TODO: Check config file` (`resolver.rs:388`)
  where a real catalog belongs.

The marketplace finishes the plugin seam and adds the missing module side.

## Decisions (locked)

| # | Decision | Choice |
|---|----------|--------|
| M1 | Catalog topology | **Hybrid** — each publishing repo self-describes (`.wt/weaver/manifest.yaml`); a central index points at them |
| M2 | Catalog home | Dedicated repo **`Web-tree/weaver-marketplace`** (not folded into a dev-standards repo — keeps catalog vs standards separate, gives a predictable home for the future resolver/crawler) |
| M3 | Reference syntax | New `use: name@ref` key on `modules:` (backwards compatible — `source:` still works). Rejected `source: "marketplace:beads"` — a no-`://` string is misread as a local path (C1) |
| M4 | Lifecycle field | **None.** No `maturity`/`lifecycle` field — nothing consumes it (YAGNI). "Not resolvable yet" is expressed structurally (no pinned `ref`/`artifact`), not by a label. Add such a field only when a feature reads it (a `rw marketplace list` column, a deprecation warning) |
| M5 | Namespace | `.wt/weaver/` in every repo (org-namespaced); k8s-style `apiVersion: wt.weaver/v1` + `kind:` headers |
| M6 | v1 plugin entries | The catalog schema carries plugins, but v1 ships **no live plugin entry** — a commented example shows the shape (beads-init isn't built yet, so no dangling unresolvable entry) |

## Engine seams this lands on (verified against code)

- `PluginSource` = `Local | Git | Registry` (`plugin/mod.rs:16`); `resolve_ensure_type`
  maps ensure `a.b` → plugin `a-b` and **defaults to `Registry`** (`resolver.rs:76-88`).
- `get_registry_url()` = `$RW_REGISTRY_URL` → `// TODO: Check config file` →
  fantasy default (`resolver.rs:382-392`). **The catalog is that config file.**
- Fetcher already downloads `plugin.wasm` from a **GitHub release**
  (`fetcher.rs:34`) — so `artifact: release` needs no new fetch path.
- Modules have **no** registry concept — `modules: Vec<ModuleConfig{name,source,ref,path}}`
  (`config.rs`), `path` parsed but unread (C1). `use:` resolution is net-new.

## Artifacts

### A. Self-describing manifest — in each publishing repo

Discovery metadata only. Does **not** re-declare the module's inputs/tasks (those
stay authoritative in `weaver.module.yaml`; no drift).

```yaml
# beads-module/.wt/weaver/manifest.yaml
apiVersion: wt.weaver/v1
kind: Manifest
provides:
  - name: beads
    kind: module            # module | plugin
    root: "."               # where weaver.module.yaml lives (repo root today; subdir ⇒ needs G4)
    description: "Onboard a repo to the shared beads (bd) tracker (Dolt over Tailscale)."
    maintainers: [max]
```

### B. Central catalog — in `Web-tree/weaver-marketplace`

```yaml
# weaver-marketplace/.wt/weaver/marketplace.yaml
apiVersion: wt.weaver/v1
kind: Marketplace
entries:
  - name: beads
    kind: module
    source: "https://github.com/Web-tree/beads-module.git"
    ref: "v0.1.0"
    root: "."
    description: "Onboard a repo to the shared beads (bd) tracker."

# --- plugin entry shape (schema carries it; no live plugin yet) ---
# - name: beads-init
#   kind: plugin
#   source: "https://github.com/Web-tree/beads-module.git"
#   ref: "v0.1.0"
#   wit_world: "ensure-provider"
#   provides_ensure: "beads.init"   # ensure type this plugin backs
#   artifact: release               # fetch plugin.wasm from the GH release
```

### C. Consumer resolution — the LATER pass, sketched for shape

```yaml
marketplace:
  source: "https://github.com/Web-tree/weaver-marketplace.git"
  ref: main
modules:
  - use: "beads@v0.1.0"     # engine expands via catalog → source + ref + root
```

Engine changes (next pass, not now): parse `marketplace:` + `use:`; expand `use:`
against the fetched catalog; back `get_registry_url()`'s TODO with the catalog;
use `provides_ensure` so `resolve_ensure_type` finds a plugin's real repo/release
instead of URL-templating the fantasy domain.

## What ships in THIS pass (all authorable today — no engine change)

1. **`Web-tree/beads-module`** — the module (per the beads spec) **+
   `.wt/weaver/manifest.yaml`**.
2. **`Web-tree/weaver-marketplace`** — `.wt/weaver/marketplace.yaml` with the
   beads entry + the commented plugin example. Index-only: human/agent-facing,
   **no engine reads it yet**.
3. **repo-weaver dogfood** — `weaver.yaml` uses direct `source:` (resolution
   engine doesn't exist); the catalog entry's `source`/`ref` are kept in sync by
   hand and cross-checked against what the dogfood uses.
4. **repo-weaver §6 backlog** — gains the marketplace engine items (below).

## Out of scope (this pass)

- The resolution engine: `marketplace:`/`use:` parsing, catalog fetch/expand,
  catalog-backed plugin registry, `provides_ensure` wiring.
- Any crawler/aggregator (federation) — hybrid stays centralized for v1.
- `rw marketplace` CLI verbs (list/search/add).
- Subpath sourcing (G4) — a *hosting* monorepo would need it; a *pointing* index
  does not, so we don't need it now.

## Extract-backlog additions (fold into the beads spec §6)

| ID | Gap |
|----|-----|
| G6 | Resolution engine: `marketplace:` config + `use: name@ref` expansion; catalog backs `get_registry_url()`'s TODO; `provides_ensure` replaces the fantasy plugin default |
| G7 | `rw module new` should scaffold `.wt/weaver/manifest.yaml` alongside the module (ties to G1) |
| G8 | `rw marketplace` verbs (list/search/validate) — validate that catalog `source`/`ref` resolve and match the target repo's self-manifest |
```
