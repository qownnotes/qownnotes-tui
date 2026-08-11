use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread,
};

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::{
    config::{Config, NoteFolder, NoteSort},
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
    pub viewer_scroll: u16,
    pub viewer_max_scroll: u16,
    pub viewer_page_size: u16,
    pub status: String,
    pub loading: bool,
    pub show_help: bool,
    pub folder_area: Rect,
    pub notes_area: Rect,
    pub viewer_area: Rect,
    pub folder_list_offset: usize,
    pub note_list_offset: usize,
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
            viewer_scroll: 0,
            viewer_max_scroll: 0,
            viewer_page_size: 1,
            status: "Scanning notes...".into(),
            loading: true,
            show_help: false,
            folder_area: Rect::default(),
            notes_area: Rect::default(),
            viewer_area: Rect::default(),
            folder_list_offset: 0,
            note_list_offset: 0,
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
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
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

    fn handle_mouse(&mut self, mouse: MouseEvent, scans: &Sender<ScanResult>) {
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
                    self.content = content;
                    self.current_note = Some(path.clone());
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
            Event::Resize | Event::Tick => {}
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
