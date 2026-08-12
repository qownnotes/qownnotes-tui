# qownnotes-tui Project Plan

## 1. Project Summary

`qownnotes-tui` is a standalone Rust terminal application for browsing, searching,
creating, and editing QOwnNotes-compatible Markdown notes. It operates directly
on a local note folder and does not require the QOwnNotes desktop application to
be installed or running.

The note files are the source of truth. The application will keep its own
rebuildable index and will not depend on QOwnNotes runtime note IDs or its global
application database.

The binary, package, and repository name are all `qownnotes-tui`.

## 2. Goals

- Provide a fast, keyboard-first terminal note editor.
- Read and write the same Markdown files used by QOwnNotes.
- Support nested note folders, search, links, backlinks, and attachments.
- Detect external changes and prevent silent overwrites.
- Preserve files faithfully when they are opened and saved.
- Read QOwnNotes tags and eventually write them safely.
- Support the current QOwnNotes encrypted-note format in a later milestone.
- Work on Linux, macOS, and Windows terminals.
- Remain useful without Nextcloud, ownCloud, Qt, or a running QOwnNotes process.

## 3. Non-Goals

- Connecting to the QOwnNotes MCP server.
- Implementing CalDAV todos or the QOwnNotes todo dialog.
- Linking QOwnNotes C++ or Qt code through FFI.
- Reproducing the QOwnNotes graphical interface.
- Managing QOwnNotes scripts or providing QML compatibility.
- Writing the global `QOwnNotes.sqlite` application database.
- Implementing a cloud synchronization client. Existing filesystem sync tools
  remain responsible for synchronization.
- Rendering every Markdown or HTML feature in the first release.

## 4. Licensing Strategy

The initial implementation should be original Rust code based on public file
formats and observable behavior. Do not translate or copy QOwnNotes C++ code
without first deciding to distribute the affected implementation under terms
compatible with QOwnNotes' GPL-2.0-only license.

Dependency licenses must be recorded and checked before the first release. Keep
the application license decision explicit in `Cargo.toml` and add a repository
`LICENSE` before implementation begins. GPL-2.0-only is the simplest choice if
exact translations of QOwnNotes behavior are later needed; otherwise a clean
implementation can use a separately chosen license after review.

## 5. Compatibility Contract

### 5.1 Source of Truth

- Note content lives in ordinary UTF-8 Markdown or text files.
- Stable note identity is the normalized path relative to the selected note
  root, never an in-memory or database ID.
- The index is disposable and must be rebuildable from the filesystem.
- `QOwnNotes.sqlite` is application-private and must never be modified.
- `<note-root>/notes.sqlite` is initially read-only.

### 5.2 Folder Rules

- Recursively scan configured note extensions, initially `md`, `txt`, and
  `markdown`.
- Exclude `.git`, `media`, `attachments`, and `trash` from note discovery by
  default.
- Show `media` and `attachments` through attachment-specific UI rather than as
  note folders.
- Preserve relative links when possible.
- Match QOwnNotes' default behavior by deriving note filenames from the first
  meaningful content line on save. Explicit rename support and link-update
  previews remain separate follow-up work.

### 5.3 File Fidelity

- Detect and preserve UTF-8 BOM state.
- Detect and preserve `LF` or `CRLF` line endings.
- Preserve whether the file ends with a newline.
- Refuse to modify files that are not valid UTF-8 in the MVP; permit read-only
  viewing with an explanatory error.
- Avoid content normalization unless the user invokes formatting explicitly.
- Preserve filesystem permissions where supported.

### 5.4 Concurrent Editing

Every opened buffer records the content hash and relevant filesystem metadata
from the last successful read. Before saving, the application must verify that
the on-disk file still matches that baseline.

If the file changed externally, offer these actions:

1. View the diff.
2. Reload and discard the local buffer.
3. Save the local buffer to a conflict copy.
4. Overwrite only after explicit confirmation.

Normal saves use a temporary file in the same directory followed by an atomic
replace where the platform supports it. No save may silently overwrite an
externally modified note.

## 6. User Experience

### 6.1 Default Layout

Use a three-pane layout on sufficiently wide terminals:

```text
+------------------+---------------------------+---------------------------+
| Folders / Filters| Notes                     | Editor / Preview          |
|                  |                           |                           |
|                  |                           |                           |
+------------------+---------------------------+---------------------------+
| Mode | note path | modified/conflict state | status message             |
+------------------------------------------------------------------------+
```

