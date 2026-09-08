# 22 - Module Ref Bump (`wvr module update`)

Demonstrates the CLI workflow for bumping a pinned module reference in
workspace config.

## What this covers

- `wvr module list` to inspect current module refs (PRD §8)
- `wvr module update <name> --ref <newRef>` to update a pinned ref (PRD §8)
- Update intent captured in config before running `wvr apply`

## How to run

```sh
cd before
wvr module list
wvr module update platform-standards --ref v2
```

## Expected result

After `wvr module update`, `before/weaver.yaml` should match `after/weaver.yaml`
with the module ref changed from `v1` to `v2`.
