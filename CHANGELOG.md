# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-08-12

### Added

- Editing the selected note directly from the note list with `e`.
- Incremental, case-insensitive search across note names and note text with `/`.
- `Esc` clearing a retained note search after leaving search input or editing a note.
- Page and note-boundary cursor navigation while editing notes.

## [0.2.0] - 2026-08-11

### Added

- In-app multiline note editing from the viewer with `e`.
- Markdown syntax highlighting while editing notes.
- Automatic preview of the first note when a note folder is loaded.
- Persistence of the selected note folder between app launches.
- Automatic note saving with a configurable interval of 10 seconds by default.
- A Settings dialog with a General tab for changing the note save interval.
- Automatic viewer reload when the current note changes outside the app.
- Conflict detection that prevents autosave from overwriting external changes.

## [0.1.0]

### Added

- Read-only terminal browser for QOwnNotes-compatible Markdown folders.
- Automatic discovery of all available note folders configured in QOwnNotes.
- Note-folder selector that starts with the folder currently active in QOwnNotes.
- Notes pane focused by default at startup.
- Recursive background scanning for `md`, `txt`, and `markdown` notes.
- Three-pane desktop layout and single-pane narrow-terminal layout.
- Read-only note viewer with soft wrapping and bounded scrolling.
- Line, page, beginning, and end navigation in the viewer.
- Markdown source highlighting for headings, lists, blockquotes, fenced and inline
  code, links, bold text, and italic text.
- Vertical viewer scrollbar.
- Mouse selection for note folders and notes, plus mouse-wheel scrolling.
- Note-list sorting that follows QOwnNotes' `notesPanelSort` and
  `notesPanelOrder` settings.
- UTF-8 validation and UTF-8 BOM handling.
- CLI, environment-variable, and TOML configuration support for an explicit note
  root.
- Structured logging and panic-safe terminal restoration.
- Nix package, flake, devenv development shell, and `just` build recipes.
- Shared devenv and `just` configuration from `pbek/nix-shared`.
- Cross-platform CI for Linux, macOS, and Windows.

### Fixed

- Rust 1.85 CI builds by pinning `ignore` to its last MSRV-compatible release.
- Dependency-audit CI permissions by running `cargo audit` directly.
- Removed the vulnerable `time` dependency previously pulled in by log rotation.

### Security

- QOwnNotes configuration databases are opened read-only.
- Directory symlinks are not followed while scanning notes.
- Reserved `.git`, `media`, `attachments`, and `trash` directories are excluded
  from note discovery.

[Unreleased]: https://github.com/qownnotes/qownnotes-tui/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/qownnotes/qownnotes-tui/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/qownnotes/qownnotes-tui/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/qownnotes/qownnotes-tui/releases/tag/v0.1.0