On narrow terminals, show one primary pane at a time and switch panes with a
stable keybinding. The editor always gets the largest available area.

### 6.2 Interaction Model

- Provide Normal and Insert modes with a small, documented command set.
- Do not claim full Vim compatibility.
- Support conventional terminal shortcuts where they do not conflict with
  terminal behavior.
- Make every destructive action confirmable and cancellable.
- Provide a command palette for discoverability.
- Show pending filesystem conflicts persistently until resolved.
- Keep mouse support optional and disabled by configuration.

Initial keybindings:

| Key       | Action                                               |
| --------- | ---------------------------------------------------- |
| `j` / `k` | Move selection in Normal mode                        |
| `h` / `l` | Move between panes in Normal mode                    |
| `Enter`   | Open selected folder, note, or link                  |
| `i`       | Enter Insert mode                                    |
| `Esc`     | Return to Normal mode or close a transient view      |
| `Ctrl-s`  | Save the current note                                |
| `Ctrl-p`  | Open command palette                                 |
| `/`       | Search notes                                         |
| `n`       | Create a note                                        |
| `r`       | Rename selected note with preview                    |
| `d`       | Move selected note to local trash after confirmation |
| `Tab`     | Cycle panes                                          |
| `q`       | Quit when no unsaved buffers require a decision      |

Keybindings must be configurable after the MVP.

### 6.3 Editor Requirements

- Use a rope-backed text buffer so large notes do not require repeated full
  string copies.
- Support multiline selection, clipboard operations, undo, and redo.
- Keep undo history per open buffer.
- Handle Unicode grapheme widths correctly in cursor placement and selection.
- Support bracket matching and Markdown-aware syntax highlighting.
- Provide optional soft wrapping independent of file content.
- Keep editor state separate from rendering so it can be unit tested.
- Autosave is off by default. A configurable idle autosave can be added only
  after conflict handling is proven reliable.

## 7. Proposed Architecture

Use a single Cargo package initially. Split into workspace crates only when a
real reusable boundary emerges.

```text
src/
  main.rs             process startup and terminal lifecycle
  cli.rs              command-line parsing
  config.rs           configuration loading and validation
  app.rs              application state and event dispatch
  action.rs           semantic actions produced by input and background work
  event.rs            terminal, timer, watcher, and worker events
  ui/
    mod.rs            layout and top-level drawing
    folders.rs        folder/filter pane
    notes.rs          note list pane
    editor.rs         editor rendering
    preview.rs        Markdown preview
    overlays.rs       search, palette, dialogs, and diff views
  editor/
    mod.rs            editor state machine
    buffer.rs         rope, selections, and edit operations
    history.rs        undo and redo transactions
    motion.rs         cursor and selection movement
  notes/
    mod.rs            note repository interface
    model.rs          stable note and file metadata types
    scan.rs           note-folder discovery
    store.rs          guarded reads and atomic writes
    naming.rs         create, rename, and collision policy
    watch.rs          external filesystem event normalization
  search/
    mod.rs            query API
    query.rs          query parser
    index.rs          rebuildable content index
  markdown/
    mod.rs            parsed document model
    links.rs          Markdown and wiki links
    backlinks.rs      reverse-link index
    highlight.rs      syntax spans
  tags/
    mod.rs            tag domain model
    qownnotes.rs      guarded `notes.sqlite` compatibility layer
  crypto/
    mod.rs            encrypted-note detection and interface
    qownnotes_v2.rs   compatible current-format implementation
  error.rs            user-facing and diagnostic errors
```

### 7.1 State and Event Flow

- Keep `App` as the single owner of user-visible state.
- Translate terminal input into semantic `Action` values before mutating state.
- Run directory scans, indexing, filesystem watching, and expensive Markdown
  parsing outside the draw path.
- Send background results to the application through bounded channels.
- Coalesce duplicate filesystem events before triggering reads or rescans.
- Never perform disk or database I/O while rendering a frame.

An async runtime is not required solely for terminal input. Prefer threads and
channels for the initial filesystem workloads unless selected dependencies make
an async runtime materially simpler.

### 7.2 Initial Dependency Candidates

Confirm current maintenance and licenses before adding dependencies.

