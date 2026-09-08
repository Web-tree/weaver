# 02 - Template Rendering

Demonstrates template rendering with variable substitution using Tera (Jinja2-compatible).

## What this covers

- `ensure.file.from_template` (PRD §6)
- Variable substitution in `.j2` templates (PRD §5.3, §5.4)
- Input values provided via `weaver.yaml`

## Module contents

- `templates/config.yaml.j2` — template using `{{ project_name }}` and `{{ port }}`

## How to run

```sh
cd before
wvr apply
```

## Expected result

After apply, `before/` should match `after/`:
- `app/config.yaml` rendered with `project_name: "my-service"` and `port: 8080`
- The `.j2` extension is stripped from the output filename
