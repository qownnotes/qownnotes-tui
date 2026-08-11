use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Root directory containing QOwnNotes-compatible notes
    #[arg(long, value_name = "PATH", env = "QOWNNOTES_TUI_NOTES_DIR")]
    pub notes_dir: Option<PathBuf>,
}
