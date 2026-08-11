use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::{
    config::{self, Config, NoteFolder, NoteSort},
    error::NoteReadError,
    event::{Event, Events, ScanResult},
    notes::{model::Note, scan},
    terminal::TerminalGuard,
    ui,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pane {
    Folders,
    Notes,
    Viewer,
}

pub struct App {
    pub note_folders: Vec<NoteFolder>,
    pub selected_folder: usize,
    pub active_folder: usize,
    pub note_sort: NoteSort,
    pub notes: Vec<Note>,
    pub selected_note: usize,
    pub pane: Pane,
    pub content: String,
    pub current_note: Option<PathBuf>,
    pub editing: bool,
    pub editor_cursor: usize,
    pub editor_scroll: u16,
    pub editor_horizontal_scroll: u16,
    pub viewer_scroll: u16,
    pub viewer_max_scroll: u16,
    pub viewer_page_size: u16,
    pub status: String,
    pub loading: bool,
    pub show_help: bool,
    pub show_settings: bool,
    pub settings_interval: String,
    pub folder_area: Rect,
    pub notes_area: Rect,
    pub viewer_area: Rect,
    pub folder_list_offset: usize,
    pub note_list_offset: usize,
    persisted_content: String,
    dirty: bool,
    dirty_since: Option<Instant>,
    external_conflict: bool,
    save_interval: Duration,
    should_quit: bool,
}

impl App {
    fn new(config: Config) -> Self {
        Self {
            selected_folder: config.active_folder,
            active_folder: config.active_folder,
            note_sort: config.note_sort,
            note_folders: config.note_folders,
            notes: Vec::new(),
            selected_note: 0,
            pane: Pane::Notes,
            content: String::new(),
            current_note: None,
            editing: false,
            editor_cursor: 0,
            editor_scroll: 0,
            editor_horizontal_scroll: 0,
            viewer_scroll: 0,
            viewer_max_scroll: 0,
            viewer_page_size: 1,
            status: "Scanning notes...".into(),
            loading: true,
            show_help: false,
            show_settings: false,
            settings_interval: config.save_interval_seconds.to_string(),
            folder_area: Rect::default(),
            notes_area: Rect::default(),
            viewer_area: Rect::default(),
            folder_list_offset: 0,
            note_list_offset: 0,
            persisted_content: String::new(),
            dirty: false,
            dirty_since: None,
            external_conflict: false,
            save_interval: Duration::from_secs(config.save_interval_seconds.max(1)),
            should_quit: false,
        }
    }

    pub fn root(&self) -> &Path {
        &self.note_folders[self.active_folder].path
    }

    fn scan_finished(&mut self, folder: usize, result: Result<Vec<Note>, String>) {
        if folder != self.active_folder {
            return;
        }
        self.loading = false;
        match result {
            Ok(mut notes) => {
                sort_notes(&mut notes, self.note_sort);
                self.notes = notes;
                self.selected_note = 0;
                self.status = format!(
                    "{}: {} notes",
                    self.note_folders[folder].name,
                    self.notes.len()
                );
                if !self.notes.is_empty() {
                    self.load_selected_note();
                }
            }
            Err(error) => self.status = error,
        }
    }

    fn handle_key(&mut self, key: KeyEvent, scans: &Sender<ScanResult>) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if self.show_help {
            self.show_help = false;
            return;
        }
        if self.show_settings {
            self.handle_settings_key(key);
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.save_note();
            self.should_quit = !self.dirty;
            return;
        }
        if self.editing {
            self.handle_editor_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => {
                self.save_note();
                self.should_quit = true;
            }
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('s') => self.show_settings = true,
            KeyCode::Char('e') if self.pane == Pane::Viewer && self.current_note.is_some() => {
                self.editing = true;
                self.editor_cursor = self.content.len();
                self.editor_scroll = 0;
                self.editor_horizontal_scroll = 0;
                self.status = "Editing note".into();
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::PageDown => self.scroll_viewer(self.viewer_page_size as isize),
            KeyCode::PageUp => self.scroll_viewer(-(self.viewer_page_size as isize)),
            KeyCode::Home if self.pane == Pane::Viewer => self.viewer_scroll = 0,
            KeyCode::End if self.pane == Pane::Viewer => {
                self.viewer_scroll = self.viewer_max_scroll;
            }
            KeyCode::Char('h') | KeyCode::Left => self.previous_pane(),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => self.next_pane(),
            KeyCode::Enter => self.open_selection(scans),
            KeyCode::Char('R') => self.start_scan(scans),
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.save_note();
            return;
        }
        if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.reload_note();
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.save_note();
                if !self.dirty {
                    self.editing = false;
                    self.status = self
                        .current_note
                        .as_ref()
                        .map_or_else(String::new, |path| path.display().to_string());
                }
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.content.insert(self.editor_cursor, character);
                self.editor_cursor += character.len_utf8();
                self.mark_dirty();
            }
            KeyCode::Enter => {
                self.content.insert(self.editor_cursor, '\n');
                self.editor_cursor += 1;
                self.mark_dirty();
            }
            KeyCode::Tab => {
                self.content.insert_str(self.editor_cursor, "    ");
                self.editor_cursor += 4;
                self.mark_dirty();
            }
            KeyCode::Backspace if self.editor_cursor > 0 => {
                let previous = previous_boundary(&self.content, self.editor_cursor);
                self.content.drain(previous..self.editor_cursor);
                self.editor_cursor = previous;
                self.mark_dirty();
            }
            KeyCode::Delete if self.editor_cursor < self.content.len() => {
                let next = next_boundary(&self.content, self.editor_cursor);
                self.content.drain(self.editor_cursor..next);
                self.mark_dirty();
            }
            KeyCode::Left => {
                self.editor_cursor = previous_boundary(&self.content, self.editor_cursor)
            }
            KeyCode::Right => self.editor_cursor = next_boundary(&self.content, self.editor_cursor),
            KeyCode::Up => self.move_editor_vertical(-1),
            KeyCode::Down => self.move_editor_vertical(1),
            KeyCode::Home => {
                self.editor_cursor = self.content[..self.editor_cursor]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
            }
            KeyCode::End => {
                self.editor_cursor += self.content[self.editor_cursor..]
                    .find('\n')
                    .unwrap_or(self.content.len() - self.editor_cursor);
            }
            _ => {}
        }
    }

    fn handle_settings_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.show_settings = false;
                self.settings_interval = self.save_interval.as_secs().to_string();
            }
            KeyCode::Enter => {
                let Some(value) = self
                    .settings_interval
                    .parse::<u64>()
                    .ok()
                    .filter(|value| *value > 0)
                else {
                    self.status = "Save interval must be at least 1 second".into();
                    return;
                };
                match config::save_interval_seconds(value) {
                    Ok(()) => {
                        self.save_interval = Duration::from_secs(value);
                        self.show_settings = false;
                        self.status = format!("Save interval set to {value} seconds");
                    }
                    Err(error) => self.status = format!("Unable to save settings: {error:#}"),
                }
            }
            KeyCode::Char(character) if character.is_ascii_digit() => {
                if self.settings_interval == "0" {
                    self.settings_interval.clear();
                }
                self.settings_interval.push(character);
            }
            KeyCode::Backspace => {
                self.settings_interval.pop();
            }
            KeyCode::Up => self.adjust_interval(1),
            KeyCode::Down => self.adjust_interval(-1),
            _ => {}
        }
    }

    fn adjust_interval(&mut self, delta: i64) {
        let current = self.settings_interval.parse::<u64>().unwrap_or(1);
        self.settings_interval = if delta > 0 {
            current.saturating_add(delta as u64)
        } else {
            current.saturating_sub(delta.unsigned_abs()).max(1)
        }
        .to_string();
    }

    fn mark_dirty(&mut self) {
        self.dirty = self.content != self.persisted_content;
        if self.dirty {
            self.dirty_since.get_or_insert_with(Instant::now);
            self.status = "Modified".into();
        } else {
            self.dirty_since = None;
        }
    }

    fn move_editor_vertical(&mut self, delta: isize) {
        let before = &self.content[..self.editor_cursor];
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        let column = self.content[line_start..self.editor_cursor].chars().count();
        let target_start = if delta < 0 {
            if line_start == 0 {
                return;
            }
            self.content[..line_start - 1]
                .rfind('\n')
                .map_or(0, |index| index + 1)
        } else {
            let Some(next) = self.content[self.editor_cursor..].find('\n') else {
                return;
            };
            self.editor_cursor + next + 1
        };
        let target_end = self.content[target_start..]
            .find('\n')
            .map_or(self.content.len(), |index| target_start + index);
        self.editor_cursor = self.content[target_start..target_end]
            .char_indices()
            .nth(column)
            .map_or(target_end, |(offset, _)| target_start + offset);
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, scans: &Sender<ScanResult>) {
        if self.editing || self.show_settings {
            return;
        }
        if self.show_help {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                self.show_help = false;
            }
            return;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(index) = list_item_at(
                    self.folder_area,
                    mouse.column,
                    mouse.row,
                    self.folder_list_offset,
                    self.note_folders.len(),
                ) {
                    self.selected_folder = index;
                    self.pane = Pane::Folders;
                    self.open_selection(scans);
                } else if let Some(index) = list_item_at(
                    self.notes_area,
                    mouse.column,
                    mouse.row,
                    self.note_list_offset,
                    self.notes.len(),
                ) {
                    self.selected_note = index;
                    self.pane = Pane::Notes;
                    self.load_selected_note();
                }
            }
            MouseEventKind::ScrollUp => self.handle_mouse_scroll(mouse, -3),
            MouseEventKind::ScrollDown => self.handle_mouse_scroll(mouse, 3),
            _ => {}
        }
    }

    fn handle_mouse_scroll(&mut self, mouse: MouseEvent, delta: isize) {
        if self.viewer_area.contains((mouse.column, mouse.row).into()) {
            self.pane = Pane::Viewer;
            self.scroll_viewer(delta);
        } else if self.notes_area.contains((mouse.column, mouse.row).into()) {
            self.pane = Pane::Notes;
            self.move_selection(delta);
        } else if self.folder_area.contains((mouse.column, mouse.row).into()) {
            self.pane = Pane::Folders;
            self.move_selection(delta);
        }
    }

    fn move_selection(&mut self, delta: isize) {
        match self.pane {
            Pane::Folders => {
                self.selected_folder =
                    move_index(self.selected_folder, self.note_folders.len(), delta);
            }
            Pane::Notes => {
                self.selected_note = move_index(self.selected_note, self.notes.len(), delta);
                self.load_selected_note();
            }
            Pane::Viewer => self.scroll_viewer(delta),
        }
    }

    fn scroll_viewer(&mut self, delta: isize) {
        self.viewer_scroll = self
            .viewer_scroll
            .saturating_add_signed(delta.clamp(i16::MIN as isize, i16::MAX as isize) as i16)
            .min(self.viewer_max_scroll);
    }

    fn open_selection(&mut self, scans: &Sender<ScanResult>) {
        if self.pane == Pane::Folders {
            if self.selected_folder != self.active_folder {
                self.active_folder = self.selected_folder;
                self.notes.clear();
                self.content.clear();
                self.current_note = None;
                self.viewer_scroll = 0;
                self.start_scan(scans);
                if let Err(error) = config::selected_note_folder(self.root()) {
                    self.status = format!("Unable to remember note folder: {error:#}");
                }
            }
            self.pane = Pane::Notes;
            return;
        }
        if self.pane != Pane::Notes {
            return;
        }
        if self.load_selected_note() {
            self.pane = Pane::Viewer;
        }
    }

    fn load_selected_note(&mut self) -> bool {
        let path = self
            .notes
            .get(self.selected_note)
            .map(|note| note.relative_path.clone());
        if let Some(path) = path {
            match read_note(self.root(), &path) {
                Ok(content) => {
                    self.content = content.clone();
                    self.persisted_content = content;
                    self.current_note = Some(path.clone());
                    self.dirty = false;
                    self.dirty_since = None;
                    self.external_conflict = false;
                    self.viewer_scroll = 0;
                    self.status = path.display().to_string();
                    return true;
                }
                Err(error) => {
                    self.content.clear();
                    self.current_note = None;
                    self.status = error.to_string();
                }
            }
        }
        false
    }

    fn start_scan(&mut self, scans: &Sender<ScanResult>) {
        self.loading = true;
        self.status = format!("Scanning {}...", self.note_folders[self.active_folder].name);
        spawn_scan(self.active_folder, self.root().to_path_buf(), scans.clone());
    }

    fn tick(&mut self) {
        self.check_external_change();
        if self.dirty
            && self
                .dirty_since
                .is_some_and(|since| since.elapsed() >= self.save_interval)
        {
            self.save_note();
        }
    }

    fn check_external_change(&mut self) {
        let Some(relative) = self.current_note.clone() else {
            return;
        };
        match read_note(self.root(), &relative) {
            Ok(disk_content) if disk_content != self.persisted_content => {
                if self.dirty {
                    self.external_conflict = true;
                    self.status =
                        "Note changed outside the app; Esc cannot save until resolved".into();
                } else {
                    self.content = disk_content.clone();
                    self.persisted_content = disk_content;
                    self.editor_cursor = self.editor_cursor.min(self.content.len());
                    while !self.content.is_char_boundary(self.editor_cursor) {
                        self.editor_cursor -= 1;
                    }
                    self.viewer_scroll = 0;
                    self.status = format!("Reloaded {} after external change", relative.display());
                }
            }
            Ok(_) => self.external_conflict = false,
            Err(error) => {
                self.external_conflict = true;
                self.status = format!("Note changed outside the app: {error}");
            }
        }
    }

    fn save_note(&mut self) {
        if !self.dirty {
            return;
        }
        self.check_external_change();
        if self.external_conflict {
            return;
        }
        let Some(relative) = self.current_note.clone() else {
            return;
        };
        let path = self.root().join(&relative);
        match fs::write(&path, self.content.as_bytes()) {
            Ok(()) => {
                self.persisted_content.clone_from(&self.content);
                self.dirty = false;
                self.dirty_since = None;
                self.status = format!("Saved {}", relative.display());
            }
            Err(error) => self.status = format!("Unable to save {}: {error}", relative.display()),
        }
    }

    fn reload_note(&mut self) {
        let Some(relative) = self.current_note.clone() else {
            return;
        };
        match read_note(self.root(), &relative) {
            Ok(content) => {
                self.content = content.clone();
                self.persisted_content = content;
                self.editor_cursor = self.content.len();
                self.dirty = false;
                self.dirty_since = None;
                self.external_conflict = false;
                self.status = format!("Reloaded {}; local edits discarded", relative.display());
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn next_pane(&mut self) {
        self.pane = match self.pane {
            Pane::Folders => Pane::Notes,
            Pane::Notes => Pane::Viewer,
            Pane::Viewer => Pane::Folders,
        };
    }

    fn previous_pane(&mut self) {
        self.pane = match self.pane {
            Pane::Folders => Pane::Viewer,
            Pane::Notes => Pane::Folders,
            Pane::Viewer => Pane::Notes,
        };
    }
}

pub fn run(mut terminal: TerminalGuard, config: Config) -> anyhow::Result<()> {
    let (scan_tx, scan_rx) = mpsc::channel();
    let mut app = App::new(config);
    app.start_scan(&scan_tx);
    let events = Events::new(scan_rx);

    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        match events.next()? {
            Event::Key(key) => app.handle_key(key, &scan_tx),
            Event::Mouse(mouse) => app.handle_mouse(mouse, &scan_tx),
            Event::ScanFinished((folder, result)) => app.scan_finished(folder, result),
            Event::Tick => app.tick(),
            Event::Resize => {}
        }
    }
    Ok(())
}

fn spawn_scan(folder: usize, root: PathBuf, sender: Sender<ScanResult>) {
    thread::spawn(move || {
        let result = scan::scan(&root).map_err(|error| format!("Scan failed: {error:#}"));
        let _ = sender.send((folder, result));
    });
}

fn read_note(root: &Path, relative: &Path) -> Result<String, NoteReadError> {
    let path = root.join(relative);
    let bytes = fs::read(&path)?;
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec())
        .map_err(|_| NoteReadError::InvalidUtf8(relative.to_path_buf()))
}

