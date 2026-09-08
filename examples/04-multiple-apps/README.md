# 04 - Multiple Apps

Demonstrates two apps referencing the same module with different inputs,
producing different outputs in separate directories (monorepo pattern).

## What this covers

- Multiple apps from one module (PRD §4.2)
- Path scoping per app
- Different input values producing different rendered outputs

## Module contents

- `templates/config.yaml.j2` — template using `{{ app_name }}` and `{{ env }}`

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `apps/staging/config.yaml` rendered with `env: staging`
- `apps/production/config.yaml` rendered with `env: production`
