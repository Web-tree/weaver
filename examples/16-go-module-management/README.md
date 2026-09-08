# 16 - Go Module Dependency Management

Demonstrates Go dependency convergence using native `go` tooling.

## What this covers

- `ensure.go.module_dep` with explicit versions (PRD §6)
- `ensure.go.tidy` to normalize transitive module metadata (PRD §6)
- Native-tool behavior with `go get` and `go mod tidy` (PRD §2)

## Before state

- Service has a minimal `go.mod` and only one direct dependency
- HTTP router and structured logging dependencies are missing
- `go.sum` does not contain checksums for new modules

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `go.mod` contains required direct dependencies at target versions
- `go.sum` reflects a tidy module graph
- A static operational runbook from the module is copied into the service
