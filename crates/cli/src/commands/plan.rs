use clap::Args;
use std::path::PathBuf;
use tracing::info;

#[derive(Args)]
pub struct PlanArgs {
    /// Exit with code 2 if ANY change is planned — creates, updates, or
    /// drift (Terraform convention), not drift-only. Exit 0 means the
    /// workspace is already converged; exit 1 means an error occurred.
    #[arg(long)]
    pub detailed_exitcode: bool,

    /// Save the plan to a file
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub async fn run(args: PlanArgs) -> anyhow::Result<crate::exit::ExitCode> {
    info!("Running plan...");

    let apply_args = crate::commands::apply::ApplyArgs {
        auto_approve: false,
        strategy: "stop".to_string(),
        plan: None,
        offline: false,
    };

    let change_count = crate::commands::apply::execute(apply_args, true).await?;

    if args.detailed_exitcode && change_count > 0 {
        return Ok(crate::exit::ExitCode::Violations);
    }
    Ok(crate::exit::ExitCode::Success)
}
