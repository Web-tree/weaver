use std::collections::HashMap;
use tera::{Context, Tera};

pub struct TemplateEngine {
    _tera: Tera,
}

impl TemplateEngine {
    pub fn new() -> anyhow::Result<Self> {
        let tera = Tera::default();
        Ok(Self { _tera: tera })
    }

    pub fn render(&self, template_str: &str, context: &Context) -> anyhow::Result<String> {
        let cleaned = trim_block_lines(template_str);
        Ok(Tera::one_off(&cleaned, context, false)?)
    }
}

/// Approximate Jinja2's `trim_blocks=True, lstrip_blocks=True` behaviour.
///
/// Lines whose visible content is only Tera block/comment tags (`{% ... %}`,
/// `{# ... #}`) have their leading indentation and trailing newline stripped
/// so the tag line vanishes from the output while the tag itself is still
/// processed. Variable tags (`{{ ... }}`) and lines mixing tags with text are
/// left untouched.
fn trim_block_lines(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut remaining = src;
    while !remaining.is_empty() {
        let (line, rest) = match remaining.find('\n') {
            Some(idx) => (&remaining[..idx], &remaining[idx + 1..]),
            None => (remaining, ""),
        };
        let had_newline = remaining.len() != line.len();

        if is_block_only_line(line) {
            out.push_str(line.trim_start_matches([' ', '\t']));
            // drop the newline entirely
        } else {
            out.push_str(line);
            if had_newline {
                out.push('\n');
            }
        }
        remaining = rest;
    }
    out
}

/// True when a line contains only whitespace and Tera block/comment tags.
fn is_block_only_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let mut rest = trimmed;
    let mut saw_block = false;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("{%")
            && let Some(end) = after.find("%}")
        {
            rest = after[end + 2..].trim_start();
            saw_block = true;
            continue;
        }
        if let Some(after) = rest.strip_prefix("{#")
            && let Some(end) = after.find("#}")
        {
            rest = after[end + 2..].trim_start();
            saw_block = true;
            continue;
        }
        return false;
    }
    saw_block
}

/// Build a Tera context from app inputs. Supports scalars, lists, and maps so
/// templates can iterate with `{% for %}` and branch with `{% if %}`.
pub fn build_context(inputs: &HashMap<String, serde_yml::Value>) -> anyhow::Result<Context> {
    let mut ctx = Context::new();
    for (key, value) in inputs {
        let json = yaml_to_json(value)
            .map_err(|e| anyhow::anyhow!("input '{key}': cannot serialize for template: {e}"))?;
        ctx.insert(key, &json);
    }
    Ok(ctx)
}

