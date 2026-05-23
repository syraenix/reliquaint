use std::process::ExitCode;

fn main() -> ExitCode {
    let is_cli = std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "list" | "run" | "doctor" | "--help" | "-h" | "--version" | "-V"));

    if is_cli {
        reliquaint::cli::run()
    } else {
        reliquaint::gui::run_gui();
        ExitCode::SUCCESS
    }
}
