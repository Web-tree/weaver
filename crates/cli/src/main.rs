mod commands;
mod prompts;

use clap::{CommandFactory, Parser};
use commands::{apply, describe, init, list, plan};
use weaver_core::{LoggingOptions, setup_tracing_with_options};

#[derive(Parser)]
#[command(name = "wvr")]
#[command(about = "Declarative directory configuration")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Enable debug logging
    #[arg(long, global = true)]
    verbose: bool,

    /// Suppress all non-error output
    #[arg(long, global = true)]
    quiet: bool,

    /// Output logs/result in JSON format
    #[arg(long, global = true)]
    json: bool,
}

#[derive(clap::Subcommand)]
enum Commands {
    Init(init::InitArgs),
    Plan(plan::PlanArgs),
    Apply(apply::ApplyArgs),
    List(list::ListArgs),
    /// Inspect an application's configuration
    Describe(commands::describe::DescribeArgs),
    /// Manage modules
    Module(commands::module::ModuleArgs),
    /// Run configured checks
    Check(commands::check::CheckArgs),
    /// Manage plugins
    Plugins(commands::plugins::PluginsArgs),
    Run(crate::commands::run::RunArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Setup tracing with CLI options
    let logging_opts = LoggingOptions {
        json: cli.json,
        verbose: cli.verbose,
        quiet: cli.quiet,
    };
    setup_tracing_with_options(&logging_opts)?;

    // Handle --no-color: set env var for downstream tools
    if cli.no_color {
        // SAFETY: Single-threaded initialization before any child threads are spawned.
        unsafe { std::env::set_var("NO_COLOR", "1") };
    }

    match cli.command {
        Some(Commands::Init(args)) => {
            init::run(args)?;
        }
        Some(Commands::Plan(args)) => {
            plan::run(args).await?;
        }
        Some(Commands::Apply(args)) => {
            apply::run(args).await?;
        }
        Some(Commands::List(args)) => {
            list::run(args).await?;
        }
        Some(Commands::Describe(args)) => {
            describe::run(args).await?;
        }
        Some(Commands::Run(args)) => {
            crate::commands::run::run(args).await?;
        }
        Some(Commands::Module(args)) => {
            commands::module::execute(args)?;
        }
        Some(Commands::Check(args)) => {
            commands::check::execute(args)?;
        }
        Some(Commands::Plugins(args)) => {
            commands::plugins::execute(args).await?;
        }
        None => {
            Cli::command().print_help()?;
        }
    }

    Ok(())
}
