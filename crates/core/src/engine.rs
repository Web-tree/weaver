use crate::template::TemplateEngine;
use weaver_ops::fs::ensure_dir;
use std::path::Path;

pub struct Engine;

impl Engine {
    pub fn ensure_folder_exists(path: &Path) -> anyhow::Result<()> {
        ensure_dir(path)
    }

    pub fn ensure_file_from_template(
        template_engine: &TemplateEngine,
        template_str: &str,
        context: &tera::Context,
        dest: &Path,
    ) -> anyhow::Result<()> {
        let content = template_engine.render(template_str, context)?;
        if let Some(parent) = dest.parent() {
            ensure_dir(parent)?;
        }
        std::fs::write(dest, content)?;
        Ok(())
    }

    pub fn ensure_file_copy(src: &Path, dest: &Path) -> anyhow::Result<()> {
        weaver_ops::fs::copy_file(src, dest)
    }

    pub fn ensure_task_wrapper(path: &Path, command: &str) -> anyhow::Result<()> {
        let content = format!("#!/bin/sh\nexec {}\n", command);
        if let Some(parent) = path.parent() {
            ensure_dir(parent)?;
        }
        std::fs::write(path, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    }
}
