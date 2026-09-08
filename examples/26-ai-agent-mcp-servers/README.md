# 26 - AI Agent MCP Servers

> **Status:** design-only / pending implementation. Demonstrates the desired shape of a new primitive `ensure.mcp.server`. Not yet wired into the engine — same status as examples 18–25.

Demonstrates how a module declares **MCP (Model Context Protocol) servers** and the engine merges them into the right config file for each selected agent — without overwriting unrelated settings the user already has.

## What this covers

- A new ensure: `ensure.mcp.server` with fields `name`, `transport` (`http` | `stdio`), `url` / `command` / `args`, `headers`, `env`, `targets`.
- Multi-target rendering: a single declaration lands in `.mcp.json`, `.claude/settings.json` (`mcpServers` key), and `.cursor/mcp.json` — chosen by `targets:`.
  - `targets: ["claude"]` writes both `.mcp.json` (project-scoped, shared across Claude clients) and `.claude/settings.json` (user-scoped).
  - `targets: ["cursor"]` writes `.cursor/mcp.json`.
- **Merge, don't overwrite:** existing unrelated keys in `settings.json` (e.g. `permissions`, `model`) are preserved. Only the `mcpServers` block is managed.
- Secret references via `${{ secrets.* }}` — same resolver as the existing `secrets:` block (no plaintext in `weaver.yaml`).
- Re-uses the `when:`/`targets:` selector pattern from example 24 — "selecting tools = selecting which files to sync".

## Why a dedicated primitive

Today `ensure.file.from_template` can render a whole `.mcp.json`, but it cannot **merge** into a JSON file that already has user-managed keys. MCP server config is the canonical "structured insert into a shared JSON file" use case — exactly the JSON analogue of `ensure.file.md_section`. Without it, every module that wants to add an MCP server has to own the entire `settings.json`, which conflicts with anything else (hooks, permissions, model defaults).

## Comparison to APM

[microsoft/apm](https://github.com/microsoft/apm) treats MCP servers as a top-level primitive in `apm.yml`:

```yaml
dependencies:
  mcp:
    - name: io.github.github/github-mcp-server
      transport: http
```

This example brings the same capability into Weaver while keeping the `ensure.*` model — modules describe MCP servers as ensures, not as a separate dependency category, and the same `targets:` mechanism that selects per-tool agent files (example 24) selects per-tool MCP config files.

## Module contents

- `weaver.module.yaml` — inputs: `project_name` (string), `tools` (`list(string)`, subset of `claude`/`cursor`).
- Secret: `GITHUB_PAT` is sourced from the top-level `secrets:` block in `weaver.yaml`, not as a module input. The ensure references it via `${{ secrets.GITHUB_PAT }}` and the rendered files emit a literal `${GITHUB_PAT}` (resolved at process spawn by the MCP client, never written as plaintext).

## targets vs inputs.tools

`inputs.tools` is informational — it documents which clients the app supports. The authoritative selector is `targets:` on each ensure. In this example app-b sets `tools: ["claude"]` and accordingly each ensure has `targets: ["claude"]`. The engine does not auto-intersect; mismatched declarations are the caller's responsibility.

## Two apps in one config

- `app-a` selects `[claude, cursor]` → gets `.mcp.json` + `.claude/settings.json` + `.cursor/mcp.json`.
- `app-b` selects `[claude]` only → gets `.mcp.json` + `.claude/settings.json`. Has a pre-existing `.claude/settings.json` with `permissions` and `model` keys; the apply must preserve those byte-for-byte.

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:

- `app-a/.mcp.json` — both servers, project-scoped.
- `app-a/.claude/settings.json` — `mcpServers` block matches, no other keys.
- `app-a/.cursor/mcp.json` — both servers in Cursor's format.
- `app-b/.mcp.json` — both servers.
- `app-b/.claude/settings.json` — `mcpServers` block added; pre-existing `permissions` and `model` keys still there.

## Drift behaviour

- Hand-edit a server's `url` in `.claude/settings.json` → `wvr plan` flags drift on that block only.
- Hand-edit `permissions` (unmanaged) → never flagged.
- Add a brand-new server to `weaver.yaml` and re-apply → block is updated, unrelated keys still survive.

## References

- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Claude Code MCP configuration](https://code.claude.com/docs/en/mcp)
- [APM MCP dependency declaration](https://github.com/microsoft/apm)
