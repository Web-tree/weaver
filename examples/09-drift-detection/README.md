# 09 - Drift Detection

Demonstrates drift detection when a user manually edits a managed file.

## What this covers

- Drift detection (PRD §9.3)
- Managed file safety (PRD §9.2)
- `--strategy overwrite` to resolve drift
- State tracking with SHA-256 checksums

## Before state

- Module has `files/config.txt` with original content
- `app/config.txt` has been manually edited by the user (drift)
- `.rw/state.yaml` has the checksum of the original content

## How to run

```sh
cd before

# Plan detects drift
wvr plan                          # exits with error: "Drift detected"

# Apply with overwrite resolves it
wvr apply --strategy overwrite --auto-approve
```

## Expected result

After `wvr apply --strategy overwrite`, `before/` should match `after/`:
- `app/config.txt` restored to original module content
- `.rw/state.yaml` updated with current checksum
