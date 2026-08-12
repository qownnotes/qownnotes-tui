# Contributor Guide

## Project

`qownnotes-tui` is a Rust 2024 terminal application for browsing and editing
QOwnNotes-compatible note folders. Rust 1.85 is the minimum supported version.

## Development

- Use `just build` to build a debug binary.
- Use `just test` to run all tests.
- Use `just check` before submitting changes; it checks formatting, runs Clippy
  with warnings denied, and runs all tests.
- Use `just nix-build` when changing packaging and `just flake-check` when
  changing Nix configuration.
- Keep changes focused and follow the existing Rust and Ratatui patterns.

## Changelog

Update `CHANGELOG.md` for every notable change. Add concise entries under an
`[Unreleased]` section using the Keep a Changelog categories (`Added`, `Changed`,
`Deprecated`, `Removed`, `Fixed`, or `Security`). Documentation-only,
formatting-only, and internal refactoring changes do not need an entry unless
they affect users or contributors.

## Version Bumps

This project follows Semantic Versioning. To bump the version:

1. Update the package version in `Cargo.toml`.
2. Update the root package version in `Cargo.lock` by running
   `cargo check --locked` after changing `Cargo.toml`. If the lock file needs an
   update, run `cargo check`, then verify with `cargo check --locked`.
3. Update the package version in `flake.nix`.
4. Move the relevant entries from `[Unreleased]` into a dated
   `## [X.Y.Z] - YYYY-MM-DD` section in `CHANGELOG.md`.
5. Update the comparison links at the bottom of `CHANGELOG.md`: point
   `[Unreleased]` from `vX.Y.Z` to `HEAD` and add the new release comparison
   link.
6. Run `just check`, `just nix-build`, and `just flake-check`.

The release workflow reads the version from Cargo metadata, creates the
corresponding `vX.Y.Z` tag, and extracts release notes from the matching
`CHANGELOG.md` section. The Cargo version and changelog heading must therefore
match exactly.