fn yaml_to_json(value: &serde_yml::Value) -> Result<serde_json::Value, String> {
    use serde_json::Value as J;
    use serde_yml::Value as Y;
    Ok(match value {
        Y::Null => J::Null,
        Y::Bool(b) => J::Bool(*b),
        Y::Number(n) => {
            if let Some(i) = n.as_i64() {
                J::Number(i.into())
            } else if let Some(u) = n.as_u64() {
                J::Number(u.into())
            } else {
                serde_json::Number::from_f64(n.as_f64()).map(J::Number).unwrap_or(J::Null)
            }
        }
        Y::String(s) => J::String(s.clone()),
        Y::Sequence(seq) => {
            let mut out = Vec::with_capacity(seq.len());
            for item in seq {
                out.push(yaml_to_json(item)?);
            }
            J::Array(out)
        }
        Y::Mapping(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k.clone(), yaml_to_json(v)?);
            }
            J::Object(out)
        }
        Y::Tagged(tagged) => yaml_to_json(tagged.value())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(tpl: &str, ctx: &Context) -> String {
        TemplateEngine::new().unwrap().render(tpl, ctx).unwrap()
    }

    #[test]
    fn build_context_scalars_render() {
        let mut inputs = HashMap::new();
        inputs.insert("name".into(), serde_yml::Value::String("api".into()));
        inputs.insert("count".into(), serde_yml::Value::Number(3.into()));
        inputs.insert("debug".into(), serde_yml::Value::Bool(true));

        let ctx = build_context(&inputs).unwrap();
        let out = render("{{ name }} {{ count }} {{ debug }}", &ctx);
        assert_eq!(out, "api 3 true");
    }

    #[test]
    fn build_context_list_iterates_with_for() {
        let mut inputs = HashMap::new();
        inputs.insert(
            "ports".into(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::String("3000".into()),
                serde_yml::Value::String("8080".into()),
            ]),
        );

        let ctx = build_context(&inputs).unwrap();
        let out = render(
            "{% for p in ports %}{{ p }}{% if not loop.last %},{% endif %}{% endfor %}",
            &ctx,
        );
        assert_eq!(out, "3000,8080");
    }

    #[test]
    fn build_context_bool_branches_with_if() {
        let mut inputs = HashMap::new();
        inputs.insert("enable_db".into(), serde_yml::Value::Bool(true));

        let ctx = build_context(&inputs).unwrap();
        let out = render("{% if enable_db %}db{% else %}no{% endif %}", &ctx);
        assert_eq!(out, "db");

        let mut inputs2 = HashMap::new();
        inputs2.insert("enable_db".into(), serde_yml::Value::Bool(false));
        let ctx2 = build_context(&inputs2).unwrap();
        let out2 = render("{% if enable_db %}db{% else %}no{% endif %}", &ctx2);
        assert_eq!(out2, "no");
    }

    #[test]
    fn build_context_mapping_accesses_fields() {
        let mut map = serde_yml::Mapping::new();
        map.insert("host", serde_yml::Value::String("localhost".into()));
        let mut inputs = HashMap::new();
        inputs.insert("db".into(), serde_yml::Value::Mapping(map));

        let ctx = build_context(&inputs).unwrap();
        let out = render("{{ db.host }}", &ctx);
        assert_eq!(out, "localhost");
    }

    #[test]
    fn trim_block_lines_strips_tag_only_lines() {
        let src = "before\n    {% if x %}\ncontent\n  {% endif %}\nafter\n";
        let want = "before\n{% if x %}content\n{% endif %}after\n";
        assert_eq!(trim_block_lines(src), want);
    }

    #[test]
    fn trim_block_lines_keeps_lines_with_text() {
        let src = "  foo {% if x %} bar {% endif %} baz\n";
        assert_eq!(trim_block_lines(src), src);
    }

    #[test]
    fn trim_block_lines_keeps_variable_only_lines() {
        let src = "  {{ name }}\n";
        assert_eq!(trim_block_lines(src), src);
    }

    #[test]
    fn trim_block_lines_strips_comment_only_lines() {
        let src = "a\n  {# c #}\nb\n";
        assert_eq!(trim_block_lines(src), "a\n{# c #}b\n");
    }

    #[test]
    fn trim_block_lines_handles_multiple_tags_on_one_line() {
        let src = "  {% if a %}{% if b %}\nbody\n{% endif %}{% endif %}\n";
        assert_eq!(
            trim_block_lines(src),
            "{% if a %}{% if b %}body\n{% endif %}{% endif %}"
        );
    }

    #[test]
    fn trim_block_lines_handles_no_trailing_newline() {
        assert_eq!(trim_block_lines("  {% if x %}"), "{% if x %}");
    }

    #[test]
    fn renders_docker_compose_style_template_with_loop_and_if() {
        let template = "version: \"3.8\"\n\nservices:\n  app:\n    image: {{ image_name }}:latest\n    ports:\n{% for port in ports %}\n      - \"{{ port }}:{{ port }}\"\n{% endfor %}\n{% if enable_db %}\n  database:\n    image: postgres:16\n    environment:\n      POSTGRES_DB: {{ db_name }}\n      POSTGRES_USER: admin\n    ports:\n      - \"5432:5432\"\n{% endif %}\n";

        let mut inputs = HashMap::new();
        inputs.insert("image_name".into(), serde_yml::Value::String("myapp".into()));
        inputs.insert("enable_db".into(), serde_yml::Value::Bool(true));
        inputs.insert("db_name".into(), serde_yml::Value::String("appdb".into()));
        inputs.insert(
            "ports".into(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::String("3000".into()),
                serde_yml::Value::String("8080".into()),
            ]),
        );

        let ctx = build_context(&inputs).unwrap();
        let out = render(template, &ctx);
        let expected = "version: \"3.8\"\n\nservices:\n  app:\n    image: myapp:latest\n    ports:\n      - \"3000:3000\"\n      - \"8080:8080\"\n  database:\n    image: postgres:16\n    environment:\n      POSTGRES_DB: appdb\n      POSTGRES_USER: admin\n    ports:\n      - \"5432:5432\"\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn build_context_list_of_numbers() {
        let mut inputs = HashMap::new();
        inputs.insert(
            "ids".into(),
            serde_yml::Value::Sequence(vec![
                serde_yml::Value::Number(1.into()),
                serde_yml::Value::Number(2.into()),
            ]),
        );

        let ctx = build_context(&inputs).unwrap();
        let out = render("{{ ids | length }}", &ctx);
        assert_eq!(out, "2");
    }
}
