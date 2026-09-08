# 30 - tmux window name follows Claude Code's title

Demonstrates converging a managed block inside a **plain-text** config that the
user also hand-edits — here, a tmux config — by solving a concrete problem:
**make the tmux window name (`#W`) track the terminal title (`#T`) that Claude
Code sets**, so a `/rename` (or any task-summary update) inside Claude Code
retitles the surrounding tmux window too.

This is the plain-text sibling of example 23. Example 23 proved the *markdown*
face of the spec's `ensure.text.section` primitive (FR-021) via
`ensure.file.md_section` with HTML-comment markers (`<!-- rw:section id="…" -->`).
A tmux config is **not** markdown — HTML comments are invalid there — so the
managed region must be delimited with tmux's own comment leader (`#`).

## Status: `pending`

This example is registered `stage: pending` in `examples/test-suite.yaml`: it is
an executable spec that **fails today and passes once implemented**. The harness
treats a pending example that starts passing as a failure ("promote to
implemented"), so promoting this is the definition of done for the feature.

What it needs from the engine (the gap this pins down):

- `ensure.text.section` accepting a **`selector.comment`** leader so the
  block-marker selector renders `# rw:section id="ID"` / `# rw:endsection id="ID"`
  instead of the markdown `<!-- … -->` form. Everything else — append-on-absent,
  idempotent update, byte-for-byte preservation outside the region — matches the
  behaviour example 23 already established.

## What this covers

- `ensure.text.section` with a `block_marker` selector on a **non-markdown** file
- `selector.comment: "#"` → `#`-delimited managed-region markers
- `on_exists: "update"` — re-apply rewrites the region in place, idempotently
- Preserving the user's hand-written tmux config (keybindings, options) across runs
- Input-driven rendering: `set_titles` and `strip_status_glyph` toggle template lines

## Why a dotfiles workspace?

The real target is `~/.tmux.conf`, a home dotfile outside any repo. Rather than
write blindly into `$HOME`, the example models the idiomatic setup: a **dotfiles
repo** whose `home/` tree mirrors `$HOME` and is stowed/symlinked there (GNU
Stow / chezmoi style). Weaver converges the *tracked* copy
(`home/.tmux.conf`), which keeps the change reviewable, diffable, and testable.

## Module contents

- `weaver.module.yaml` — inputs: `set_titles` (bool), `strip_status_glyph` (bool).
- `sections/tmux-claude-rename.conf.j2` — Tera template for the managed region
  body; the two booleans gate the `set-titles` line and the glyph-stripping
  format string.

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:

- `home/.tmux.conf` keeps its original header comment, `base-index`, `mouse`,
  `set-titles-string`, and the copy-mode keybinding — **unchanged**.
- A new region appended at EOF, between
  `# rw:section id="tmux-claude-rename"` and `# rw:endsection id="tmux-claude-rename"`,
  containing `set-titles on`, `automatic-rename on`, and the glyph-stripping
  `automatic-rename-format`.

The two `custom_assertions` in `test-suite.yaml` pin the load-bearing bits: the
glyph-stripping format fragment and the `#`-comment section marker.

## Applying it to a live tmux server

File convergence is Weaver's job; reloading the running server is an
operational step the consumer runs after `wvr apply` (or wires as a task):

```sh
tmux source-file ~/.tmux.conf
# Re-arm windows you'd previously renamed by hand (manual rename disables
# automatic-rename for that window):
tmux list-windows -a -F '#{session_name}:#{window_index}' \
  | xargs -I{} tmux setw -t {} automatic-rename on
```

## Drift behaviour (once implemented)

- Edit a line inside the `rw:section` region → `wvr plan` flags drift on that
  region only.
- Edit anything outside the markers (your keybindings, status options) → never
  flagged; it is not managed.
- Flip `strip_status_glyph` to `false` and re-apply → the region's format line is
  rewritten; the rest of the file is untouched.
