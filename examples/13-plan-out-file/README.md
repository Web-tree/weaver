# 13 - Plan Output File Safety Check

Demonstrates plan-file safety semantics inspired by Terraform workflows: if the
workspace changes after `wvr plan --out`, `wvr apply` with that saved plan must fail
instead of applying stale intent.

## What this covers

- `wvr plan --out` creates a reusable plan artifact
- `wvr apply --plan <file>` validates the workspace fingerprint before applying
- Apply fails safely when config/input drift occurs between plan and apply

## Before state

This fixture represents a workspace **after** a plan was generated and **after**
someone edited `weaver.yaml`:

- Saved plan (`.rw/plan.json`) was created with `retention_days: 30`
- Current `weaver.yaml` now has `retention_days: 90`
- `app/policies.yaml` has not been written yet

## How to run

```sh
cd before
wvr apply --plan .rw/plan.json
```

## Expected result

`wvr apply --plan .rw/plan.json` fails with a stale-plan error (fingerprint/input
mismatch), and no files are changed:

- `app/policies.yaml` remains absent
- `.rw/plan.json` remains unchanged
- Workspace still matches `after/`
