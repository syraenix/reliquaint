//! Tracing subscriber setup. See `docs/adr-0004-logging-strategy.md`.
//!
//! Conventions for instrumenting code:
//!
//! - **Levels** — `error!` for failures the user cannot proceed past, `warn!`
//!   for surprising-but-recoverable conditions, `info!` for high-level
//!   operations a user might want to see (default verbosity), `debug!` for
//!   diagnosis detail (`-v`), `trace!` for very fine-grained internals
//!   (`-vv`).
//! - **Spans** — open a span for user-meaningful operations
//!   (`launch_game`, `load_tap`, `install_game`, `doctor.<check>`). Events
//!   are for state changes inside a span.
//! - **Structured fields** — carry context as fields (`game_id`, `tap_id`,
//!   `path`), not interpolated into the message. This makes log output
//!   filterable and bug reports actionable.
//! - **Errors** — library code does not log errors; it returns them. The
//!   binary layer decides where they go.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initialize the global tracing subscriber for CLI invocations.
///
/// Output goes to stderr. Default level is `INFO`; `verbosity == 1` raises
/// to `DEBUG`, `verbosity >= 2` to `TRACE`. `RUST_LOG`, if set, overrides
/// the verbosity argument entirely.
pub fn init_cli(verbosity: u8) {
    let default = match verbosity {
        0 => "info",
        1 => "reliquaint=debug,info",
        _ => "reliquaint=trace,debug",
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init();
}

/// Initialize the global tracing subscriber for GUI invocations.
///
/// Composes a stderr formatter with the stub Tauri bridge layer that
/// Milestone 5 will wire to the frontend's diagnostic panel.
pub fn init_gui() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(TauriBridgeLayer::new())
        .try_init();
}

/// Stub layer that will forward tracing events to the Tauri frontend in
/// Milestone 5. For now it's a no-op so the subscriber stack is fully
/// composed and parser tasks can instrument freely.
struct TauriBridgeLayer;

impl TauriBridgeLayer {
    fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for TauriBridgeLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(
        &self,
        _event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // TODO(milestone-5): forward event payload to the frontend via
        // tauri::AppHandle::emit() once the AppHandle is available here.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_cli_is_safe_to_call() {
        init_cli(0);
    }

    #[test]
    fn init_gui_is_safe_to_call() {
        // Second-call is a no-op because the global subscriber is already
        // installed; should not panic.
        init_gui();
        init_gui();
    }
}
