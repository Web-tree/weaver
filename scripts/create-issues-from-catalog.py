#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Create GitHub issues from a markdown catalog.

Catalog format: each issue starts with an HTML comment marker

    <!-- issue key=<key> repo=<owner/repo> labels=<a,b,c> -->
    # <title>
    <body ...>

Bodies may reference other issues as `{{key}}`; after all issues are created the
placeholders are rewritten to `owner/repo#N` (or `#N` for the same repo) and the
bodies are updated in place.

Usage:
    scripts/create-issues-from-catalog.py <catalog.md> [--dry-run] [--only key1,key2]
                                          [--map map.json]

`--map` persists key -> {repo, number, url} so a re-run only creates missing issues.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

MARKER = re.compile(r"^<!--\s*issue\s+(?P<attrs>[^>]*?)\s*-->\s*$", re.M)
ATTR = re.compile(r"(\w+)=([^\s]+)")
PLACEHOLDER = re.compile(r"\{\{([a-z0-9-]+)\}\}")


def parse_catalog(text: str) -> list[dict]:
    issues = []
    markers = list(MARKER.finditer(text))
    for i, m in enumerate(markers):
        attrs = dict(ATTR.findall(m.group("attrs")))
        start = m.end()
        end = markers[i + 1].start() if i + 1 < len(markers) else len(text)
        chunk = text[start:end].strip()
        chunk = re.sub(r"\n---\s*$", "", chunk).strip()
        title_line, _, body = chunk.partition("\n")
        if not title_line.startswith("# "):
            sys.exit(f"issue {attrs.get('key')}: first line must be a '# title' heading")
        issues.append(
            {
                "key": attrs["key"],
                "repo": attrs["repo"],
                "labels": [l for l in attrs.get("labels", "").split(",") if l],
                "title": title_line[2:].strip(),
                "body": body.strip() + "\n",
            }
        )
    return issues


def gh(*args: str, input: str | None = None) -> str:
    r = subprocess.run(["gh", *args], input=input, text=True, capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(f"gh {' '.join(args)} failed:\n{r.stderr}")
    return r.stdout.strip()


def ensure_labels(repo: str, labels: set[str], dry_run: bool) -> None:
    existing = {l["name"] for l in json.loads(gh("label", "list", "--repo", repo, "--limit", "200", "--json", "name"))}
    colors = {"epic": "5319E7", "plugins": "0E8A16", "engine": "1D76DB", "modules": "FBCA04"}
    for label in sorted(labels - existing):
        print(f"  + label {repo} {label}")
        if not dry_run:
            gh("label", "create", label, "--repo", repo, "--color", colors.get(label, "EDEDED"))


def resolve(body: str, current_repo: str, mapping: dict) -> str:
    def sub(m: re.Match) -> str:
        key = m.group(1)
        if key not in mapping:
            return m.group(0)
        ref = mapping[key]
        return f"#{ref['number']}" if ref["repo"] == current_repo else f"{ref['repo']}#{ref['number']}"

    return PLACEHOLDER.sub(sub, body)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("catalog", type=Path)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--only", default="")
    ap.add_argument("--map", type=Path, default=Path("issue-map.json"))
    args = ap.parse_args()

    issues = parse_catalog(args.catalog.read_text())
    only = {k for k in args.only.split(",") if k}
    if only:
        issues = [i for i in issues if i["key"] in only]
    mapping: dict = json.loads(args.map.read_text()) if args.map.exists() else {}

    by_repo: dict[str, set[str]] = {}
    for i in issues:
        by_repo.setdefault(i["repo"], set()).update(i["labels"])
    for repo, labels in by_repo.items():
        ensure_labels(repo, labels, args.dry_run)

    # Pass 1: create (placeholders left as-is).
    for i in issues:
        if i["key"] in mapping:
            print(f"  = {i['key']} already {mapping[i['key']]['url']}")
            continue
        print(f"  + {i['repo']}: {i['title']}")
        if args.dry_run:
            continue
        url = gh(
            "issue", "create", "--repo", i["repo"], "--title", i["title"],
            "--body-file", "-", *sum((["--label", l] for l in i["labels"]), []),
            input=i["body"],
        )
        number = int(url.rstrip("/").rsplit("/", 1)[1])
        mapping[i["key"]] = {"repo": i["repo"], "number": number, "url": url}
        args.map.write_text(json.dumps(mapping, indent=2) + "\n")

    # Pass 2: rewrite placeholders.
    for i in issues:
        if args.dry_run or i["key"] not in mapping:
            continue
        resolved = resolve(i["body"], i["repo"], mapping)
        if resolved != i["body"]:
            ref = mapping[i["key"]]
            print(f"  ~ {i['key']} -> #{ref['number']} (links resolved)")
            gh("issue", "edit", str(ref["number"]), "--repo", ref["repo"], "--body-file", "-", input=resolved)

    unresolved = sorted({k for i in issues for k in PLACEHOLDER.findall(i["body"]) if k not in mapping})
    if unresolved:
        print(f"unresolved placeholders (not in this run): {', '.join(unresolved)}")


if __name__ == "__main__":
    main()
