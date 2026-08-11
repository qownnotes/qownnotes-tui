use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum NoteReadError {
    #[error("unable to read note: {0}")]
    Io(#[from] std::io::Error),
    #[error("note is not valid UTF-8 and is read-only: {0}")]
    InvalidUtf8(PathBuf),
}
