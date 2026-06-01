//! Tracing subscriber setup. See ADR-0004.
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

use std::sync::{Mutex, OnceLock};

use tauri::Emitter;
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
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));

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
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(TauriBridgeLayer::new())
        .try_init();
}

/// Global slot for the Tauri AppHandle. Set by [`set_gui_app_handle`]
/// from `gui::run_gui::setup` once the app is built. Until then,
/// `TauriBridgeLayer` events are dropped silently.
static GUI_APP: OnceLock<Mutex<Option<tauri::AppHandle>>> = OnceLock::new();

/// Register the Tauri `AppHandle` so subsequent tracing events fire as
/// `"log"` events to the frontend. Called once during GUI startup.
pub fn set_gui_app_handle(app: tauri::AppHandle) {
    let cell = GUI_APP.get_or_init(|| Mutex::new(None));
    *cell.lock().unwrap() = Some(app);
}

/// Tauri-side layer that turns each tracing event into a `"log"` event
/// on the Tauri event bus. The frontend (DiagnosticPanel) listens and
/// renders them in the diagnostic panel alongside emulator output.
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
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(cell) = GUI_APP.get() else {
            return;
        };
        let guard = match cell.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(app) = guard.as_ref() else {
            return;
        };

        let level = match *event.metadata().level() {
            tracing::Level::ERROR => "ERROR",
            tracing::Level::WARN => "WARN",
            tracing::Level::INFO => "INFO",
            tracing::Level::DEBUG => "DEBUG",
            tracing::Level::TRACE => "TRACE",
        };
        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);

        let _ = app.emit(
            "log",
            serde_json::json!({
                "level": level,
                "target": event.metadata().target(),
                "message": visitor.message,
                "fields": visitor.fields,
            }),
        );
    }
}

/// Pulls the `message` and structured fields out of a `tracing::Event`.
#[derive(Default)]
struct EventVisitor {
    message: String,
    fields: String,
}

impl tracing::field::Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            if !self.fields.is_empty() {
                self.fields.push(' ');
            }
            self.fields.push_str(&format!("{}={value:?}", field.name()));
        }
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