| Area               | Candidate                                                     |
| ------------------ | ------------------------------------------------------------- |
| TUI                | `ratatui`                                                     |
| Terminal backend   | `crossterm`                                                   |
| CLI                | `clap`                                                        |
| Configuration      | `serde`, `toml`, `directories`                                |
| Text buffer        | `ropey`                                                       |
| Unicode            | `unicode-segmentation`, `unicode-width`                       |
| Filesystem walking | `ignore` or `walkdir`                                         |
| Filesystem events  | `notify`                                                      |
| Content hashing    | `blake3`                                                      |
| SQLite tags        | `rusqlite` with a controlled SQLite feature policy            |
| Markdown           | `pulldown-cmark`                                              |
| Highlighting       | `syntect` or a smaller Markdown-specific highlighter          |
| Diff view          | `similar`                                                     |
| Errors             | `thiserror`, optionally `anyhow` at the binary boundary       |
| Logging            | `tracing`, `tracing-subscriber`                               |
| Secret handling    | `secrecy`, `zeroize`                                          |
| Crypto             | RustCrypto crates selected for QOwnNotes format compatibility |

Avoid adding an embedded editor widget until its selection, undo, Unicode,
and conflict behavior have been evaluated against the editor requirements.

## 8. Configuration and CLI

The first release should require an explicit note root or use a configured
default. Automatic parsing of platform-specific QOwnNotes settings is a later
convenience feature, not a core dependency.

Proposed commands and options:

```text
qownnotes-tui [--notes-dir PATH]
qownnotes-tui open PATH
qownnotes-tui doctor [--notes-dir PATH]
qownnotes-tui index rebuild [--notes-dir PATH]
qownnotes-tui --version
```

Configuration precedence, highest first:

1. Command-line options.
2. Environment variables prefixed with `QOWNNOTES_TUI_`.
3. User configuration file.
4. Built-in defaults.

The configuration file should contain only user preferences. Indexes, logs,
and other cache data belong in platform-specific cache or state directories.
No application-generated files should be placed in the note root except where
QOwnNotes compatibility explicitly requires them.

## 9. Search and Navigation

### 9.1 Initial Search

- Search note title, relative path, and body.
- Use case-insensitive matching by default.
- Support quoted phrases and multiple AND terms.
- Add `name:` and `path:` field prefixes.
- Show context snippets and match highlighting.
- Keep search responsive while the query changes.

Start with an in-memory index or parallel scan suitable for normal personal note
collections. Introduce an on-disk full-text index only after profiling shows a
need. Any on-disk index must be disposable and versioned.

### 9.2 Links

Support these link forms:

- Relative Markdown links to note files.
- Wiki links such as `[[Note name]]`.
- Wiki links with subfolder paths, headings, and display text.
- Heading anchors inside the current or another note.
- Relative links to files under `media` and `attachments`.

Ambiguous wiki links should open a chooser rather than selecting arbitrarily.
Backlinks are derived from the current index and never stored in note files.

## 10. Tags

### 10.1 Read-Only Tag Support

Open `<note-root>/notes.sqlite` read-only and map tags using the note filename
and subfolder path. If the database is absent, locked, newer than understood, or
malformed, note editing must continue without tags.

The compatibility layer must:

- Validate expected tables and columns before querying.
- Avoid migrations in read-only mode.
- Avoid depending on numeric note IDs.
- Surface database errors without terminating the application.
- Include fixture-based tests for supported schema variants.

### 10.2 Tag Writes

Tag writes are deferred until file editing, watching, and read-only tags are
stable. Before enabling writes:

- Document the supported `notes.sqlite` schema versions.
- Use short transactions and a busy timeout.
- Detect a concurrently active writer and fail safely.
- Back up the database before the first write by a new application version.
- Test changes by reopening the folder in QOwnNotes.
- Provide a configuration switch that leaves tags permanently read-only.

## 11. Encrypted Notes

Encrypted notes must be detected before the editor treats their body as normal
Markdown. Until encryption support is complete, they are read-only and clearly
marked.

The first supported encrypted format is QOwnNotes version 2:

- PBKDF2-HMAC-SHA1 with the iteration count and salt stored in the note.
- AES-256-CBC with PKCS#7 padding.
- HMAC-SHA1 authentication.
- The existing QOwnNotes encrypted-text envelope and metadata fields.

Implementation requirements:

- Authenticate ciphertext before exposing plaintext.
- Keep passwords and derived keys out of logs and error messages.
- Zeroize sensitive buffers where practical.
- Do not put passwords in process arguments or environment variables.
- Prompt through the terminal and optionally integrate with a platform keyring
  only as a separate, opt-in feature.
