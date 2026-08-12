use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, bail};
use ignore::WalkBuilder;

use super::model::Note;

const NOTE_EXTENSIONS: &[&str] = &["md", "txt", "markdown"];
const RESERVED_DIRECTORIES: &[&str] = &[".git", "media", "attachments", "trash"];

pub fn scan(root: &Path) -> anyhow::Result<Vec<Note>> {
    let mut notes = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .follow_links(false)
        .filter_entry(|entry| {
            entry.depth() == 0
                || !entry.file_type().is_some_and(|kind| kind.is_dir())
                || !RESERVED_DIRECTORIES
                    .iter()
                    .any(|reserved| entry.file_name() == *reserved)
        })
        .build();

    for entry in walker {
        let entry = entry.context("failed while scanning note root")?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) || !is_note(entry.path()) {
            continue;
        }
        let relative_path = safe_relative_path(root, entry.path())?;
        let metadata = entry
            .metadata()
            .with_context(|| format!("cannot read metadata for {}", entry.path().display()))?;
        notes.push(Note {
            relative_path,
            size: metadata.len(),
            modified: metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            search_text_lowercase: searchable_text(entry.path()),
        });
    }
    Ok(notes)
}

fn searchable_text(path: &Path) -> Arc<str> {
    let Ok(bytes) = fs::read(path) else {
        return Arc::from("");
    };
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec())
        .map(|text| Arc::from(text.to_lowercase()))
        .unwrap_or_else(|_| Arc::from(""))
}

pub fn safe_relative_path(root: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is outside the note root", path.display()))?;
    if relative.as_os_str().is_empty()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("unsafe note path: {}", path.display());
    }
    Ok(relative.to_path_buf())
}

fn is_note(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            NOTE_EXTENSIONS
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn scans_supported_extensions_and_nested_folders() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("work")).unwrap();
        fs::write(root.path().join("root.md"), "root").unwrap();
        fs::write(root.path().join("work/plan.MARKDOWN"), "plan").unwrap();
        fs::write(root.path().join("work/image.png"), []).unwrap();

        let notes = scan(root.path()).unwrap();
        let paths: Vec<_> = notes
            .iter()
            .map(|note| note.relative_path.as_path())
            .collect();
        assert!(paths.contains(&Path::new("root.md")));
        assert!(paths.contains(&Path::new("work/plan.MARKDOWN")));
    }

    #[test]
    fn excludes_reserved_directories() {
        let root = tempdir().unwrap();
        for directory in RESERVED_DIRECTORIES {
            fs::create_dir(root.path().join(directory)).unwrap();
            fs::write(root.path().join(directory).join("hidden.md"), "hidden").unwrap();
        }
        fs::write(root.path().join("visible.md"), "visible").unwrap();

        let notes = scan(root.path()).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].relative_path, Path::new("visible.md"));
    }

    #[test]
    fn does_not_hide_notes_listed_in_gitignore() {
        let root = tempdir().unwrap();
        fs::write(root.path().join(".gitignore"), "private.md\n").unwrap();
        fs::write(root.path().join("private.md"), "private").unwrap();

        let notes = scan(root.path()).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].relative_path, Path::new("private.md"));
    }

    #[test]
    fn rejects_paths_outside_root() {
        let root = Path::new("/notes");
        assert!(safe_relative_path(root, Path::new("/other/note.md")).is_err());
        assert!(safe_relative_path(root, root).is_err());
    }
}
