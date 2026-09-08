# 27 - AI Agent Hooks

> **Status:** design-only / pending implementation. Demonstrates the desired shape of a new primitive `ensure.hook` (and shared JSON-merge machinery with example 26). Same conventions as examples 18–25.

Demonstrates how a module declares **Claude Code hooks** (`PreToolUse`, `PostToolUse`, `SessionStart`, `Stop`, `UserPromptSubmit`, etc.) and the engine merges them into `.claude/settings.json` without overwriting the user's other settings keys.

## What this covers

- A new ensure: `ensure.hook` with fields `event`, `matcher` (optional; semantics depend on `event` — see below), `command`, `timeout` (optional), `targets`.
- **Same JSON-merge guarantee as example 26**: pre-existing `permissions`, `model`, `mcpServers`, etc. are preserved byte-for-byte; only the `hooks` block is managed.
- Idempotency: re-applying a hook with the same `event` + `matcher` + `command` is a no-op (no duplicate entries).
- Modules can ship the hook command itself as a template (`scripts/redact-secrets.sh.j2`) — same `ensure.file.from_template` we already use.

### `matcher` semantics by event

| Event | `matcher` meaning |
|---|---|
| `PreToolUse`, `PostToolUse` | Regex on tool name (e.g. `Bash`, `Read`). |
| `SessionStart` | Source string: `startup` / `resume` / `clear` / `compact`. Omit to match all. |
| `Stop`, `Notification`, `SubagentStop`, `PreCompact`, `UserPromptSubmit` | Not applicable; omit. |

## Why a dedicated primitive

`ensure.file.from_template` could write a whole `settings.json`, but it cannot **merge** into a JSON file with user-managed keys. Hooks are the second canonical "structured insert into a shared JSON file" use case after MCP servers (example 26) — both share the same JSON-merge engine. Without a dedicated primitive, every hook-shipping module would conflict with every other module that touches `settings.json`.

## Comparison to APM

[microsoft/apm](https://github.com/microsoft/apm) lists `hooks` as one of its seven primitives. APM resolves them at install time; Weaver applies them as part of the same plan/apply convergence loop as every other ensure, so drift detection works the same way (edit a hook by hand → `wvr plan` flags it).

## Module contents

- `weaver.module.yaml` — inputs: `project_name`, `enable_secret_redaction` (`bool`), `enable_session_banner` (`bool`).
- `templates/scripts/redact-secrets.sh.j2` — the script the `PreToolUse` hook invokes; rendered into the app so the hook command points at a real file.

## How to run

```sh
cd before
wvr apply
```

## Expected result

- `app/.claude/settings.json` — the pre-existing `permissions` and `model` keys are preserved; a new `hooks` block is added with one `PreToolUse` entry (matcher `Bash`, command `./scripts/redact-secrets.sh`) and one `SessionStart` entry.
- `app/scripts/redact-secrets.sh` — rendered from the template, executable.

## Drift behaviour

- Edit the `command` of a managed hook → `wvr plan` flags drift.
- Add an unrelated `env` key to `settings.json` → never flagged.
- Set `enable_secret_redaction: false` and re-apply → only that hook is removed; the `SessionStart` hook and unmanaged keys survive.

## References

- [Claude Code hooks](https://code.claude.com/docs/en/hooks)
- [Example 26](../26-ai-agent-mcp-servers/) — the sibling primitive that shares the JSON-merge engine.
