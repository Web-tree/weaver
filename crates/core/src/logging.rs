use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Logging options for configuring the tracing subscriber.
#[derive(Default, Clone, Debug)]
pub struct LoggingOptions {
    /// If true, output logs in JSON format (for CI/CD pipelines).
    pub json: bool,
    /// If true, enable verbose/debug level logging.
    pub verbose: bool,
    /// If true, suppress all non-error output.
    pub quiet: bool,
}

/// Sets up the tracing subscriber with the given options.
///
/// # Arguments
/// * `options` - Logging configuration options.
///
/// # Returns
/// * `anyhow::Result<()>` - Ok if successful, Err otherwise.
pub fn setup_tracing() -> anyhow::Result<()> {
    setup_tracing_with_options(&LoggingOptions::default())
}

/// Sets up the tracing subscriber with explicit options.
///
/// Logs always go to **stderr**, never stdout. This is a deliberate,
/// permanent decision, not incidental to any one command: stdout is the
/// program's actual output (a table, a plan, a `--json` report), and any
/// consumer piping that output to another program (`| jq`, `| tee`, ...)
/// must never have it interleaved with, or corrupted by, log lines. Routing
/// logs to stderr keeps that true regardless of which command runs or what
/// tracing calls it or the library code it calls make in the future.
pub fn setup_tracing_with_options(options: &LoggingOptions) -> anyhow::Result<()> {
    // Determine log level filter based on options
    let filter = if options.quiet {
        EnvFilter::new("error")
    } else if options.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    if options.json {
        // JSON format for CI/CD pipelines
        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .json()
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_file(true)
                    .with_line_number(true)
                    .with_writer(std::io::stderr),
            )
            .init();
    } else {
        // Human-readable format for interactive use
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(false).compact().with_writer(std::io::stderr))
            .init();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_options_default() {
        let opts = LoggingOptions::default();
        assert!(!opts.json);
        assert!(!opts.verbose);
        assert!(!opts.quiet);
    }
}
