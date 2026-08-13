mod app;
mod cli;
mod clipboard;
mod config;
mod error;
mod event;
mod markdown;
mod notes;
mod terminal;
mod theme;
mod ui;

use std::{fs::OpenOptions, io, panic, sync::Mutex};

use anyhow::Context;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::Cli;
use config::Config;
use directories::ProjectDirs;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if let Some(shell) = cli.generate_completion {
        let mut command = Cli::command();
        generate(shell, &mut command, "qownnotes-tui", &mut io::stdout());
        return Ok(());
    }

    let mut config = Config::load(&cli)?;
    config.theme = theme::Theme::load()?;
    init_logging();

    tracing::info!(
        notes_dir = %config.note_folders[config.active_folder].path.display(),
        note_folders = config.note_folders.len(),
        "starting application"
    );
    let terminal = terminal::TerminalGuard::enter().context("failed to initialize terminal")?;
    install_panic_hook();
    app::run(terminal, config)
}

fn init_logging() {
    let Some(dirs) = ProjectDirs::from("org", "QOwnNotes", "qownnotes-tui") else {
        return;
    };
    if std::fs::create_dir_all(dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir())).is_err() {
        return;
    }
    let path = dirs
        .state_dir()
        .unwrap_or_else(|| dirs.data_local_dir())
        .join("qownnotes-tui.log");
    let Ok(file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .with_writer(Mutex::new(file))
        .try_init();
}

fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = terminal::restore_terminal();
        previous(info);
    }));
}
