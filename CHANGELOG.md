# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-08-25

### Added

- Expandable note subfolders in the folder pane, with direct-folder filtering,
  mouse and keyboard expansion, and new-note creation in the active subfolder;
  availability follows the QOwnNotes note folder's `show_subfolders` setting,
  and directories matching its `ignoreNoteSubFolders` regular expressions are excluded
  ([#1](https://github.com/qownnotes/qownnotes-tui/issues/1)).

### Fixed

- `PageUp` and `PageDown` now move the viewer cursor while scrolling and no
  longer reset the viewport to its previous position.

## [0.6.0] - 2026-08-16

### Added

- A `--session` option to use a separate QOwnNotes settings and database
  context, matching QOwnNotes' `--session` argument; the `nix-run` and
  `nix-build-run` Just recipes now use the `test` session by default.
- `Ctrl-Space` opens the link at the viewer cursor or toggles the checkbox at
  the cursor in the viewer and editor.
- Entering edit mode from the viewer keeps the cursor position and selection,
  and leaving the editor returns the cursor position to the viewer.

### Changed

- Relicensed under GPL-3.0-only; the `LICENSE` file now contains the complete
  license text.

## [0.5.0] - 2026-08-15

### Added

- Markdown highlighting for angle-bracketed URI autolinks.
- Clickable HTTP and HTTPS links in the viewer, including links that wrap across
  lines.
- Restoration of the last opened note when the application starts.

## [0.4.0] - 2026-08-13

### Added

- Shell completion generation, with Bash, Fish, and Zsh scripts installed by
  the Nix package.
- Clickable relative, legacy `note://`, and wiki-style note links in the viewer,
  including heading navigation for relative Markdown links.
- Back and forward note navigation with `Alt-Left` and `Alt-Right`, preserving
  each history entry's viewer and editor position.
- Multiline text selection in the viewer and editor with `Shift` and the arrow
  keys, plus clipboard cut, copy, and paste with `Ctrl-X`, `Ctrl-C`, and `Ctrl-V`.
- Mouse drag selection in the viewer and editor, including wrapped and scrolled text.
- A visible viewer cursor with arrow-key navigation and `Shift-Arrow` selection.
- Reverse pane focus cycling with `Shift-Tab`.
- `Esc` navigation from the viewer back to the note list.
- Checkbox-list highlighting in the note viewer and editor.
- Setext-style heading highlighting in the note viewer and editor.

## [0.3.0] - 2026-08-12

### Added

- Editing the selected note directly from the note list with `e`.
- Incremental, case-insensitive search across note names and note text with `/`.
- `Esc` clearing a retained note search after leaving search input or editing a note.
- Page and note-boundary cursor navigation while editing notes.
- Timestamped note creation with `n` or `Ctrl-n`, matching QOwnNotes' default
  date/time heading format.
- Automatic filename updates from note headings on save, including sanitization,
  leading-emoji removal, and numeric collision suffixes.
- Confirmed note deletion with platform-trash support and permanent removal as
  a fallback when trashing is unavailable.
- Configurable UI and Markdown colors through `theme.toml`, with defaults that
  follow the terminal's ANSI color palette.
- A Home Manager module for installing the application and generating
  `theme.toml` declaratively.

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

[Unreleased]: https://github.com/qownnotes/qownnotes-tui/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/qownnotes/qownnotes-tui/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/qownnotes/qownnotes-tui/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/qownnotes/qownnotes-tui/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/qownnotes/qownnotes-tui/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/qownnotes/qownnotes-tui/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/qownnotes/qownnotes-tui/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/qownnotes/qownnotes-tui/releases/tag/v0.1.0
