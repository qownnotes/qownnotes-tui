use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use unicode_segmentation::UnicodeSegmentation;

pub fn new_note_title(now: NaiveDateTime) -> String {
    format!("Note {}", now.format("%Y-%m-%d %Hh%Ms%S"))
}

pub fn initial_content(title: &str) -> String {
    format!("# {}\n\n", title.trim())
}

pub fn automatic_relative_path(root: &Path, current: &Path, content: &str) -> PathBuf {
    let parent = current.parent().unwrap_or_else(|| Path::new(""));
    let extension = current
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("md");
    let name = note_name(content);

    for suffix in 0..=1000 {
        let base = if suffix == 0 {
            name.clone()
        } else {
            format!("{name} {suffix}")
        };
        let candidate = parent.join(format!("{base}.{extension}"));
        if same_filename(&candidate, current) || !filename_exists(root, &candidate) {
            return candidate;
        }
    }

    current.to_path_buf()
}

fn note_name(content: &str) -> String {
    let content = without_leading_metadata(content).trim();
    let first_line = content.lines().next().unwrap_or_default().trim();
    let headline = first_line.strip_prefix("# ").unwrap_or(first_line);
    let headline = strip_leading_emoji(headline);
    let sanitized = sanitize(headline);
    if sanitized.is_empty() {
        "Note".into()
    } else {
        sanitized
    }
}

fn without_leading_metadata(content: &str) -> &str {
    if let Some(rest) = content.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            return rest[end + 4..]
                .strip_prefix('\n')
                .unwrap_or(&rest[end + 4..]);
        }
    }
    if content.starts_with("<!--") {
        if let Some(end) = content.find("-->") {
            return content[end + 3..]
                .strip_prefix('\n')
                .unwrap_or(&content[end + 3..]);
        }
    }
    content
}

fn strip_leading_emoji(value: &str) -> &str {
    let Some(grapheme) = value.graphemes(true).next() else {
        return value;
    };
    let Some(first) = grapheme.chars().next().map(u32::from) else {
        return value;
    };
    let is_emoji = matches!(first,
        0x1f300..=0x1faff | 0x2702..=0x27b0 | 0x1f100..=0x1f1ff | 0x2600..=0x26ff
    );
    if is_emoji {
        value[grapheme.len()..].trim_start()
    } else {
        value
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '/' | '\\' | ':'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn same_filename(left: &Path, right: &Path) -> bool {
    left.parent() == right.parent()
        && left
            .file_name()
            .zip(right.file_name())
            .is_some_and(|(left, right)| {
                left.to_string_lossy()
                    .eq_ignore_ascii_case(&right.to_string_lossy())
            })
}

fn filename_exists(root: &Path, candidate: &Path) -> bool {
    let directory = root.join(candidate.parent().unwrap_or_else(|| Path::new("")));
    let Some(filename) = candidate.file_name() else {
        return false;
    };
    std::fs::read_dir(directory).ok().is_some_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(&filename.to_string_lossy())
        })
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn formats_qownnotes_timestamp_title_and_heading() {
        let now = NaiveDate::from_ymd_opt(2026, 8, 12)
            .unwrap()
            .and_hms_opt(14, 37, 5)
            .unwrap();
        let title = new_note_title(now);

        assert_eq!(title, "Note 2026-08-12 14h37s05");
        assert_eq!(initial_content(&title), "# Note 2026-08-12 14h37s05\n\n");
    }

    #[test]
    fn derives_sanitized_filename_from_first_meaningful_line() {
        let root = tempdir().unwrap();
        assert_eq!(
            automatic_relative_path(
                root.path(),
                Path::new("old.md"),
                "---\ntags: test\n---\n# 🚀 Project: alpha/beta\\gamma\nbody"
            ),
            Path::new("Project alphabetagamma.md")
        );
        assert_eq!(
            automatic_relative_path(
                root.path(),
                Path::new("nested/old.txt"),
                "<!-- metadata -->\nSetext title\n============"
            ),
            Path::new("nested/Setext title.txt")
        );
    }

    #[test]
    fn adds_a_suffix_when_the_derived_filename_exists() {
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("project.md"), "existing").unwrap();
        std::fs::write(root.path().join("Project 1.md"), "existing").unwrap();

        assert_eq!(
            automatic_relative_path(root.path(), Path::new("old.md"), "# Project\n"),
            Path::new("Project 2.md")
        );
    }
}
