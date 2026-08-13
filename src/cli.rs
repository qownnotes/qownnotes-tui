use std::path::PathBuf;

use clap::Parser;
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Root directory containing QOwnNotes-compatible notes
    #[arg(long, value_name = "PATH", env = "QOWNNOTES_TUI_NOTES_DIR")]
    pub notes_dir: Option<PathBuf>,

    /// Generate a completion script for a shell and exit
    #[arg(long, value_enum, value_name = "SHELL")]
    pub generate_completion: Option<Shell>,
}
