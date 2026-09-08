# 15 - Node.js npm Ensures

Demonstrates Node.js package management via native npm commands.

## What this covers

- `ensure.npm.script` for lifecycle scripts (PRD §6)
- `ensure.npm.dep` and `ensure.npm.devDep` for dependency convergence (PRD §6)
- `ensure.npm.engine` for runtime policy (PRD §6)
- Native-tool workflow (`npm pkg set`) instead of hand-editing JSON (PRD §2)

## Before state

- `services/web/package.json` exists with basic scripts
- `build`, `lint`, and `test` scripts are missing
- Required dependencies and engine constraints are missing

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `package.json` includes standardized scripts and dependency pins
- `engines.node` enforces the team's supported Node range
- Static docs from the module are copied to the app folder
