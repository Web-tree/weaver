//! Typed process exit codes for `wvr` (R24,
//! `docs/dev-standards-module-handoff.md`; `specs/004-generic-standardization-engine/spec.md`
//! FR-005).
//!
//! | Code | Meaning                                             |
//! |------|------------------------------------------------------|
//! | 0    | success / conformant                                  |
//! | 1    | user error (bad config, bad CLI args, ...)            |
//! | 2    | violations or pending changes (`plan --detailed-exitcode`, later `check`) |
//! | 3    | system error (I/O, git, network, environmental)       |
//! | 4    | plugin error (a WASM plugin failed to load, trap, or errored) |
//!
//! `main` drives the process exit through [`exit_code_for_result`], the single
//! site that maps a finished command's `anyhow::Result<ExitCode>` to the
//! number actually passed to `std::process::exit`. Commands that need a
//! precise, non-default code either return it directly (e.g. `plan` returns
//! `Ok(ExitCode::Violations)`), or -- when only an `Err` is available and no
//! existing error type identifies it reliably -- attach one explicitly via
//! [`ResultExt::with_exit_code`], recovered here through `downcast_ref`.
//!
//! Unclassified errors keep exiting `1`: only [`PluginError`](weaver_core::plugin::PluginError)
//! and `std::io::Error` (found anywhere in the error's cause chain) are
//! auto-classified, because those are the only existing error types precise
//! enough to move an error off the default without guessing.

use std::fmt;

/// A typed `wvr` process exit code (R24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Success / conformant.
    Success,
    /// User error: bad config, bad CLI args, something the user must fix.
    UserError,
    /// Violations or pending changes (`plan --detailed-exitcode`, and later
    /// `check` failures).
    Violations,
    /// System error: I/O, git, network -- anything environmental.
    SystemError,
    /// Plugin error: a WASM plugin failed to load, trap, or returned an
    /// error.
    Plugin,
}

impl ExitCode {
    /// The numeric code passed to `std::process::exit`.
    pub fn code(self) -> i32 {
        match self {
            ExitCode::Success => 0,
            ExitCode::UserError => 1,
            ExitCode::Violations => 2,
            ExitCode::SystemError => 3,
            ExitCode::Plugin => 4,
        }
    }
}

/// Internal marker attached to an `anyhow::Error` via
/// [`ResultExt::with_exit_code`] to carry an explicit [`ExitCode`] through
/// the error chain. Has no message of its own -- the error it wraps still
/// prints exactly as it would have unwrapped; see [`print_error`].
#[derive(Debug)]
struct ExitTag(ExitCode);