- Validate against fixtures produced by QOwnNotes without committing secrets.
- Never overwrite an encrypted note if decryption or authentication fails.
- Leave legacy QOwnNotes encryption formats out of scope until version 2 is
  complete and interoperable.

## 12. Milestones

### Milestone 0: Repository Foundation

Deliverables:

- Choose and add the project license.
- Initialize a Rust package whose binary is `qownnotes-tui`.
- Add formatting, linting, test, and dependency-audit commands.
- Add CI for Linux, macOS, and Windows.
- Establish an MSRV policy and record the Rust edition.
- Add structured logging to a platform state directory.
- Add a crash-safe terminal guard that always restores terminal mode.

Exit criteria:

- `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and
  `cargo test --all-targets` pass in CI.
- Starting and quitting restores the terminal after normal exit and panic.

### Milestone 1: Read-Only Note Browser

Deliverables:

- Parse CLI and configuration.
- Validate and scan a note root recursively.
- Exclude reserved and configured paths.
- Display folder and note panes with responsive narrow-terminal behavior.
- Open notes in a read-only viewer.
- Display file metadata and clear read errors.

Exit criteria:

- A representative QOwnNotes folder can be browsed without modifying it.
- Relative-path identities remain stable across rescans.
- A large fixture tree does not block terminal input during scanning.

### Milestone 2: Core Editor and Safe Saves

Deliverables:

- Rope-backed editor with Normal and Insert modes.
- Unicode-aware cursor movement and soft wrapping.
- Selection, clipboard, undo, and redo.
- Create, edit, save, save-as, and guarded delete-to-trash operations.
- File-fidelity tracking and atomic replacement.
- Hash-based external modification checks and conflict UI.

Exit criteria:

- Round-trip tests preserve BOM, line endings, and trailing-newline state.
- Concurrent external edits cannot be overwritten without confirmation.
- Saving and reopening in QOwnNotes preserves the expected note content.
- Unsaved buffers cannot be lost through normal quit flows.

### Milestone 3: Search, Markdown, and Links

Deliverables:

- Responsive full-note search with query parsing.
- Markdown syntax highlighting and preview.
- Markdown-link and wiki-link resolution.
- Heading navigation.
- Backlink index and backlink panel.
- Attachment and media link opening through a configurable system opener.

Exit criteria:

- Search remains interactive on a documented benchmark corpus.
- Link and backlink fixtures cover ambiguous names and nested folders.
- Missing or unsafe external links produce a prompt or error rather than an
  uncontrolled launch.

### Milestone 4: Filesystem Watching

Deliverables:

- Cross-platform watcher integration.
- Event debouncing and rename normalization.
- Background incremental index updates.
- Reload prompts for clean buffers and conflict state for dirty buffers.
- Recovery from watcher overflow by performing a complete rescan.

Exit criteria:

- Create, modify, rename, and delete operations from QOwnNotes are reflected
  without restarting the TUI.
- Dirty buffers survive external renames or changes without silent data loss.
- Bursts generated by synchronization clients are coalesced correctly in tests.

### Milestone 5: QOwnNotes Tags

Deliverables:

- Read-only `notes.sqlite` compatibility layer.
- Tag filters and note tag display.
- Hierarchical tag presentation.
- Documented schema validation and fixture tests.
- Optional writes only after a separate compatibility review.

Exit criteria:

- Tags created in QOwnNotes appear on the corresponding relative note paths.
- Missing, locked, or unsupported tag databases do not affect note editing.
- If writes are enabled, QOwnNotes can read all resulting tag relationships.

### Milestone 6: Encrypted Notes

Deliverables:

- Reliable encrypted-note detection.
- Interactive password prompt and in-memory unlocked state.
- QOwnNotes version-2 decryption and encryption.
- Interoperability fixtures and tamper tests.
- Clear read-only behavior for unsupported legacy formats.

Exit criteria:

- QOwnNotes can decrypt notes created by `qownnotes-tui` and vice versa.
- Modified metadata, nonce, MAC, or ciphertext is rejected safely.
- Passwords and plaintext do not appear in logs, panic output, or swap-like
  application cache files created by the program.

### Milestone 7: Release Hardening

Deliverables:

- Configurable keybindings and themes.
- `doctor` command for note-root and terminal diagnostics.
- Performance benchmarks and startup targets.
- Recovery files for unexpected shutdown with explicit privacy controls.
- Shell completions and manual page.
- Reproducible release artifacts and checksums.
- User documentation for coexistence with QOwnNotes and sync clients.

Exit criteria:

- No known data-loss bugs remain open.
- Cross-platform integration tests pass.
- Dependency licenses and security advisories are reviewed.
- A release candidate has been tested against real QOwnNotes note folders using
  backups.

## 13. Testing Strategy

### 13.1 Unit Tests

- Path normalization and stable identities.
- Extension and reserved-directory filtering.
- Encoding, BOM, newline, and trailing-newline round trips.
- Editor motions, selections, undo grouping, and Unicode width handling.
- Query parsing and match behavior.
- Markdown and wiki-link parsing and resolution.
- Conflict detection and save decision logic.
- Encrypted envelope parsing and cryptographic failure paths.

### 13.2 Integration Tests

- Scan temporary note trees containing nested folders and symlinks.
- Exercise create, rename, edit, trash, and conflict-copy workflows.
- Generate external filesystem changes while buffers are clean and dirty.
- Read versioned `notes.sqlite` fixtures.
- Verify encryption fixtures produced by QOwnNotes.
- Run terminal interaction tests through a pseudo-terminal where practical.

### 13.3 Property and Fuzz Tests

- Editing operations preserve rope and cursor invariants.
- Arbitrary file names cannot escape the configured note root.
- Markdown and encrypted-envelope parsers do not panic on arbitrary input.
- Save/load round trips preserve supported file representations.

### 13.4 Manual Compatibility Matrix

Before releases, test at least:

| Area          | Cases                                                  |
| ------------- | ------------------------------------------------------ |
| Platform      | Linux, macOS, Windows                                  |
| Terminal      | Common native terminal, tmux, SSH session              |
| QOwnNotes     | Current stable release and one prior supported release |
| Sync behavior | Local edits and a representative desktop sync client   |
| Folder shape  | Flat, deeply nested, spaces, non-ASCII names           |
| Files         | LF, CRLF, BOM, no final newline, large note            |
| Metadata      | No `notes.sqlite`, tags present, database locked       |

## 14. Security and Data-Safety Rules

- Resolve and validate paths before every mutation; never write outside the note
  root through `..`, malformed links, or symlink traversal.
- Do not follow directory symlinks by default during scanning.
- Treat note content, paths, passwords, and decrypted text as private data.
- Keep logs metadata-light and redact sensitive paths when practical.
- Do not execute commands or open links without a user action.
- Pass system-opener arguments without shell interpolation.
- Back up metadata before introducing tag writes or schema-sensitive changes.
- Prefer refusing an operation over guessing when compatibility is uncertain.

## 15. Performance Targets

Initial targets on a typical development machine:

- First usable frame within 150 ms for an already configured folder.
- Folder and title list available within 1 second for 10,000 notes.
- Search result updates within 50 ms after the index is ready.
- Editor input-to-frame latency below 16 ms under normal load.
- Opening a 1 MiB note without a visible multi-second stall.
- Incremental external-change processing without a full-tree rescan in the
  normal case.

These are targets, not assumptions. Add reproducible benchmarks before making
performance claims.

## 16. Open Decisions

Resolve these during Milestone 0 or the milestone that first needs them:

1. Project license and contribution policy.
2. Minimum supported Rust version.
3. Whether to build the editor primitives in-project or adopt and extend an
   existing Ratatui editor component.
4. Exact search index design after measuring real note collections.
5. Clipboard integration policy for local, SSH, Wayland, X11, macOS, and
   Windows environments.
6. Recovery-file encryption and retention policy.
7. Supported QOwnNotes `notes.sqlite` schema versions.
8. Whether tag writes belong in the first stable release.
9. Whether automatic QOwnNotes settings discovery is worth its platform-specific
   complexity.

## 17. First Implementation Slice

The first implementation pull request should be deliberately small:

1. Initialize the Rust package and CI.
2. Add `--notes-dir` and configuration-directory discovery.
3. Enter and reliably restore terminal raw mode.
4. Scan Markdown files in a worker and return stable relative-path records.
5. Render folder, note-list, and read-only note panes.
6. Support navigation, open, reload, help, and quit.
7. Add unit tests for scanning and path safety.

Do not include editing, SQLite, encryption, or filesystem watching in the first
slice. This establishes the application state model and terminal lifecycle
before any code can modify user data.
