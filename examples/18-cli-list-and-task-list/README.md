# 18 - CLI Discovery (`wvr list` and `wvr task list`)

Demonstrates workspace/app discovery commands that do not mutate files.

## What this covers

- `wvr list` app/module discovery (PRD §8)
- `wvr task list [app]` task discovery from app-local task definitions (PRD §8)
- Non-mutating CLI workflows for CI/operator introspection

## How to run

```sh
cd before
wvr list
wvr task list web
wvr task list infra
```

## Expected result

`wvr list` and `wvr task list` should print the apps/tasks declared in `weaver.yaml`.
Workspace files remain unchanged, so `before/` and `after/` are identical.
