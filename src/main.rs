//! Editor executable entry point.

use editor::app::bootstrap;

fn main() -> std::process::ExitCode {
    bootstrap::run()
}
