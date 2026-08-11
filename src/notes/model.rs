use std::{path::PathBuf, time::SystemTime};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    /// Normalized path relative to the configured note root.
    pub relative_path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
}

impl Note {
    pub fn name(&self) -> String {
        self.relative_path
            .file_stem()
            .unwrap_or(self.relative_path.as_os_str())
            .to_string_lossy()
            .into_owned()
    }
}
