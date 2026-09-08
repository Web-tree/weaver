use crate::config::MdSelector;
use crate::ensure::{Ensure, EnsureContext, EnsurePlan};
use crate::template::TemplateEngine;

/// Insert or replace a marker-delimited region. `content` is the inner body
/// (no surrounding newlines). Everything outside the markers is preserved.
///
/// v1 limitations: targets LF markdown; does NOT skip headings inside fenced
/// code blocks (a matching heading inside ``` will be matched); the heading
/// `path`'s final element is matched at `depth` (parent hierarchy is not
/// enforced); CRLF input is normalized to LF on write.
pub(crate) fn upsert_block_marker(input: &str, id: &str, content: &str) -> String {
    let start = format!("<!-- rw:section id=\"{id}\" -->");
    let end = format!("<!-- rw:endsection id=\"{id}\" -->");
    let body = content.trim_matches('\n');
    let block = format!("{start}\n{body}\n{end}");

    if let (Some(s), Some(e)) = (input.find(&start), input.find(&end))
        && s < e
    {
        let e_end = e + end.len();
        let mut out = String::with_capacity(input.len());
        out.push_str(&input[..s]);
        out.push_str(&block);
        out.push_str(&input[e_end..]);
        return out;
    }

    // markers missing OR malformed (end before start) -> append a fresh valid
    // block, leaving any bad markers visibly in place rather than duplicating
    // content between them.
    let mut out = input.trim_end_matches('\n').to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&block);
    out.push('\n');
    out
}

fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if hashes > 0 && line.chars().nth(hashes) == Some(' ') {
        Some(hashes)
    } else {
        None
    }
}

/// Insert or replace the body under a heading. `path`'s last element is the
/// heading title rendered at `depth` (`#`*depth). The managed region runs from
/// the heading line to the line before the next heading of level <= depth (or
/// EOF). Content outside is preserved.
///
/// v1 limitations: targets LF markdown; does NOT skip headings inside fenced
/// code blocks (a matching heading inside ``` will be matched); the heading
/// `path`'s final element is matched at `depth` (parent hierarchy is not
/// enforced); CRLF input is normalized to LF on write.
pub(crate) fn upsert_heading(input: &str, path: &[String], depth: usize, content: &str) -> String {
    let title = path.last().map(|s| s.as_str()).unwrap_or("");
    let heading_line = format!("{} {}", "#".repeat(depth), title);
    let body = content.trim_matches('\n');

    let lines: Vec<&str> = input.lines().collect();
    let mut start_idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if heading_level(line) == Some(depth) && line.trim_end() == heading_line {
            start_idx = Some(i);
            break;
        }
    }

    let trailing_nl = input.ends_with('\n');

    if let Some(start) = start_idx {
        let mut end = lines.len();
        for (i, line) in lines.iter().enumerate().skip(start + 1) {
            if let Some(l) = heading_level(line)
                && l <= depth
            {
                end = i;
                break;
            }
        }
        let mut out: Vec<String> = Vec::new();
        out.extend(lines[..start].iter().map(|s| s.to_string()));
        out.push(heading_line.clone());
        out.push(String::new());
        out.push(body.to_string());
        if end < lines.len() {
            out.push(String::new());
            out.extend(lines[end..].iter().map(|s| s.to_string()));
        }
        let mut joined = out.join("\n");
        if trailing_nl {
            joined.push('\n');
        }
        return joined;
    }

    let mut out = input.trim_end_matches('\n').to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&heading_line);
    out.push_str("\n\n");
    out.push_str(body);
    out.push('\n');
    out
}

/// `ensure.file.md_section` — converge one managed region of a markdown file
/// (block-marker or heading selector). Content outside the region is preserved.
pub struct EnsureFileMdSection {
    pub file: String,
    pub selector: MdSelector,
    pub content: Option<String>,
    pub content_from_template: Option<String>,
}

