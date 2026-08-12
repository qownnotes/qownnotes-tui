use std::{path::PathBuf, sync::Arc, time::SystemTime};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    /// Normalized path relative to the configured note root.
    pub relative_path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    pub search_text_lowercase: Arc<str>,
}

impl Note {
    pub fn name(&self) -> String {
        self.relative_path
            .file_stem()
            .unwrap_or(self.relative_path.as_os_str())
            .to_string_lossy()
            .into_owned()
    }

    pub fn matches_search(&self, terms: &[String]) -> bool {
        let name = self.name().to_lowercase();
        terms
            .iter()
            .all(|term| name.contains(term) || self.search_text_lowercase.contains(term))
    }
}
