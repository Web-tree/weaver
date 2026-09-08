# Weaver: Gap Analysis vs PRD

**Created**: 2026-01-03  
**Reference**: [PRD.md](../PRD.md) Section 12 (MVP definition)

This document tracks the gaps between the original PRD requirements and current implementation status.

---

## ✅ Implemented (001-repo-weaver-mvp)

| Feature | PRD Section | Status |
|---------|-------------|--------|
| YAML loader with modules/apps | §5.1 | ✅ Complete |
| Apps with path scoping | §4.2 | ✅ Complete |
| Modules pinned by git ref | §4.3 | ✅ Complete |
| Global cache `~/.rw/store` | §5.3 | ✅ Complete |
| Answers storage + prompting | §5.3 | ✅ Complete |
| `ensure.folder.exists` | §6 | ✅ Complete |
| `ensure.file.from_template` | §6 | ✅ Complete |
| `ensure.task.wrapper` | §6 | ✅ Complete |
| `wvr init` | §8 | ✅ Complete |
| `wvr plan` | §8 | ✅ Complete |
| `wvr apply` | §8 | ✅ Complete |
| `wvr run` | §8 | ✅ Complete |
| Pipeline tasks with JSON capture | §7.1-7.2 | ✅ Complete |
| AWS SSM WASM plugin | §6 (secrets) | ✅ Complete |
| Secret redaction (`Secret<T>`) | §9.5 | ✅ Complete |
| Drift detection | §9.3 | ✅ Complete |
| Lockfile integrity check | §9.4 | ✅ Complete |
| Offline fallback to cache | Edge case | ✅ Complete |

---

## ❌ Not Implemented (Gaps)

### Priority 1 (Critical for MVP)

| Feature | PRD Section | Spec | Notes |
|---------|-------------|------|-------|
| `includes` YAML merging | §5.1-5.2 | 002 | Tree-like config, deep merge maps, concat arrays |
| `wvr list` command | §8 | 002 | Show apps and tasks |
| `ensure.git.submodule` | §6, §12 | 002 | Vendor upstream dependencies |
| `ensure.git.clone_pinned` | §6, §12 | 002 | Alternative to submodules |
| k3s-nebula validation | §10, §14 | 002 | End-to-end acceptance criteria |

### Priority 2 (Important)

| Feature | PRD Section | Spec | Notes |
|---------|-------------|------|-------|
| `wvr describe <app>` | §8 | 002 | Show resolved config after merges |
| `wvr check [app]` | §8 | 002 | Run validation checks |
| `wvr module list` | §8 | 002 | List modules with source/ref |
| `wvr module update` | §8 | 002 | Update pinned ref in config |
| `ensure.npm.script` | §6, §12 | 002 | Use `npm pkg set` |
| Task composition (`call`) | §7.3 | 002 | Call other tasks |
| Import tasks from modules | §7.3 | 002 | Reuse module tasks |

### Priority 3 (Nice to Have for MVP)

| Feature | PRD Section | Spec | Notes |
|---------|-------------|------|-------|
| `ensure.ai.patch` | §4.6, §6, §12 | 002 | Diff, verify, rollback |
| `--from <stepId>` for `wvr run` | §8 | - | Resume from step |

---

## Future (Not MVP)

These features are mentioned in the PRD but explicitly out of scope for MVP:

| Feature | PRD Section | Notes |
|---------|-------------|-------|
| `ensure.go.module_dep` | §6 | Go ecosystem |
| `ensure.cargo.dep` | §6 | Rust ecosystem |
| `ensure.tf.vars_file` | §6 | Terraform ecosystem |
| `ensure.kubectl.apply` | §6 | Kubernetes ecosystem |
| `ensure.kustomize.resource` | §6 | Kubernetes ecosystem |
| `ensure.helm.release` | §6 | Kubernetes ecosystem |

---

## Implementation Order

Recommended order based on dependencies:

```
Phase 1: Config Foundation
├── includes YAML merging (FR-001, FR-002, FR-003)
└── wvr list command (FR-004)

Phase 2: Git Ensures (P1)
├── ensure.git.submodule (FR-009)
└── ensure.git.clone_pinned (FR-010)

Phase 3: CLI Commands (P2)
├── wvr describe (FR-005)
├── wvr check (FR-006)
└── wvr module list/update (FR-007, FR-008)

Phase 4: Ecosystem Ensures (P2)
└── ensure.npm.script (FR-011)

Phase 5: Advanced (P3)
└── ensure.ai.patch (FR-012)

Phase 6: Validation
└── k3s-nebula end-to-end test
```

---

## Tracking

| Spec | Branch | Status |
|------|--------|--------|
| 001-Weaver-mvp | `001-Weaver-mvp` | ✅ Complete |
| 002-gap-analysis | `002-gap-analysis` | 📝 In Progress |
