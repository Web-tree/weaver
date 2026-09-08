# app-b

A fuller service that uses the complete automation catalogue. Hand-written intro — Weaver must not clobber this.

## Automation

Claude Code picks these up automatically from `.claude/agents/` and `.claude/commands/`. Other agents should be instructed to delegate to them or call them by the equivalent manual step.

### Subagents

- **`test-runner`** — Runs the test suite in a clean subagent context and reports a short, actionable summary.
- **`security-reviewer`** — Read-only review of the staged diff for common security issues.

### Slash commands

- **`/release`** — Cut a release: bump version, tag, push.
- **`/sync-deps`** — Resync dependencies against the lockfile and report drift.
