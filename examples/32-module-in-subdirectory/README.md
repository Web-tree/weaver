# 32 - Module in a Subdirectory

Demonstrates `modules[].path`: one git repository (`standards/`) ships two
modules, `modules/a` and `modules/b`, and each app consumes one of them.

## What this covers

- `path:` on a `modules:` entry selects a subdirectory of the module's git
  checkout: weaver reads `weaver.module.yaml`, `files/` and `templates/` from
  `<cache>/<commit>/<path>` instead of the repository root.
- Two `modules:` entries can share one `source` and `ref`; the repository is
  cloned once per commit and each entry points into it.
- `path` must be relative, stay inside the repository, and name a directory
  that holds a module. Otherwise the error lists the module directories the
  repository does contain.
- `weaver.lock` records `path` next to the pinned commit.
- `wvr module add ../standards --ref v1 --path modules/a` writes the same entry.

## Module contents

- `standards/modules/a/templates/README.md.j2`: README rendered with `service_name`
- `standards/modules/b/files/.editorconfig`: static editor config
- `standards/README.md`: repository content that is not part of either module

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `web/README.md` rendered from `modules/a`
- `api/.editorconfig` copied from `modules/b`
- Nothing from the repository root (such as `standards/README.md`) reaches either app
