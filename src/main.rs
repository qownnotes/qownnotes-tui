mod app;
mod cli;
mod config;
mod error;
mod event;
mod markdown;
mod notes;
mod terminal;
mod ui;

use std::panic;

use anyhow::Context;
use clap::Parser;
use cli::Cli;
use config::Config;
use directories::ProjectDirs;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load(&cli)?;
    let _log_guard = init_logging();

    tracing::info!(
        notes_dir = %config.note_folders[config.active_folder].path.display(),
        note_folders = config.note_folders.len(),
        "starting application"
    );
    let terminal = terminal::TerminalGuard::enter().context("failed to initialize terminal")?;
    install_panic_hook();
    app::run(terminal, config)
}

fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let dirs = ProjectDirs::from("org", "QOwnNotes", "qownnotes-tui")?;
    if std::fs::create_dir_all(dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir())).is_err() {
        return None;
    }
    let writer = tracing_appender::rolling::daily(
        dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir()),
        "qownnotes-tui.log",
    );
    let (writer, guard) = tracing_appender::non_blocking(writer);
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .with_writer(writer)
        .init();
    Some(guard)
}

fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = terminal::restore_terminal();
        previous(info);
    }));
}