impl EnsureFileMdSection {
    fn resolved_content(&self, ctx: &EnsureContext) -> anyhow::Result<String> {
        if let Some(rel) = &self.content_from_template {
            let p = ctx.module_path.join(rel);
            let src = std::fs::read_to_string(&p)
                .map_err(|e| anyhow::anyhow!("cannot read section template {}: {e}", p.display()))?;
            let engine = TemplateEngine::new()?;
            engine.render(&src, &ctx.tera_context)
        } else {
            Ok(self.content.clone().unwrap_or_default())
        }
    }
}

impl Ensure for EnsureFileMdSection {
    fn plan(&self, _ctx: &EnsureContext) -> anyhow::Result<EnsurePlan> {
        let what = match &self.selector {
            MdSelector::BlockMarker { id } => format!("block '{id}'"),
            MdSelector::Heading { path, .. } => format!("heading '{}'", path.join(" > ")),
        };
        Ok(EnsurePlan {
            description: format!("Ensure {what} section in {}", self.file),
            actions: vec![format!("converge section in {}", self.file)],
        })
    }

    fn execute(&self, ctx: &EnsureContext) -> anyhow::Result<()> {
        if ctx.dry_run {
            return Ok(());
        }
        let target = ctx.app_path.join(&self.file);
        let current = std::fs::read_to_string(&target)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", target.display()))?;
        let content = self.resolved_content(ctx)?;
        let updated = match &self.selector {
            MdSelector::BlockMarker { id } => upsert_block_marker(&current, id, &content),
            MdSelector::Heading { path, depth } => upsert_heading(&current, path, *depth, &content),
        };
        std::fs::write(&target, updated)?;
        Ok(())
    }
}

/// `ensure.file.exists` — create an empty file (and parent dirs) if absent.
/// Never truncates an existing file (idempotent, non-clobbering).
pub struct EnsureFileExists {
    /// Destination path, relative to the app root.
    pub dest: String,
}

impl Ensure for EnsureFileExists {
    fn plan(&self, ctx: &EnsureContext) -> anyhow::Result<EnsurePlan> {
        let target = ctx.app_path.join(&self.dest);
        let actions = if target.exists() {
            vec![]
        } else {
            vec![format!("create file {}", self.dest)]
        };
        Ok(EnsurePlan {
            description: format!("Ensure file '{}' exists", self.dest),
            actions,
        })
    }

    fn execute(&self, ctx: &EnsureContext) -> anyhow::Result<()> {
        if ctx.dry_run {
            return Ok(());
        }
        let target = ctx.app_path.join(&self.dest);
        if !target.exists() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, b"")?;
        }
        Ok(())
    }
}

/// `ensure.file.from_template` — render a module-relative Tera template into an
/// app-relative destination. weaver fully owns the destination file.
pub struct EnsureFileFromTemplate {
    /// Template path, relative to the module root.
    pub template: String,
    /// Destination path, relative to the app root.
    pub dest: String,
}

impl EnsureFileFromTemplate {
    fn render(&self, ctx: &EnsureContext) -> anyhow::Result<String> {
        let template_path = ctx.module_path.join(&self.template);
        let src = std::fs::read_to_string(&template_path).map_err(|e| {
            anyhow::anyhow!("cannot read template {}: {e}", template_path.display())
        })?;
        let engine = TemplateEngine::new()?;
        engine.render(&src, &ctx.tera_context)
    }
}

impl Ensure for EnsureFileFromTemplate {
    fn plan(&self, _ctx: &EnsureContext) -> anyhow::Result<EnsurePlan> {
        Ok(EnsurePlan {
            description: format!("Render '{}' -> '{}'", self.template, self.dest),
            actions: vec![format!("write {}", self.dest)],
        })
    }

    fn execute(&self, ctx: &EnsureContext) -> anyhow::Result<()> {
        if ctx.dry_run {
            return Ok(());
        }
        let rendered = self.render(ctx)?;
        let target = ctx.app_path.join(&self.dest);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, rendered)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn block_marker_appends_then_updates_idempotently() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("AGENTS.md");
        std::fs::write(&file, "# Title\n\nbody\n").unwrap();

