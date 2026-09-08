# 25 - Claude Subagents + Slash Commands + Automation Section

Demonstrates Claude Code's two remaining first-party extension points — **subagents** (`.claude/agents/<name>.md`) and **slash commands** (`.claude/commands/<name>.md`) — and reuses `ensure.file.md_section` from example 23 to describe them in `AGENTS.md`, so non-Claude agents know they exist.

> Skills live in example 23; this example deliberately does not overlap.

## What this covers

- `ensure.file.from_template` with nested paths (`.claude/agents/`, `.claude/commands/`) — already covered by example 02, included here for the Claude-specific use case.
- `when:` guards on ensures (example 24) used per-subagent and per-command — `list(string)` selectors again.
- `ensure.file.md_section` on `AGENTS.md` to document what's installed — same primitive as example 23, different content.

## Files generated

Each subagent and slash command is a Markdown file with YAML frontmatter:

- `.claude/agents/<name>.md` — frontmatter keys: `name`, `description`, `tools`, `model`.
- `.claude/commands/<name>.md` — frontmatter keys: `description`, `allowed-tools`.

## Module contents

- `weaver.module.yaml` — inputs: `project_name`, `subagents` (`list(string)`), `commands` (`list(string)`).
- `templates/.claude/agents/test-runner.md.j2` — runs the test suite in an isolated context and summarises.
- `templates/.claude/agents/security-reviewer.md.j2` — read-only security review of the staged diff.
- `templates/.claude/commands/release.md.j2` — `/release` slash command.
- `templates/.claude/commands/sync-deps.md.j2` — `/sync-deps` slash command.
- `sections/automation.md.j2` — AGENTS.md `## Automation` section body.

## How to run

```sh
cd before
wvr apply
```

## Expected result

- `app-a` (`subagents: ["test-runner"]`, `commands: ["release"]`) gets two `.claude/` files and an Automation section listing them.
- `app-b` (full catalogue) gets four `.claude/` files and an Automation section listing all four.
- Each app's pre-existing `AGENTS.md` intro paragraph is preserved byte-for-byte.

## Why AGENTS.md also?

Claude Code reads `.claude/agents/` and `.claude/commands/` directly. Cursor, GitHub Copilot, Gemini CLI, Codex CLI, and Aider do not. When `AGENTS.md` lists the available automations, those other agents can delegate (or perform the equivalent steps manually) rather than being unaware that the tooling exists. Same reason skills got an AGENTS.md section in example 23.

## References

- [Claude Code subagents](https://code.claude.com/docs/en/subagents)
- [Claude Code slash commands](https://code.claude.com/docs/en/slash-commands)
- [AGENTS.md open standard](https://agents.md/)