fn previous_boundary(content: &str, cursor: usize) -> usize {
    content[..cursor]
        .char_indices()
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn next_boundary(content: &str, cursor: usize) -> usize {
    content[cursor..]
        .chars()
        .next()
        .map_or(cursor, |character| cursor + character.len_utf8())
}

fn move_index(current: usize, count: usize, delta: isize) -> usize {
    if count == 0 {
        return 0;
    }
    current.saturating_add_signed(delta).min(count - 1)
}

fn list_item_at(area: Rect, column: u16, row: u16, offset: usize, count: usize) -> Option<usize> {
    let inside = column > area.x
        && column < area.right().saturating_sub(1)
        && row > area.y
        && row < area.bottom().saturating_sub(1);
    if !inside {
        return None;
    }
    let index = offset + usize::from(row - area.y - 1);
    (index < count).then_some(index)
}

fn sort_notes(notes: &mut [Note], sort: NoteSort) {
    notes.sort_by(|left, right| match sort {
        NoteSort::LastModified => right
            .modified
            .cmp(&left.modified)
            .then_with(|| left.relative_path.cmp(&right.relative_path)),
        NoteSort::Alphabetical { descending } => {
            let left_name = left.name();
            let right_name = right.name();
            let order = left_name
                .to_lowercase()
                .cmp(&right_name.to_lowercase())
                .then_with(|| left.relative_path.cmp(&right.relative_path));
            if descending { order.reverse() } else { order }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        sync::mpsc,
        time::{Duration, SystemTime},
    };

    use crate::{
        config::{Config, NoteFolder, NoteSort},
        notes::model::Note,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;

    use super::{App, Pane, list_item_at, move_index, sort_notes};

    #[test]
    fn selection_stays_in_bounds() {
        assert_eq!(move_index(0, 0, 1), 0);
        assert_eq!(move_index(0, 3, -1), 0);
        assert_eq!(move_index(2, 3, 1), 2);
        assert_eq!(move_index(1, 3, 1), 2);
    }

    #[test]
    fn viewer_scrolling_stays_in_bounds() {
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: "/notes".into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
        });
        app.viewer_max_scroll = 10;

        app.scroll_viewer(4);
        assert_eq!(app.viewer_scroll, 4);
        app.scroll_viewer(20);
        assert_eq!(app.viewer_scroll, 10);
        app.scroll_viewer(-20);
        assert_eq!(app.viewer_scroll, 0);
    }

    #[test]
    fn navigating_notes_loads_content_without_moving_focus() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("first.md"), "first").unwrap();
        fs::write(root.path().join("second.md"), "second").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
        });
        app.notes = vec![
            Note {
                relative_path: "first.md".into(),
                size: 5,
                modified: SystemTime::UNIX_EPOCH,
            },
            Note {
                relative_path: "second.md".into(),
                size: 6,
                modified: SystemTime::UNIX_EPOCH,
            },
        ];

        app.move_selection(1);

        assert_eq!(app.selected_note, 1);
        assert_eq!(app.content, "second");
        assert_eq!(app.current_note.as_deref(), Some(Path::new("second.md")));
        assert_eq!(app.pane, Pane::Notes);
    }

    #[test]
    fn completed_scan_loads_the_first_note_into_the_preview() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("note.md"), "previewed").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
        });
        let notes = vec![Note {
            relative_path: "note.md".into(),
            size: 9,
            modified: SystemTime::UNIX_EPOCH,
        }];

        app.scan_finished(0, Ok(notes));

        assert_eq!(app.content, "previewed");
        assert_eq!(app.current_note.as_deref(), Some(Path::new("note.md")));
        assert_eq!(app.pane, Pane::Notes);
    }

    #[test]
    fn enter_on_note_moves_focus_to_viewer() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("note.md"), "content").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 7,
            modified: SystemTime::UNIX_EPOCH,
        });
        let (scan_tx, _scan_rx) = mpsc::channel();

        app.open_selection(&scan_tx);

        assert_eq!(app.content, "content");
        assert_eq!(app.pane, Pane::Viewer);
    }

    #[test]
    fn editor_changes_and_saves_a_note() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("note.md"), "content").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 7,
            modified: SystemTime::UNIX_EPOCH,
        });
        app.load_selected_note();
        app.editing = true;
        app.editor_cursor = app.content.len();

        app.handle_editor_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));
        app.save_note();

        assert_eq!(
            fs::read_to_string(root.path().join("note.md")).unwrap(),
            "content!"
        );
        assert!(!app.dirty);
    }

    #[test]
    fn reloads_an_unmodified_note_changed_outside_the_app() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("note.md"), "original").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 8,
            modified: SystemTime::UNIX_EPOCH,
        });
        app.load_selected_note();
        fs::write(root.path().join("note.md"), "external").unwrap();

        app.check_external_change();

        assert_eq!(app.content, "external");
        assert!(!app.external_conflict);
    }

    #[test]
    fn does_not_overwrite_an_external_change_with_dirty_content() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("note.md"), "original").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 8,
            modified: SystemTime::UNIX_EPOCH,
        });
        app.load_selected_note();
        app.content.push_str(" local");
        app.mark_dirty();
        fs::write(root.path().join("note.md"), "external").unwrap();

        app.save_note();

        assert_eq!(
            fs::read_to_string(root.path().join("note.md")).unwrap(),
            "external"
        );
        assert!(app.dirty);
        assert!(app.external_conflict);
    }

    #[test]
    fn maps_mouse_coordinates_to_visible_list_items() {
        let area = Rect::new(10, 5, 20, 6);
        assert_eq!(list_item_at(area, 11, 6, 3, 10), Some(3));
        assert_eq!(list_item_at(area, 11, 9, 3, 10), Some(6));
        assert_eq!(list_item_at(area, 10, 6, 3, 10), None);
        assert_eq!(list_item_at(area, 11, 10, 3, 10), None);
    }

    #[test]
    fn sorts_notes_like_qownnotes() {
        let now = SystemTime::UNIX_EPOCH;
        let mut notes = vec![
            Note {
                relative_path: "zebra.md".into(),
                size: 0,
                modified: now,
            },
            Note {
                relative_path: "Alpha.md".into(),
                size: 0,
                modified: now + Duration::from_secs(1),
            },
        ];
        sort_notes(&mut notes, NoteSort::LastModified);
        assert_eq!(notes[0].relative_path, Path::new("Alpha.md"));
        sort_notes(&mut notes, NoteSort::Alphabetical { descending: true });
        assert_eq!(notes[0].relative_path, Path::new("zebra.md"));
    }
}
