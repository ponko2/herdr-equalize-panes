mod equalizer;
mod herdr;
mod pane_tree;
mod plugin;

use env_logger::Env;
use std::process::ExitCode;

fn main() -> ExitCode {
    env_logger::Builder::from_env(Env::new().filter_or("HERDR_PLUGIN_LOG", "warn")).init();

    if let Err(error) = plugin::run() {
        log::error!("{error:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
