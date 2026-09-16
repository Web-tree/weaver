mod commands;
mod exit;
mod prompts;
mod update_notice;

use clap::{CommandFactory, Parser};
use commands::{apply, describe, init, list, plan};
use exit::ExitCode;
use weaver_core::{LoggingOptions, setup_tracing_with_options};

#[derive(Parser)]
#[command(name = "wvr")]
#[command(version)]
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
    /// Update wvr to the latest release
    SelfUpdate(commands::self_update::SelfUpdateArgs),
}

#[tokio::main]
async fn main() {
    let code = run().await;

    // `std::process::exit` skips destructors and any buffering the normal
    // return-from-main path would flush for us, so do it ourselves first.
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    std::process::exit(code.code());
}

/// Runs the CLI end to end and settles on the single [`ExitCode`] `main`
/// exits with (R24). This is the one place `dispatch`'s result is turned
/// into a process exit code, via `exit::exit_code_for_result`.
async fn run() -> ExitCode {
    let cli = Cli::parse();

    // Setup tracing with CLI options
    let logging_opts = LoggingOptions {
        json: cli.json,
        verbose: cli.verbose,
        quiet: cli.quiet,
    };
    if let Err(err) = setup_tracing_with_options(&logging_opts) {
        exit::print_error(&err);
        return exit::exit_code_for_result(&Err(err));
    }

    // Handle --no-color: set env var for downstream tools
    if cli.no_color {
        // SAFETY: Single-threaded initialization before any child threads are spawned.
        unsafe { std::env::set_var("NO_COLOR", "1") };
    }

    // `self-update` owns the release lifecycle end to end; every other command
    // gets a release check running concurrently with it.
    let update_watch = match cli.command {
        Some(Commands::SelfUpdate(_)) => None,
        _ => Some(update_notice::start(!cli.quiet && !cli.json)),
    };

    let result = dispatch(cli.command, cli.json).await;

    if let Some(watch) = update_watch {
        watch.finish().await;
    }

    if let Err(err) = &result {
        exit::print_error(err);
    }
    exit::exit_code_for_result(&result)
}

async fn dispatch(command: Option<Commands>, json: bool) -> anyhow::Result<ExitCode> {
    let code = match command {
        Some(Commands::Init(args)) => {
            init::run(args)?;
            ExitCode::Success
        }
        Some(Commands::Plan(args)) => plan::run(args).await?,
        Some(Commands::Apply(args)) => {
            apply::run(args).await?;
            ExitCode::Success
        }
        Some(Commands::List(args)) => {
            list::run(args).await?;
            ExitCode::Success
        }
        Some(Commands::Describe(args)) => {
            describe::run(args).await?;
            ExitCode::Success
        }
        Some(Commands::Run(args)) => {
            crate::commands::run::run(args).await?;
            ExitCode::Success
        }
        Some(Commands::Module(args)) => {
            commands::module::execute(args)?;
            ExitCode::Success
        }
        Some(Commands::Check(args)) => commands::check::execute(args, json).await?,
        Some(Commands::Plugins(args)) => {
            commands::plugins::execute(args).await?;
            ExitCode::Success
        }
        Some(Commands::SelfUpdate(args)) => {
            commands::self_update::run(args).await?;
            ExitCode::Success
        }
        None => {
            Cli::command().print_help()?;
            ExitCode::Success
        }
    };

    Ok(code)
}
