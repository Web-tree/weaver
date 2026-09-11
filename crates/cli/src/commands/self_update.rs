use std::io::IsTerminal;

use clap::Args;
use console::style;
use weaver_core::paths;
use weaver_core::update::check::UpdateCheck;
use weaver_core::update::release::BUILD_TARGET;
use weaver_core::update::{Status, Updater};

/// Update `wvr` to the latest published release
#[derive(Args)]
pub struct SelfUpdateArgs {
    /// Report whether a newer release exists without installing it
    #[arg(long)]
    check: bool,

    /// Install a specific release tag (for example v0.2.0)
    #[arg(long, value_name = "TAG")]
    tag: Option<String>,

    /// Install even when the target version is not newer
    #[arg(long)]
    force: bool,

    /// Do not ask for confirmation
    #[arg(long, short = 'y')]
    yes: bool,
}

pub async fn run(args: SelfUpdateArgs) -> anyhow::Result<()> {
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let updater = Updater::from_env(current.clone())?;

    let release = match &args.tag {
        Some(tag) => updater.release_by_tag(tag).await?,
        None => updater.latest_release().await?,
    };
    let target_version = release.version()?;
    let status = updater.status_for(&release)?;

    if args.check {
        report(&status, &release.html_url);
        return Ok(());
    }

    let up_to_date = target_version <= current;
    if up_to_date && !args.force {
        println!("wvr {current} is up to date (latest release: {target_version})");
        return Ok(());
    }

    let install_path = updater.install_path()?;
    if !args.yes && !confirm(&current, &target_version, &install_path)? {
        println!("Aborted");
        return Ok(());
    }

    println!(
        "Downloading wvr {target_version} for {BUILD_TARGET} from {}",
        updater.repo()
    );

    let installed = updater.install(&release).await?;

    // Keep the cached check in sync so the background notice stops firing.
    let state = UpdateCheck::new(
        &installed,
        &release.html_url,
        weaver_core::update::check::now_unix(),
    );
    if let Err(e) = state.save(&paths::update_check_file()) {
        tracing::debug!(error = %e, "could not refresh update check state");
    }

    if installed == current {
        println!(
            "{} wvr {installed} ({})",
            style("Reinstalled").green().bold(),
            install_path.display()
        );
    } else {
        println!(
            "{} wvr {current} -> {installed} ({})",
            style("Updated").green().bold(),
            install_path.display()
        );
    }

    Ok(())
}

fn report(status: &Status, url: &str) {
    match status {
        Status::UpToDate { current } => {
            println!("wvr {current} is up to date");
        }
        Status::Available {
            current,
            latest,
            url: release_url,
        } => {
            let link = if release_url.is_empty() {
                url
            } else {
                release_url
            };
            println!(
                "{} wvr {current} -> {latest}",
                style("Update available:").yellow().bold()
            );
            if !link.is_empty() {
                println!("  {link}");
            }
            println!("  Run `wvr self-update` to install it");
        }
    }
}

fn confirm(
    current: &semver::Version,
    target: &semver::Version,
    install_path: &std::path::Path,
) -> anyhow::Result<bool> {
    // Non-interactive runs must not hang waiting for an answer.
    if !std::io::stdin().is_terminal() {
        return Ok(true);
    }

    let prompt = format!(
        "Replace {} (wvr {current}) with wvr {target}?",
        install_path.display()
    );

    Ok(
        dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(prompt)
            .default(true)
            .interact()?,
    )
}
