# 28 - Module Sub-Path Dependencies

> **Status:** design-only / pending implementation. Demonstrates the desired shape of a `paths:` selector on `modules:` plus per-file integrity hashes in `weaver.lock`. Same conventions as examples 18–25.

Demonstrates how to depend on **specific files inside a multi-asset upstream repo** — for example, one skill from `anthropics/skills`, or one agent from `github/awesome-copilot` — without pulling the entire repo into the module cache.

## What this covers

- A new optional `paths:` field on each entry in `modules:` (top-level glob list).
- Module fetcher (`crates/core/src/module.rs`) materialises only matching paths under `~/.rw/store/<source>/<ref>/`, leaving the rest unfetched.
- `weaver.lock` (`crates/core/src/lockfile.rs`) records ref **plus per-file `sha256` hashes** for every materialised path. Drift on any one file is detectable independently of the others.
- Backwards compatible: omitting `paths:` keeps today's whole-repo behaviour.
- Composes with existing ensures — e.g. `ensure.file.from_module` referencing `skills/frontend-design/SKILL.md` still works because the file is on disk under the module's store path.

## Why a sub-path selector

Today every module pull is "whole git repo". That works for purpose-built modules but not for **curated upstream collections** like:

- `anthropics/skills` → ships dozens of independent skill folders.
- `github/awesome-copilot` → ships dozens of agents and prompts as individual files.

Pulling the entire repo to use one skill is wasteful and over-broadens drift detection (an unrelated upstream change to a different skill would still touch the cache for this repo). APM solves this by allowing dependency entries like `anthropics/skills/skills/frontend-design` and `github/awesome-copilot/agents/api-architect.agent.md` — i.e. a `repo + sub-path` tuple. This example brings the same capability to Weaver.

## Comparison to APM

```yaml
# APM (apm.yml)
dependencies:
  apm:
    - anthropics/skills/skills/frontend-design
    - github/awesome-copilot/agents/api-architect.agent.md

# Weaver (this example)
modules:
  - name: frontend-skill
    source: https://github.com/anthropics/skills
    ref: v1.2.0
    paths:
      - skills/frontend-design/**
  - name: api-architect
    source: https://github.com/github/awesome-copilot
    ref: main
    paths:
      - agents/api-architect.agent.md
```

Same outcome, expressed in Weaver's existing `modules:` schema.

## Lockfile shape (proposed)

`weaver.lock` gains a `files:` map per module entry:

```yaml
version: "1"
modules:
  frontend-skill:
    source: https://github.com/anthropics/skills
    ref: v1.2.0
    files:
      "skills/frontend-design/SKILL.md":
        sha256: "<hex>"
      "skills/frontend-design/examples/button.tsx":
        sha256: "<hex>"
      "skills/frontend-design/examples/form.tsx":
        sha256: "<hex>"
  api-architect:
    source: https://github.com/github/awesome-copilot
    ref: main
    files:
      "agents/api-architect.agent.md":
        sha256: "<hex>"
```

Modules without `paths:` keep today's single `checksum:` field for the whole tree — no migration required.

## How to run

```sh
cd before
wvr plan        # Shows what would be fetched
wvr apply       # Materialises only the listed paths
```

## Expected result

- `~/.rw/store/<source>/<ref>/skills/frontend-design/**` is populated.
- `~/.rw/store/<source>/<ref>/skills/<other>/**` is **not** present (unfetched siblings).
- `weaver.lock` lists each fetched file with its `sha256`.
- Apps reference the fetched files via `ensure.file.from_template` as today; no change to the ensures themselves.

## Drift behaviour

- Bump `ref:` for one module → only that module's files are re-fetched and re-hashed; other modules' lock entries unchanged.
- Upstream force-pushes a tag → checksum mismatch on the affected file aborts apply with a clear error (same EC-002 contract as today, but at file granularity).

## References

- [crates/core/src/module.rs](../../crates/core/src/module.rs) — module resolver
- [crates/core/src/lockfile.rs](../../crates/core/src/lockfile.rs) — lockfile schema
- [APM dependency syntax](https://github.com/microsoft/apm) — the upstream pattern this mirrors
