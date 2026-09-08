# 10 - Template Conditionals

Demonstrates advanced Tera template features: conditionals, loops, and filters.

## What this covers

- Conditionals and loops in templates (PRD §5.4)
- `{% if %}` / `{% else %}` blocks
- `{% for %}` loops over lists
- Boolean and list input types

## Module contents

- `templates/docker-compose.yaml.j2` — template with conditional services and loop-generated ports

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `app/docker-compose.yaml` rendered with database service (because `enable_db: true`)
- Port mappings generated from the `ports` list
