# 06 - Config Includes

Demonstrates config splitting using `weaver.d/` auto-discovery for modular
configuration management.

## What this covers

- Tree-like configs (PRD §5.2)
- `weaver.d/` auto-discovery and merging
- Arrays concatenate (modules, apps), maps override (secrets)
- Deterministic ordering (alphabetical by filename)

## Config structure

- `weaver.yaml` — root config with version and modules
- `weaver.d/apps.yaml` — apps defined in a separate fragment

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- Modules from root config and apps from fragment are merged
- `app/settings.txt` copied from module
