# 05 - Nested Folders

Demonstrates that deeply nested directory structures in modules are preserved
in the output.

## What this covers

- Deep directory structure preservation (PRD §3)
- Automatic parent directory creation
- Both static files and templates in nested paths

## Module contents

- `files/src/main/resources/app.properties` — deeply nested static file
- `templates/src/main/resources/config.yaml.j2` — deeply nested template

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `app/src/main/resources/app.properties` copied verbatim
- `app/src/main/resources/config.yaml` rendered from template
- All intermediate directories created automatically
