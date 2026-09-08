# 23 - AI Agent Skills + AGENTS.md Invocation Order

Demonstrates the new `ensure.file.md_section` ensure by solving a concrete, real-world problem: **install a few Claude skills and document the order in which agents should invoke them inside `AGENTS.md`** — without clobbering the user's existing content.

## What this covers

- `ensure.file.md_section` with **both v1 selector types** side-by-side on the same file
  - `selector.type: "heading"` — path-based CommonMark heading targeting (`## Skills`)
  - `selector.type: "block_marker"` — HTML-comment delimited region (`<!-- rw:section id="..." -->`)
- `ensure.file.from_template` for whole-file skill definitions (plain path, already covered by example 02)
- Preserving pre-existing user content in `AGENTS.md` across `wvr apply` runs
- Rendering lists and per-item catalogues in Tera templates

## Why two selector kinds?

Markdown files in the wild don't have a single "right way" to carve them up, so `ensure.file.md_section` supports a pluggable selector (see `specs/001-repo-weaver-mvp/contracts/weaver.yaml.md` §`ensure.file.md_section`). This example uses both v1 kinds to make the choice concrete:

- **`heading`** — best when the managed region is a logical part of the document's outline that humans will navigate (`## Skills`). Survives reformatting around it as long as the heading itself stays.
- **`block_marker`** — best for volatile content that shouldn't interfere with heading structure (`recent-changes`). Survives structural rearrangement because the boundary is explicit.

Reserved selector types (`mdast`, `mdq`, `regex`, `frontmatter`, `line_range`) are not shipped in v1 but are reserved to keep the namespace clean as the primitive grows.

## Module contents

- `weaver.module.yaml` — inputs: `project_name`, `skills`, `workflow`, `recent_changes`.
- `templates/.claude/skills/plan-change/SKILL.md.j2`, `.../write-tests/SKILL.md.j2`, `.../review-diff/SKILL.md.j2` — three whole-file skill definitions with YAML frontmatter (`name`, `description`, `allowed-tools`).
- `sections/skills.md.j2` — Tera template for the `## Skills` section body; renders the skill catalogue and the numbered invocation order.
- `sections/recent-changes.md.j2` — Tera template for the marker-delimited recent-changes block.

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:

- `app/.claude/skills/plan-change/SKILL.md`, `.../write-tests/SKILL.md`, `.../review-diff/SKILL.md` — three skill files rendered from templates.
- `app/AGENTS.md` has:
  - The pre-existing `# acme-api` intro paragraph — **unchanged**.
  - The pre-existing `## Manual Additions` section — **unchanged**.
  - A new `## Skills` section with the skill catalogue and a numbered invocation order (heading selector).
  - A `<!-- rw:section id="recent-changes" -->…<!-- rw:endsection id="recent-changes" -->` block containing the recent-changes bullets (block-marker selector).

## Drift behaviour

Try these after a successful `wvr apply`:

- Edit a bullet inside the `## Skills` section → `wvr plan` exits non-zero with drift on that region only.
- Edit `## Manual Additions` → never flagged; it's not managed.
- Add a wholly new `## Glossary` heading anywhere in the file → preserved; Weaver doesn't care about unmanaged siblings.
- Change `skills` / `workflow` inputs in `weaver.yaml` and re-apply → the `## Skills` region is updated; `## Manual Additions` and the `## Glossary` you added still survive.

## Why put skill ordering in AGENTS.md?

The `.claude/skills/` directory is read natively by Claude Code, but Cursor, Copilot, Codex CLI, Gemini CLI, Aider, and most other agents do not consume it. They all read `AGENTS.md` (the [Linux Foundation open standard](https://agents.md/)). Documenting the workflow in `AGENTS.md` makes the invocation order visible to *every* agent, while the `.claude/skills/` files give Claude Code the operational detail.
