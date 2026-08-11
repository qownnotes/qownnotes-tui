# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/qownnotes/qownnotes-tui/commits/main
