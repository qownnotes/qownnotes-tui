# qownnotes-tui

`qownnotes-tui` is a keyboard-first terminal browser for local,
QOwnNotes-compatible Markdown note folders. The current implementation is the
read-only first slice described in [`PROJECT_PLAN.md`](PROJECT_PLAN.md).
See [`CHANGELOG.md`](CHANGELOG.md) for notable changes.

## Run

Enter the development environment and open a note folder:

```console
nix develop
just run --notes-dir ~/Notes
```

Alternatively, build and run the Nix package directly:

```console
nix run . -- --notes-dir ~/Notes
```

The note root can also be set with `QOWNNOTES_TUI_NOTES_DIR` or with
`notes_dir = "/path/to/notes"` in the platform configuration file. CLI options
take precedence over the environment and configuration file. If none is set,
the application reads all available configured note folders from QOwnNotes and
initially opens its current folder. The QOwnNotes application database is
opened read-only.

## Keys

| Key                  | Action                                |
| -------------------- | ------------------------------------- |
| `j`, `k`             | Move selection or scroll the viewer   |
| `PageUp`, `PageDown` | Scroll the viewer by one page         |
| `Home`, `End`        | Jump to the start or end of the note  |
| `h`, `l`, `Tab`      | Switch panes                          |
| `Enter`              | Activate a note folder or open a note |
| `R`                  | Rescan the active note folder         |
| `?`                  | Show help                             |
| `q`, `Ctrl-c`        | Quit                                  |

The viewer soft-wraps and syntax-highlights Markdown without modifying note
content. Note folders and notes can also be activated with the left mouse
button, and the mouse wheel scrolls lists or the viewer. The note list follows
QOwnNotes' configured alphabetical or last-change sorting. The application does
not modify notes in this milestone.

## Development

Run `just` to list recipes. The main validation command is `just check`; use
`just nix-build` for the reproducible package and `just flake-check` to validate
all flake outputs.

Rust 1.85 is the minimum supported version. The crate uses Rust edition 2024.

## License

Copyright (C) 2026 qownnotes-tui contributors. Licensed under GPL-2.0-only. See
[`LICENSE`](LICENSE).
