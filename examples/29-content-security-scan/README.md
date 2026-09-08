# 29 - Content Security Scan

> **Status:** design-only / pending implementation. Demonstrates the desired shape of a new check `check.content_scan`. Same conventions as examples 18–25.

Demonstrates how to **block hostile module content** (Unicode bidi/invisible-character attacks, homoglyph deception) **before** templates are rendered or files are written. Adds a new `check.content_scan` to the existing `checks:` block on apps (see examples 20 and 21 for the existing check pattern).

## What this covers

- A new check: `check.content_scan` with fields:
  - `rules` (`list(string)`) — which rule kinds to run.
  - `targets` (glob list, repo-relative; default = all module-supplied content).
  - `fail_on` (`list(string)`, e.g. `[error]`).
  - `allow_codepoints` (`list(string)`, optional) — explicit whitelist of codepoints (e.g. `["U+202E"]`) for legitimate uses.
- Rule kinds (v1):
  - `unicode_bidi` — flags U+202A..U+202E and U+2066..U+2069 (the "Trojan Source" range).
  - `unicode_invisible` — flags zero-width chars (U+200B..U+200D, U+FEFF), tag chars (U+E0000..U+E007F), and other non-printing control chars outside whitelisted ranges.
  - `homoglyphs` — flags mixed-script identifiers in code-block fences and YAML frontmatter values (Latin/Cyrillic/Greek confusables).
- Pre-render: the check runs **after** module fetch and **before** any `ensure.*` writes, so a malicious skill never reaches the working tree.
- Hard fail by default for `error`-severity findings (same exit-code contract as today's `check.*` ensures, EC-003).

## Why

[microsoft/apm](https://github.com/microsoft/apm) advertises Unicode-exploit content scanning at install time as a default security feature. Weaver fetches Markdown skills/agents from arbitrary git sources too, and renders them into the user's target directory (and Claude reads them as instructions). A poisoned skill — for example a directive hidden inside a U+202E override — is the same threat model. This example encodes the desired countermeasure.

The Trojan Source class of attack is documented in [Boucher & Anderson 2021](https://trojansource.codes/).

## Module contents

This example does not need a remote module — it ships two **fixtures** locally:

- `fixtures/clean-skill.md` — a normal skill file. Apply succeeds.
- `fixtures/malicious-skill.md` — contains a U+202E (right-to-left override) inside a markdown bullet. Apply must abort.

Two apps point at the two fixtures so a single `wvr plan` shows both the success and the failure paths.

## How to run

```sh
cd before
wvr apply
```

## Expected result

- `app-clean/.claude/skills/example/SKILL.md` is rendered successfully.
- `app-malicious/.claude/skills/example/SKILL.md` is **not** written.
- `wvr apply` exits with code 3 (EC-003) and stderr matches `after/expected-stderr.txt`:

```
error: check.content_scan failed for app=app-malicious
  rule: unicode_bidi
  file: fixtures/malicious-skill.md
  line: 9, column: 30
  finding: codepoint U+202E (RIGHT-TO-LEFT OVERRIDE)
  hint: see https://trojansource.codes/ — remove the character or whitelist it explicitly via `allow_codepoints:`
```

## Composition with the existing checks model

The new check slots into the same `checks:` array used by examples 20 (`check.k8s.ingress_annotations`) and 21 (`check.terraform.ec2_tags`). Same exit codes, same `--fail-on` semantics, same `wvr plan` reporting. The difference is the **input domain**: existing checks operate on user-authored YAML/HCL; `check.content_scan` operates on module-supplied Markdown / templates.

## Why this is a check, not an ensure

A check is read-only and gates apply. A scan should never **modify** content (that would change the malicious bytes silently and confuse blame). Failing the apply with a clear pointer is the right behaviour — same reasoning that makes lints checks rather than ensures.

## References

- [Trojan Source: Invisible Vulnerabilities](https://trojansource.codes/) — the canonical attack class.
- [APM security model](https://github.com/microsoft/apm) — the upstream feature this mirrors.
- [Example 20](../20-check-k8s-ingress-annotations/) and [Example 21](../21-check-terraform-ec2-tags/) — existing `check.*` invocation pattern.
