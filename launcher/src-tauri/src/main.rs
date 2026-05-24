use std::process::ExitCode;

fn main() -> ExitCode {
    // The GUI only makes sense on a bare invocation — `run_gui` ignores
    // argv. Any arguments are a CLI invocation, so hand them to Clap,
    // which owns parsing, the required-subcommand check, global flags
    // (`-v`/`--verbose`, `--help`, `--version`), and all error reporting.
    if std::env::args().nth(1).is_some() {
        reliquaint::cli::run()
    } else {
        reliquaint::gui::run_gui();
        ExitCode::SUCCESS
    }
}
