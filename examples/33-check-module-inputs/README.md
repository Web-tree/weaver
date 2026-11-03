# 33 - Module Inputs in Checks (`WVR_INPUT_*`)

Demonstrates how a module's `checks:` read the consuming app's inputs: every
resolved input is exported to the check's shell as `WVR_INPUT_<NAME>`.

## What this covers

- `web_path` is set by the app (`apps/site`) and overrides the module default
  (`apps/web`); `node_version` and `required_scripts` fall back to their
  defaults.
- A string arrives as-is (`$WVR_INPUT_WEB_PATH` is `apps/site`), a number in
  decimal (`$WVR_INPUT_NODE_VERSION` is `22`), and a list as compact JSON
  (`$WVR_INPUT_REQUIRED_SCRIPTS` is `["build","test"]`).
- Check commands are not templated: `caller-concurrency` greps for the literal
  `${{ github.workflow }}-${{ github.ref }}`, which reaches the shell unchanged.

## How to run

```sh
cd before
wvr check
```

## Expected result

All four checks pass and nothing in the tree changes, so `after/` is identical
to `before/`. Deleting `apps/site/.nvmrc` or the `test` script fails the
matching check.
