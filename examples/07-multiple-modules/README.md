# 07 - Multiple Modules

Demonstrates multiple modules, each used by a different app in the same workspace.

## What this covers

- Composition and reuse (PRD §2.3)
- Multiple module sources
- Each app gets files only from its referenced module

## Module contents

- `module-backend/files/server.conf` — backend config
- `module-frontend/files/nginx.conf` — frontend config

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `api/server.conf` from module-backend
- `web/nginx.conf` from module-frontend
- No cross-contamination between apps