        let out = super::upsert_block_marker(
            &std::fs::read_to_string(&file).unwrap(),
            "recent-changes",
            "- one\n- two",
        );
        std::fs::write(&file, &out).unwrap();
        let got = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            got,
            "# Title\n\nbody\n\n<!-- rw:section id=\"recent-changes\" -->\n- one\n- two\n<!-- rw:endsection id=\"recent-changes\" -->\n"
        );

        let out2 = super::upsert_block_marker(&got, "recent-changes", "- three");
        assert_eq!(
            out2,
            "# Title\n\nbody\n\n<!-- rw:section id=\"recent-changes\" -->\n- three\n<!-- rw:endsection id=\"recent-changes\" -->\n"
        );
        assert_eq!(super::upsert_block_marker(&out2, "recent-changes", "- three"), out2);
    }

    #[test]
    fn block_marker_swapped_markers_append_without_duplicating_content() {
        let input = "<!-- rw:endsection id=\"x\" -->\nstray\n<!-- rw:section id=\"x\" -->\n";
        let out = super::upsert_block_marker(input, "x", "new");
        assert_eq!(out.matches("stray").count(), 1, "content between swapped markers must not be duplicated");
        assert!(out.contains("<!-- rw:section id=\"x\" -->\nnew\n<!-- rw:endsection id=\"x\" -->"));
    }

    #[test]
    fn heading_appends_then_updates_preserving_other_sections() {
        let input = "# Title\n\n## Manual Additions\n\nkeep me\n";
        let out = super::upsert_heading(input, &["Skills".to_string()], 2, "body line");
        assert_eq!(
            out,
            "# Title\n\n## Manual Additions\n\nkeep me\n\n## Skills\n\nbody line\n"
        );
        let out2 = super::upsert_heading(&out, &["Skills".to_string()], 2, "new body");
        assert_eq!(
            out2,
            "# Title\n\n## Manual Additions\n\nkeep me\n\n## Skills\n\nnew body\n"
        );
    }

    #[test]
    fn heading_section_stops_at_next_same_or_higher_level_heading() {
        let input = "## Skills\n\nold\n\n## Other\n\nleave\n";
        let out = super::upsert_heading(input, &["Skills".to_string()], 2, "new");
        assert_eq!(out, "## Skills\n\nnew\n\n## Other\n\nleave\n");
    }

    fn ctx(app: PathBuf) -> EnsureContext {
        EnsureContext {
            app_path: app,
            dry_run: false,
            module_path: PathBuf::from("."),
            tera_context: tera::Context::new(),
        }
    }

    #[test]
    fn creates_missing_file_and_is_idempotent() {
        let dir = tempdir().unwrap();
        let e = EnsureFileExists { dest: "sub/new.txt".into() };
        e.execute(&ctx(dir.path().to_path_buf())).unwrap();
        let p = dir.path().join("sub/new.txt");
        assert!(p.exists());
        std::fs::write(&p, "user content").unwrap();
        e.execute(&ctx(dir.path().to_path_buf())).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "user content");
    }

    #[test]
    fn renders_module_template_into_app_dest() {
        let module = tempdir().unwrap();
        let app = tempdir().unwrap();
        std::fs::create_dir_all(module.path().join("templates")).unwrap();
        std::fs::write(
            module.path().join("templates/greeting.txt.j2"),
            "Hello {{ project_name }}\n",
        )
        .unwrap();

        let mut tc = tera::Context::new();
        tc.insert("project_name", "acme-api");
        let ctx = EnsureContext {
            app_path: app.path().to_path_buf(),
            dry_run: false,
            module_path: module.path().to_path_buf(),
            tera_context: tc,
        };

        let e = EnsureFileFromTemplate {
            template: "templates/greeting.txt.j2".into(),
            dest: "greeting.txt".into(),
        };
        e.execute(&ctx).unwrap();
        let out = std::fs::read_to_string(app.path().join("greeting.txt")).unwrap();
        assert_eq!(out, "Hello acme-api\n");
    }
}
