# 03 - Mixed Files and Templates

Demonstrates a module that contains both static files and templates processed together.

## What this covers

- `ensure.file.copy` and `ensure.file.from_template` combined (PRD §6)
- Static files copied as-is, templates rendered with variables

## Module contents

- `files/.gitignore` — static file, copied verbatim
- `files/LICENSE` — static file, copied verbatim
- `templates/README.md.j2` — template rendered with project variables
- `templates/Makefile.j2` — template rendered with project variables

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `app/.gitignore` and `app/LICENSE` copied verbatim
- `app/README.md` and `app/Makefile` rendered from templates
