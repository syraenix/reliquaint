//! Error-handling foundation. See `docs/adr-0005-error-handling-strategy.md`.
//!
//! Conventions:
//!
//! - **Library code** (every module besides `main`, `gui`, `commands`) uses
//!   per-module `thiserror` enums with structured-field variants. Foreign
//!   errors (`io::Error`, `toml::de::Error`, ...) wrap as variants with
//!   `#[source]`. Library code never returns `anyhow::Error`.
//! - **Binary / Tauri-handler code** uses `anyhow::Result` and adds
//!   call-site context with `.context()`.
//! - Library code does not log errors on the way up — it returns them. The
//!   binary layer decides where they go (stderr, GUI panel, log file).
//! - `unwrap()` / `expect()` are reserved for statically-guaranteed
//!   invariants; the `expect()` message describes the invariant, not the
//!   operation.

/// Top-level error type for the reliquaint library.
///
/// Parser modules add domain-specific variants via `#[from]` as they're
/// implemented (catalog → `CatalogError`, install records → `InstallError`,
/// tap metadata → `TapError`, ...). Marked `#[non_exhaustive]` so new
/// variants can be added without breaking exhaustive matches downstream.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReliquaintError {
    #[error(transparent)]
    Catalog(#[from] crate::catalog::CatalogError),
}

/// Install a panic hook that prints a "please file a bug" banner before
/// re-invoking the previous hook. Call once at each binary entry point.
///
/// The hook tells the user how to capture trace-level logs for the bug
/// report. It chains to the previous hook rather than replacing it, so the
/// standard backtrace still prints.
pub fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!();
        eprintln!("reliquaint panicked unexpectedly — this is a bug.");
        eprintln!("Please file an issue and include trace-level logs:");
        eprintln!("    RUST_LOG=trace reliquaint <your command> 2> reliquaint.log");
        eprintln!();
        original(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_hook_runs_and_panic_is_caught() {
        install_panic_hook();
        let result = std::panic::catch_unwind(|| {
            panic!("deliberate test panic");
        });
        assert!(result.is_err(), "panic should be caught by catch_unwind");
    }
}
