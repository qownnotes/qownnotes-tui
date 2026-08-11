use std::{fs, path::PathBuf};

use anyhow::{Context, bail};
use directories::{BaseDirs, ProjectDirs};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;

use crate::cli::Cli;

#[derive(Debug)]
pub struct Config {
    pub note_folders: Vec<NoteFolder>,
    pub active_folder: usize,
    pub note_sort: NoteSort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteFolder {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoteSort {
    LastModified,
    Alphabetical { descending: bool },
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    notes_dir: Option<PathBuf>,
}

impl Config {
    pub fn load(cli: &Cli) -> anyhow::Result<Self> {
        let file = load_file()?;
        let note_sort = qownnotes_settings_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .map_or(NoteSort::LastModified, |settings| {
                note_sort_from_qsettings(&settings)
            });
        if let Some(notes_dir) = cli.notes_dir.clone().or(file.notes_dir) {
            let path = notes_dir
                .canonicalize()
                .with_context(|| format!("cannot access note root {}", notes_dir.display()))?;
            if !path.is_dir() {
                bail!("note root is not a directory: {}", path.display());
            }
            let name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned();
            return Ok(Self {
                note_folders: vec![NoteFolder { name, path }],
                active_folder: 0,
                note_sort,
            });
        }

        let (note_folders, active_folder) = discover_qownnotes_note_folders().context(
            "no note root configured; pass --notes-dir PATH, set \
             QOWNNOTES_TUI_NOTES_DIR, add notes_dir to the configuration file, \
             or configure a note folder in QOwnNotes",
        )?;
        Ok(Self {
            note_folders,
            active_folder,
            note_sort,
        })
    }
}

fn discover_qownnotes_note_folders() -> Option<(Vec<NoteFolder>, usize)> {
    let dirs = BaseDirs::new()?;
    let settings_path = qownnotes_settings_path()?;
    let database_path = dirs.data_dir().join("PBE/QOwnNotes/QOwnNotes.sqlite");
    discover_from_qownnotes(&settings_path, &database_path).ok()
}

fn qownnotes_settings_path() -> Option<PathBuf> {
    Some(BaseDirs::new()?.config_dir().join("PBE/QOwnNotes.conf"))
}

fn note_sort_from_qsettings(settings: &str) -> NoteSort {
    match qsettings_value(settings, "notesPanelSort") {
        Some("0") => NoteSort::Alphabetical {
            descending: qsettings_value(settings, "notesPanelOrder") == Some("1"),
        },
        _ => NoteSort::LastModified,
    }
}

fn discover_from_qownnotes(
    settings_path: &std::path::Path,
    database_path: &std::path::Path,
) -> anyhow::Result<(Vec<NoteFolder>, usize)> {
    let settings = fs::read_to_string(settings_path).unwrap_or_default();
    let active_id = qsettings_value(&settings, "currentNoteFolderId")
        .and_then(|value| value.parse::<i64>().ok());
    let active_path = qsettings_value(&settings, "notesPath").map(PathBuf::from);
    let connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("cannot open QOwnNotes database {}", database_path.display()))?;
    let mut query = connection
        .prepare("SELECT id, name, local_path FROM noteFolder ORDER BY priority ASC, id ASC")?;
    let folders = query
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                PathBuf::from(row.get::<_, String>(2)?),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let resolve = |path: &std::path::Path| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            database_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .join(path)
        }
    };
    let mut available = Vec::new();
    let mut active_folder = None;
    for (id, name, stored_path) in folders {
        let path = resolve(&stored_path);
        if !path.is_dir() {
            continue;
        }
        let is_active = active_id == Some(id)
            || active_path
                .as_ref()
                .is_some_and(|active| resolve(active) == path);
        if is_active {
            active_folder = Some(available.len());
        }
        let name = if name.trim().is_empty() {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        } else {
            name
        };
        available.push(NoteFolder { name, path });
    }
    if available.is_empty() {
        bail!("QOwnNotes has no available configured note folders");
    }
    Ok((available, active_folder.unwrap_or(0)))
}

fn qsettings_value<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    let mut in_general = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_general = line == "[General]";
        } else if in_general {
            if let Some(value) = line
                .strip_prefix(key)
                .and_then(|line| line.strip_prefix('='))
            {
                return Some(value.trim());
            }
        }
    }
    None
}

fn load_file() -> anyhow::Result<FileConfig> {
    let Some(dirs) = ProjectDirs::from("org", "QOwnNotes", "qownnotes-tui") else {
        return Ok(FileConfig::default());
    };
    let path = dirs.config_dir().join("config.toml");
    match fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents)
            .with_context(|| format!("invalid configuration file {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FileConfig::default()),
        Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn discovers_the_active_qownnotes_folder() {
        let root = tempdir().unwrap();
        let first = root.path().join("first");
        let active = root.path().join("active");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&active).unwrap();
        let database = root.path().join("QOwnNotes.sqlite");
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE noteFolder (
                    id INTEGER PRIMARY KEY,
                    name TEXT,
                    local_path TEXT,
                    priority INTEGER DEFAULT 0
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO noteFolder (id, name, local_path, priority) VALUES (1, 'First', ?1, 0)",
                [first.to_str().unwrap()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO noteFolder (id, name, local_path, priority) VALUES (2, 'Active', ?1, 1)",
                [active.to_str().unwrap()],
            )
            .unwrap();
        drop(connection);
        let settings = root.path().join("QOwnNotes.conf");
        fs::write(&settings, "[General]\ncurrentNoteFolderId=2\n").unwrap();

        let (folders, active_folder) = discover_from_qownnotes(&settings, &database).unwrap();
        assert_eq!(active_folder, 1);
        assert_eq!(folders.len(), 2);
        assert_eq!(
            folders[0],
            NoteFolder {
                name: "First".into(),
                path: first
            }
        );
        assert_eq!(
            folders[1],
            NoteFolder {
                name: "Active".into(),
                path: active
            }
        );
    }

    #[test]
    fn reads_only_general_qsettings_values() {
        let settings = "[Other]\nnotesPath=/wrong\n[General]\nnotesPath=/notes\n";
        assert_eq!(qsettings_value(settings, "notesPath"), Some("/notes"));
        assert_eq!(qsettings_value(settings, "missing"), None);
    }

    #[test]
    fn reads_qownnotes_note_sorting() {
        assert_eq!(
            note_sort_from_qsettings("[General]\nnotesPanelSort=0\nnotesPanelOrder=1\n"),
            NoteSort::Alphabetical { descending: true }
        );
        assert_eq!(
            note_sort_from_qsettings("[General]\nnotesPanelSort=1\nnotesPanelOrder=0\n"),
            NoteSort::LastModified
        );
        assert_eq!(note_sort_from_qsettings(""), NoteSort::LastModified);
    }
}
