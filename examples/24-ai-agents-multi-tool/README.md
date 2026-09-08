# 24 - AI Agent Multi-Tool Selector

Demonstrates how to generate the right agent-config file for each selected AI tool — **without** adding any AI-agent-specific schema to `weaver.yaml`. The selector is just a `list(string)` input plus a generic `when:` guard on each ensure.

## What this covers

- Generic `when:` guard on ensures (PRD §6; contract: `specs/001-repo-weaver-mvp/contracts/weaver.yaml.md` §"Conditional ensures")
- Rendering templates into nested paths (`.cursor/rules/project.mdc`, `.github/copilot-instructions.md`)
- Three apps sharing the same module but selecting different agent subsets
- YAML anchors (`&tool_ensures` / `*tool_ensures`) to share ensure lists across apps

## Covered agents (top 6 + universal)

| Agent | Generated path | Format |
|---|---|---|
| — | `AGENTS.md` | [Universal open standard](https://agents.md/) (always generated) |
| Claude Code | `CLAUDE.md` | Markdown |
| Cursor | `.cursor/rules/project.mdc` | `.mdc` with YAML frontmatter (`alwaysApply`, `description`, `globs`) |
| GitHub Copilot | `.github/copilot-instructions.md` | Markdown |
| Windsurf | `.windsurfrules` | Plain text / Markdown |
| Gemini CLI | `GEMINI.md` | Markdown |
| Aider | `CONVENTIONS.md` + `.aider.conf.yml` | Markdown + YAML config wiring |

## Why no AI-specific schema?

The user's core observation: "selecting which agents to use" is just "selecting which files to sync". We already have file sync (`ensure.file.from_template`, example 02). The only missing piece was a generic conditional on ensures — which is useful for many reasons beyond AI agents (feature flags, environment-specific files, opt-in integrations). Hence `when:`, not `agents:`.

## Module contents

- `weaver.module.yaml` — inputs: `project_name`, `language`, `conventions`, `agents` (`list(string)` defaulting to `["claude", "cursor", "copilot"]`).
- `templates/AGENTS.md.j2` — the universal file. Always generated. Lists which per-tool files exist.
- `templates/CLAUDE.md.j2`, `.cursor/rules/project.mdc.j2`, `.github/copilot-instructions.md.j2`, `.windsurfrules.j2`, `GEMINI.md.j2`, `CONVENTIONS.md.j2`, `.aider.conf.yml.j2` — one per tool; gated by `when:` in `weaver.yaml`.

## How to run

```sh
cd before
wvr apply
```

## Expected result

Three app directories, each with a different set of files:

- `app-a` (`agents: ["claude", "cursor"]`) → `AGENTS.md`, `CLAUDE.md`, `.cursor/rules/project.mdc`.
- `app-b` (`agents: ["copilot", "windsurf", "gemini", "aider"]`) → `AGENTS.md`, `.github/copilot-instructions.md`, `.windsurfrules`, `GEMINI.md`, `CONVENTIONS.md`, `.aider.conf.yml`.
- `app-c` (default `["claude", "cursor", "copilot"]`) → `AGENTS.md`, `CLAUDE.md`, `.cursor/rules/project.mdc`, `.github/copilot-instructions.md`.

Re-running `wvr apply` is a no-op. Removing an agent from the list and re-applying deletes the corresponding managed file (standard drift-safe removal behaviour).

## Switching the selector at runtime

Every ensure input is overridable via `--set`, so you can flip the selector without editing `weaver.yaml`:

```sh
wvr apply --set app-a.agents='["claude","cursor","copilot","windsurf","gemini","aider"]'
```
