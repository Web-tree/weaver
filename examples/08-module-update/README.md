# 08 - Module Update

Demonstrates updating a module from v1 to v2. The workspace has already been
applied with v1, and the config now points to v2.

## What this covers

- Updates without re-copying everything (PRD §2.4)
- Idempotency (PRD §9.1)
- State tracking across module version changes

## Module versions

- `module-v1/files/config.txt` — v1 content
- `module-v2/files/config.txt` — v2 content with new settings

## Before state

- `weaver.yaml` points to module-v2
- `app/config.txt` still has v1 content (from previous apply)
- `.rw/state.yaml` has checksum of v1 content

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `app/config.txt` updated to v2 content
- `.rw/state.yaml` updated with v2 checksum
- No drift error because the file matches the tracked state from v1
