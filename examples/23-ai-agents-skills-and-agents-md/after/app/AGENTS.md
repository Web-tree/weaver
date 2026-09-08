# acme-api

A small HTTP service that keeps the acme product catalogue in sync with downstream
inventory systems. Written in TypeScript on Node 20.

Contributors — human and AI — are expected to read this file before touching code.

## Manual Additions

> Hand-written notes that Weaver must preserve across runs.

- Before opening a PR, post a preview link in `#eng-reviews`.
- Never log customer email addresses, even at debug level.
- The staging database is rebuilt nightly; treat anything in `main` as fair game for the next rebuild.

## Skills

The following skills are installed under `.claude/skills/` and are available to any Claude Code session in this repository:

- **`plan-change`** — Outline the intended change before editing any code.
- **`write-tests`** — Draft a failing test that expresses the desired new behaviour.
- **`review-diff`** — Critique the staged diff before it is committed.

### Invocation order

When handling a feature request, non-trivial bug fix, or refactor in acme-api, invoke these skills in the following order:

1. `plan-change`
2. `write-tests`
3. `review-diff`

Stop between steps and wait for user confirmation — each skill's job is to produce a short artefact (plan, failing test, review) that gates the next one. Skip steps only when the task is clearly small enough that a human would skip them too.

<!-- rw:section id="recent-changes" -->
- Switched request validation from ad-hoc checks to zod schemas.
- Added a nightly integration job against the staging database.
<!-- rw:endsection id="recent-changes" -->
