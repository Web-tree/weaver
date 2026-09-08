# 01 - Basic Static Files

Demonstrates the simplest use case: a module with static files that get copied
verbatim into the app directory.

## What this covers

- `ensure.file.copy` (PRD §6)
- Basic `wvr apply` workflow (PRD §8)
- State tracking in `.rw/state.yaml`

## Module contents

- `files/config.txt` — a plain config file
- `files/scripts/run.sh` — a nested script file

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `app/config.txt` copied verbatim from module
- `app/scripts/run.sh` copied verbatim from module
- `.rw/state.yaml` tracks both files with SHA-256 checksums