impl fmt::Display for ExitTag {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl std::error::Error for ExitTag {}

/// Attach an explicit [`ExitCode`] to a command's error result.
///
/// Use this only where no existing error type makes classification of the
/// failure reliable (see module docs) -- most `anyhow::bail!` sites should
/// stay untagged and keep exiting `1`. Every error type that *is* reliably
/// classifiable today (`PluginError`, `std::io::Error`) is already handled
/// by [`exit_code_for_error`]'s type-based checks, so this extension point
/// currently has no production call site -- it exists for the next command
/// that needs one, and is exercised directly by this module's tests.
#[allow(dead_code)]
pub trait ResultExt<T> {
    fn with_exit_code(self, code: ExitCode) -> anyhow::Result<T>;
}

impl<T> ResultExt<T> for anyhow::Result<T> {
    fn with_exit_code(self, code: ExitCode) -> anyhow::Result<T> {
        self.map_err(|err| err.context(ExitTag(code)))
    }
}

/// The single mapping site from a finished command's result to the process
/// [`ExitCode`]. `main` calls this exactly once, after the command has
/// already run to completion.
pub fn exit_code_for_result(result: &anyhow::Result<ExitCode>) -> ExitCode {
    match result {
        Ok(code) => *code,
        Err(err) => exit_code_for_error(err),
    }
}

/// Classify an error into an [`ExitCode`]. Order:
///
/// 1. An explicit [`ExitTag`] attached via [`ResultExt::with_exit_code`] wins.
/// 2. A [`weaver_core::plugin::PluginError`] anywhere in the cause chain -> `Plugin`.
/// 3. A `std::io::Error` anywhere in the cause chain -> `SystemError`.
/// 4. Anything else -> `UserError`. **Never widen this default** -- most
///    `anyhow::bail!` sites are user errors and already rely on it.
fn exit_code_for_error(err: &anyhow::Error) -> ExitCode {
    // `anyhow::Error::downcast_ref` has a special carve-out for the type
    // passed to `.context()`, recovering it directly even though the actual
    // stored error is an internal `ContextError<ExitTag, _>` wrapper. This
    // only works on the outermost `anyhow::Error`, not on `.chain()`
    // entries, which is why it's handled separately from the checks below.
    if let Some(tag) = err.downcast_ref::<ExitTag>() {
        return tag.0;
    }
    if err
        .chain()
        .any(|cause| cause.downcast_ref::<weaver_core::plugin::PluginError>().is_some())
    {
        return ExitCode::Plugin;
    }
    if err.chain().any(|cause| cause.downcast_ref::<std::io::Error>().is_some()) {
        return ExitCode::SystemError;
    }
    ExitCode::UserError
}

/// Print an error the way `main` used to rely on anyhow's `Termination` impl
/// to do (`eprintln!("Error: {err:?}")`), preserving the exact chain/
/// backtrace format, minus the invisible [`ExitTag`] marker layer.
pub fn print_error(err: &anyhow::Error) {
    let mut chain = err.chain();
    if err.downcast_ref::<ExitTag>().is_some() {
        // Discard the tag layer itself; it carries no message.
        chain.next();
    }
    let Some(top) = chain.next() else {
        return;
    };
    eprint!("Error: {top}");

    let rest: Vec<_> = chain.collect();
    if rest.is_empty() {
        eprintln!();
    } else if rest.len() == 1 {
        eprintln!("\n\nCaused by:\n    {}", rest[0]);
    } else {
        eprintln!("\n\nCaused by:");
        for (i, cause) in rest.iter().enumerate() {
            eprintln!("    {i}: {cause}");
        }
    }

    if matches!(std::env::var("RUST_BACKTRACE").as_deref(), Ok(v) if v != "0") {
        eprintln!("\nStack backtrace:\n{}", err.backtrace());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_the_r24_table() {
        assert_eq!(ExitCode::Success.code(), 0);
        assert_eq!(ExitCode::UserError.code(), 1);
        assert_eq!(ExitCode::Violations.code(), 2);
        assert_eq!(ExitCode::SystemError.code(), 3);
        assert_eq!(ExitCode::Plugin.code(), 4);
    }

    #[test]
    fn ok_result_maps_to_its_own_code() {
        let result: anyhow::Result<ExitCode> = Ok(ExitCode::Violations);
        assert_eq!(exit_code_for_result(&result), ExitCode::Violations);
    }

    #[test]
    fn unrecognized_error_maps_to_user_error() {
        let err = anyhow::anyhow!("something went wrong");
        assert_eq!(exit_code_for_error(&err), ExitCode::UserError);

        let result: anyhow::Result<ExitCode> = Err(err);
        assert_eq!(exit_code_for_result(&result), ExitCode::UserError);
    }

    #[test]
    fn tagged_error_recovers_its_explicit_code() {
        let result: anyhow::Result<()> = Err(anyhow::anyhow!("disk full"));
        let tagged = result.with_exit_code(ExitCode::SystemError).unwrap_err();
        assert_eq!(exit_code_for_error(&tagged), ExitCode::SystemError);
    }

    #[test]
    fn tagged_error_still_carries_its_original_message() {
        // The tag itself displays as nothing (see `ExitTag`'s `Display`
        // impl) -- `print_error` skips it and prints the real message, which
        // is what we can assert on here without capturing stderr.
        let result: anyhow::Result<()> = Err(anyhow::anyhow!("disk full"));
        let tagged = result.with_exit_code(ExitCode::SystemError).unwrap_err();
        assert!(tagged.downcast_ref::<ExitTag>().is_some());
        assert_eq!(tagged.chain().map(|e| e.to_string()).collect::<Vec<_>>(), vec![
            String::new(),
            "disk full".to_string(),
        ]);
    }

    #[test]
    fn plugin_error_in_chain_maps_to_plugin() {
        let plugin_err = weaver_core::plugin::PluginError::ContainerRuntimeNotFound;
        let err: anyhow::Error = plugin_err.into();
        assert_eq!(exit_code_for_error(&err), ExitCode::Plugin);
    }

    #[test]
    fn plugin_error_nested_as_a_source_still_maps_to_plugin() {
        let plugin_err = weaver_core::plugin::PluginError::PluginNotCached {
            name: "go-dep".to_string(),
        };
        let err: anyhow::Error =
            anyhow::Error::new(plugin_err).context("resolving ensure type 'go.dep'");
        assert_eq!(exit_code_for_error(&err), ExitCode::Plugin);
    }

    #[test]
    fn io_error_in_chain_maps_to_system_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err: anyhow::Error = io_err.into();
        assert_eq!(exit_code_for_error(&err), ExitCode::SystemError);
    }

    #[test]
    fn explicit_tag_wins_over_type_based_classification() {
        // A plugin error explicitly tagged UserError (e.g. a config-shape
        // problem the plugin subsystem surfaced) should honor the tag.
        let plugin_err = weaver_core::plugin::PluginError::ContainerRuntimeNotFound;
        let result: anyhow::Result<()> = Err(plugin_err.into());
        let tagged = result.with_exit_code(ExitCode::UserError).unwrap_err();
        assert_eq!(exit_code_for_error(&tagged), ExitCode::UserError);
    }
}
