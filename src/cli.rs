use std::path::PathBuf;

use clap::Parser;
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Root directory containing QOwnNotes-compatible notes
    #[arg(long, value_name = "PATH", env = "QOWNNOTES_TUI_NOTES_DIR")]
    pub notes_dir: Option<PathBuf>,

    /// Run in a different context for QOwnNotes settings and internal files
    #[arg(long, value_name = "NAME", env = "QOWNNOTES_TUI_SESSION")]
    pub session: Option<String>,

    /// Generate a completion script for a shell and exit
    #[arg(long, value_enum, value_name = "SHELL")]
    pub generate_completion: Option<Shell>,
}
