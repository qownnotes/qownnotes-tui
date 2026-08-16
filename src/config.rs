use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use directories::{BaseDirs, ProjectDirs};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::cli::Cli;
use crate::theme::Theme;

#[derive(Debug)]
pub struct Config {
    pub note_folders: Vec<NoteFolder>,
    pub active_folder: usize,
    pub note_sort: NoteSort,
    pub save_interval_seconds: u64,
    pub theme: Theme,
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    notes_dir: Option<PathBuf>,
    #[serde(default)]
    selected_note_folder: Option<PathBuf>,
    #[serde(default)]
    selected_note: Option<PathBuf>,
    #[serde(default = "default_save_interval_seconds")]
    save_interval_seconds: u64,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            notes_dir: None,
            selected_note_folder: None,
            selected_note: None,
            save_interval_seconds: default_save_interval_seconds(),
        }
    }
}

const fn default_save_interval_seconds() -> u64 {
    10
}

impl Config {
    pub fn load(cli: &Cli) -> anyhow::Result<Self> {
        let file = load_file()?;
        let save_interval_seconds = file.save_interval_seconds.max(1);
        let qownnotes = qownnotes_paths(cli.session.as_deref());
        let note_sort = qownnotes
            .as_ref()
            .and_then(|paths| fs::read_to_string(&paths.settings).ok())
            .map_or(NoteSort::LastModified, |settings| {
                note_sort_from_qsettings(&settings)
            });
        if let Some(notes_dir) = cli.notes_dir.clone().or(file.notes_dir.clone()) {
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
                save_interval_seconds,
                theme: Theme::default(),
            });
        }

        let (note_folders, qownnotes_active_folder) = qownnotes
            .as_ref()
            .and_then(discover_qownnotes_note_folders)
            .context(
                "no note root configured; pass --notes-dir PATH, set \
                 QOWNNOTES_TUI_NOTES_DIR, add notes_dir to the configuration file, \
                 or configure a note folder in QOwnNotes",
            )?;
        let active_folder = selected_folder_index(
            &note_folders,
            file.selected_note_folder.as_deref(),
            qownnotes_active_folder,
        );
        Ok(Self {
            note_folders,
            active_folder,
            note_sort,
            save_interval_seconds,
            theme: Theme::default(),
        })
    }
}

fn selected_folder_index(
    note_folders: &[NoteFolder],
    selected: Option<&Path>,
    fallback: usize,
) -> usize {
    selected
        .and_then(|selected| {
            note_folders
                .iter()
                .position(|folder| folder.path == selected)
        })
        .unwrap_or(fallback)
}

pub fn save_interval_seconds(value: u64) -> anyhow::Result<()> {
    let mut file = load_file()?;
    file.save_interval_seconds = value.max(1);
    save_file(&file)
}

pub fn selected_note_folder(value: &Path) -> anyhow::Result<()> {
    let mut file = load_file()?;
    file.selected_note_folder = Some(value.to_path_buf());
    save_file(&file)
}

pub fn remembered_note(root: &Path) -> anyhow::Result<Option<PathBuf>> {
    let file = load_file()?;
    Ok(relative_selected_note(root, file.selected_note.as_deref()))
}

pub fn selected_note(root: &Path, relative: Option<&Path>) -> anyhow::Result<()> {
    let mut file = load_file()?;
    file.selected_note = relative.map(|relative| root.join(relative));
    save_file(&file)
}

fn relative_selected_note(root: &Path, selected: Option<&Path>) -> Option<PathBuf> {
    selected?.strip_prefix(root).ok().map(Path::to_path_buf)
}

fn save_file(file: &FileConfig) -> anyhow::Result<()> {
    let path = config_path().context("cannot determine configuration directory")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(file).context("cannot serialize configuration")?;
    fs::write(&path, contents).with_context(|| format!("cannot write {}", path.display()))
}

/// Locations of the QOwnNotes settings file and database for an optional
/// session name, mirroring QOwnNotes' `--session` handling.
pub struct QOwnNotesPaths {
    pub settings: PathBuf,
    pub database: PathBuf,
}

fn qownnotes_paths(session: Option<&str>) -> Option<QOwnNotesPaths> {
    let dirs = BaseDirs::new()?;
    let app = match session {
        Some(session) => format!("QOwnNotes-{session}"),
        None => "QOwnNotes".to_string(),
    };
    Some(QOwnNotesPaths {
        settings: dirs.config_dir().join(format!("PBE/{app}.conf")),
        database: dirs.data_dir().join(format!("PBE/{app}/QOwnNotes.sqlite")),
    })
}

fn discover_qownnotes_note_folders(paths: &QOwnNotesPaths) -> Option<(Vec<NoteFolder>, usize)> {
    discover_from_qownnotes(&paths.settings, &paths.database).ok()
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
    let Some(path) = config_path() else {
        return Ok(FileConfig::default());
    };
    match fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents)
            .with_context(|| format!("invalid configuration file {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FileConfig::default()),
        Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
    }
}

fn config_path() -> Option<PathBuf> {
    Some(
        ProjectDirs::from("org", "QOwnNotes", "qownnotes-tui")?
            .config_dir()
            .join("config.toml"),
    )
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
    fn resolves_qownnotes_paths_per_session() {
        let dirs = BaseDirs::new().unwrap();

        let default = qownnotes_paths(None).unwrap();
        assert_eq!(
            default.settings,
            dirs.config_dir().join("PBE/QOwnNotes.conf")
        );
        assert_eq!(
            default.database,
            dirs.data_dir().join("PBE/QOwnNotes/QOwnNotes.sqlite")
        );

        let session = qownnotes_paths(Some("test")).unwrap();
        assert_eq!(
            session.settings,
            dirs.config_dir().join("PBE/QOwnNotes-test.conf")
        );
        assert_eq!(
            session.database,
            dirs.data_dir().join("PBE/QOwnNotes-test/QOwnNotes.sqlite")
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

    #[test]
    fn reads_the_selected_note_folder_from_the_app_config() {
        let file: FileConfig =
            toml::from_str(
                "selected_note_folder = '/notes/work'\nselected_note = '/notes/work/todo.md'\nsave_interval_seconds = 10\n",
            )
            .unwrap();

        assert_eq!(
            file.selected_note_folder,
            Some(PathBuf::from("/notes/work"))
        );
        assert_eq!(
            file.selected_note,
            Some(PathBuf::from("/notes/work/todo.md"))
        );
    }

    #[test]
    fn prefers_the_saved_note_folder_when_it_is_available() {
        let folders = vec![
            NoteFolder {
                name: "First".into(),
                path: "/notes/first".into(),
            },
            NoteFolder {
                name: "Saved".into(),
                path: "/notes/saved".into(),
            },
        ];

        assert_eq!(
            selected_folder_index(&folders, Some(Path::new("/notes/saved")), 0),
            1
        );
        assert_eq!(
            selected_folder_index(&folders, Some(Path::new("/notes/missing")), 0),
            0
        );
    }

    #[test]
    fn restores_only_notes_inside_the_active_folder() {
        assert_eq!(
            relative_selected_note(
                Path::new("/notes/work"),
                Some(Path::new("/notes/work/projects/todo.md"))
            ),
            Some(PathBuf::from("projects/todo.md"))
        );
        assert_eq!(
            relative_selected_note(
                Path::new("/notes/work"),
                Some(Path::new("/notes/personal/todo.md"))
            ),
            None
        );
    }
}
