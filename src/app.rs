use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

use chrono::Local;
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use percent_encoding::percent_decode_str;
use ratatui::layout::Rect;

use crate::{
    config::{self, Config, NoteFolder, NoteSort},
    error::NoteReadError,
    event::{Event, Events, ScanResult},
    markdown::{NoteLink, NoteLinkTarget},
    notes::{model::Note, naming, scan},
    terminal::TerminalGuard,
    theme::Theme,
    ui,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pane {
    Folders,
    Notes,
    Viewer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoteHistoryEntry {
    path: PathBuf,
    editor_cursor: usize,
    editor_scroll: u16,
    editor_horizontal_scroll: u16,
    viewer_scroll: u16,
}

pub struct App {
    pub note_folders: Vec<NoteFolder>,
    pub selected_folder: usize,
    pub active_folder: usize,
    pub note_sort: NoteSort,
    pub theme: Theme,
    pub all_notes: Vec<Note>,
    pub notes: Vec<Note>,
    pub selected_note: usize,
    pub pane: Pane,
    pub content: String,
    pub current_note: Option<PathBuf>,
    pub editing: bool,
    pub editor_cursor: usize,
    pub editor_scroll: u16,
    pub editor_horizontal_scroll: u16,
    pub editor_page_size: u16,
    pub viewer_scroll: u16,
    pub viewer_max_scroll: u16,
    pub viewer_page_size: u16,
    pub viewer_heading: Option<String>,
    pub status: String,
    pub loading: bool,
    pub search_query: String,
    pub searching: bool,
    pub show_help: bool,
    pub show_settings: bool,
    pub confirm_delete: bool,
    pub settings_interval: String,
    pub folder_area: Rect,
    pub notes_area: Rect,
    pub viewer_area: Rect,
    pub viewer_links: Vec<NoteLink>,
    pub viewer_link_cells: Vec<(u16, u16, usize)>,
    pub folder_list_offset: usize,
    pub note_list_offset: usize,
    note_history: Vec<NoteHistoryEntry>,
    note_history_index: Option<usize>,
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
            theme: config.theme,
            note_folders: config.note_folders,
            all_notes: Vec::new(),
            notes: Vec::new(),
            selected_note: 0,
            pane: Pane::Notes,
            content: String::new(),
            current_note: None,
            editing: false,
            editor_cursor: 0,
            editor_scroll: 0,
            editor_horizontal_scroll: 0,
            editor_page_size: 1,
            viewer_scroll: 0,
            viewer_max_scroll: 0,
            viewer_page_size: 1,
            viewer_heading: None,
            status: "Scanning notes...".into(),
            loading: true,
            search_query: String::new(),
            searching: false,
            show_help: false,
            show_settings: false,
            confirm_delete: false,
            settings_interval: config.save_interval_seconds.to_string(),
            folder_area: Rect::default(),
            notes_area: Rect::default(),
            viewer_area: Rect::default(),
            viewer_links: Vec::new(),
            viewer_link_cells: Vec::new(),
            folder_list_offset: 0,
            note_list_offset: 0,
            note_history: Vec::new(),
            note_history_index: None,
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
                self.all_notes = notes;
                self.apply_search();
                if self.search_query.is_empty() {
                    self.status = format!(
                        "{}: {} notes",
                        self.note_folders[folder].name,
                        self.notes.len()
                    );
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
        if self.confirm_delete {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.delete_current_note(),
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.confirm_delete = false;
                    self.status = "Note deletion cancelled".into();
                }
                _ => {}
            }
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.save_note();
            self.should_quit = !self.dirty;
            return;
        }
        if key.modifiers == KeyModifiers::ALT {
            match key.code {
                KeyCode::Left => {
                    self.navigate_note_history(-1);
                    return;
                }
                KeyCode::Right => {
                    self.navigate_note_history(1);
                    return;
                }
                _ => {}
            }
        }
        if self.editing {
            self.handle_editor_key(key);
            return;
        }
        if self.searching {
            self.handle_search_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => {
                self.save_note();
                self.should_quit = true;
            }
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('s') => self.show_settings = true,
            KeyCode::Char('n') => self.create_note(),
            KeyCode::Char('d') if self.current_note.is_some() => self.confirm_delete = true,
            KeyCode::Char('/') => {
                self.searching = true;
                self.pane = Pane::Notes;
                self.update_search_status();
            }
            KeyCode::Esc if !self.search_query.is_empty() => {
                self.search_query.clear();
                self.apply_search();
            }
            KeyCode::Char('e')
                if matches!(self.pane, Pane::Notes | Pane::Viewer)
                    && self.current_note.is_some() =>
            {
                self.editing = true;
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

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.searching = false;
                self.search_query.clear();
                self.apply_search();
            }
            KeyCode::Enter => {
                self.searching = false;
                self.update_search_status();
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.apply_search();
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.search_query.push(character);
                self.apply_search();
            }
            _ => {}
        }
    }

    fn apply_search(&mut self) {
        let terms = search_terms(&self.search_query);
        self.notes = if terms.is_empty() {
            self.all_notes.clone()
        } else {
            self.all_notes
                .iter()
                .filter(|note| note.matches_search(&terms))
                .cloned()
                .collect()
        };
        self.selected_note = 0;
        self.note_list_offset = 0;
        if self.notes.is_empty() {
            self.capture_note_position();
            self.content.clear();
            self.persisted_content.clear();
            self.current_note = None;
            self.viewer_scroll = 0;
        } else {
            self.load_selected_note();
        }
        self.update_search_status();
    }

    fn update_search_status(&mut self) {
        if self.search_query.is_empty() {
            self.status = format!("{} notes", self.notes.len());
        } else {
            self.status = format!(
                "Search: {} ({} of {} notes)",
                self.search_query,
                self.notes.len(),
                self.all_notes.len()
            );
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
            KeyCode::PageUp => {
                for _ in 0..self.editor_page_size {
                    self.move_editor_vertical(-1);
                }
            }
            KeyCode::PageDown => {
                for _ in 0..self.editor_page_size {
                    self.move_editor_vertical(1);
                }
            }
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.editor_cursor = 0;
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.editor_cursor = self.content.len();
            }
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
        if self.editing || self.show_settings || self.confirm_delete {
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
                if self.viewer_area.contains((mouse.column, mouse.row).into()) {
                    self.pane = Pane::Viewer;
                    if let Some(target) = self
                        .viewer_link_cells
                        .iter()
                        .find(|(column, row, _)| *column == mouse.column && *row == mouse.row)
                        .and_then(|(_, _, index)| self.viewer_links.get(*index))
                        .map(|link| link.target.clone())
                    {
                        self.open_note_link(&target);
                    }
                } else if let Some(index) = list_item_at(
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
                self.search_query.clear();
                self.searching = false;
                self.all_notes.clear();
                self.notes.clear();
                self.content.clear();
                self.current_note = None;
                self.viewer_scroll = 0;
                self.note_history.clear();
                self.note_history_index = None;
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
            self.capture_note_position();
            let previous_position = if self.current_note.as_deref() == Some(path.as_path()) {
                self.current_note_position()
            } else {
                None
            };
            match read_note(self.root(), &path) {
                Ok(content) => {
                    self.content = content.clone();
                    self.persisted_content = content;
                    self.current_note = Some(path.clone());
                    self.dirty = false;
                    self.dirty_since = None;
                    self.external_conflict = false;
                    self.editor_cursor = self.content.len();
                    self.editor_scroll = 0;
                    self.editor_horizontal_scroll = 0;
                    self.viewer_scroll = 0;
                    if let Some(position) = previous_position {
                        self.restore_note_position(&position);
                    } else {
                        self.record_note_navigation(path.clone());
                    }
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

    fn current_note_position(&self) -> Option<NoteHistoryEntry> {
        self.current_note.as_ref().map(|path| NoteHistoryEntry {
            path: path.clone(),
            editor_cursor: self.editor_cursor,
            editor_scroll: self.editor_scroll,
            editor_horizontal_scroll: self.editor_horizontal_scroll,
            viewer_scroll: self.viewer_scroll,
        })
    }

    fn capture_note_position(&mut self) {
        let Some(position) = self.current_note_position() else {
            return;
        };
        let Some(index) = self.note_history_index else {
            return;
        };
        if self
            .note_history
            .get(index)
            .is_some_and(|entry| entry.path == position.path)
        {
            self.note_history[index] = position;
        }
    }

    fn record_note_navigation(&mut self, path: PathBuf) {
        let keep = self.note_history_index.map_or(0, |index| index + 1);
        self.note_history.truncate(keep);
        self.note_history.push(NoteHistoryEntry {
            path,
            editor_cursor: self.editor_cursor,
            editor_scroll: self.editor_scroll,
            editor_horizontal_scroll: self.editor_horizontal_scroll,
            viewer_scroll: self.viewer_scroll,
        });
        self.note_history_index = Some(self.note_history.len() - 1);
    }

    fn restore_note_position(&mut self, position: &NoteHistoryEntry) {
        self.editor_cursor = position.editor_cursor.min(self.content.len());
        while !self.content.is_char_boundary(self.editor_cursor) {
            self.editor_cursor -= 1;
        }
        self.editor_scroll = position.editor_scroll;
        self.editor_horizontal_scroll = position.editor_horizontal_scroll;
        self.viewer_scroll = position.viewer_scroll;
    }

    fn navigate_note_history(&mut self, delta: isize) {
        if self.dirty {
            self.save_note();
            if self.dirty {
                return;
            }
        }
        self.capture_note_position();
        let Some(current) = self.note_history_index else {
            return;
        };
        let target_index = current
            .saturating_add_signed(delta)
            .min(self.note_history.len().saturating_sub(1));
        if target_index == current {
            return;
        }
        let entry = self.note_history[target_index].clone();
        let Some(selected_note) = self
            .all_notes
            .iter()
            .position(|note| note.relative_path == entry.path)
        else {
            self.status = format!("History note was not found: {}", entry.path.display());
            return;
        };

        self.search_query.clear();
        self.searching = false;
        self.notes = self.all_notes.clone();
        self.selected_note = selected_note;
        self.note_list_offset = 0;
        match read_note(self.root(), &entry.path) {
            Ok(content) => {
                self.content = content.clone();
                self.persisted_content = content;
                self.current_note = Some(entry.path.clone());
                self.dirty = false;
                self.dirty_since = None;
                self.external_conflict = false;
                self.restore_note_position(&entry);
                self.viewer_heading = None;
                self.note_history_index = Some(target_index);
                self.status = entry.path.display().to_string();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn open_note_link(&mut self, target: &NoteLinkTarget) {
        let heading = match target {
            NoteLinkTarget::Path(target) => target.split_once('#').and_then(|(_, heading)| {
                (!heading.is_empty())
                    .then(|| percent_decode_str(heading).decode_utf8_lossy().into_owned())
            }),
            NoteLinkTarget::Legacy(_) | NoteLinkTarget::Wiki(_) => None,
        };
        let Some(relative) = self.resolve_note_link(target) else {
            self.status = "Linked note was not found".into();
            return;
        };

        self.search_query.clear();
        self.searching = false;
        self.notes = self.all_notes.clone();
        let Some(index) = self
            .notes
            .iter()
            .position(|note| note.relative_path == relative)
        else {
            self.status = format!("Linked note was not found: {}", relative.display());
            return;
        };
        self.selected_note = index;
        self.note_list_offset = 0;
        if self.load_selected_note() {
            self.pane = Pane::Viewer;
            self.viewer_heading = heading;
        }
    }

    fn resolve_note_link(&self, target: &NoteLinkTarget) -> Option<PathBuf> {
        let notes = if self.all_notes.is_empty() {
            &self.notes
        } else {
            &self.all_notes
        };
        match target {
            NoteLinkTarget::Path(target) => {
                let target = percent_decode_str(target.split('#').next().unwrap_or(target))
                    .decode_utf8_lossy();
                let target = target.strip_prefix("file://").unwrap_or(&target);
                let path = Path::new(target);
                let relative = if path.is_absolute() {
                    path.strip_prefix(self.root()).ok()?.to_path_buf()
                } else {
                    let base = self
                        .current_note
                        .as_deref()
                        .and_then(Path::parent)
                        .unwrap_or_else(|| Path::new(""));
                    normalize_relative_path(base, path)?
                };
                notes
                    .iter()
                    .find(|note| paths_equal_case_insensitive(&note.relative_path, &relative))
                    .map(|note| note.relative_path.clone())
            }
            NoteLinkTarget::Legacy(target) => {
                let name = percent_decode_str(
                    target
                        .strip_prefix("note://")
                        .unwrap_or(target)
                        .split('#')
                        .next()
                        .unwrap_or(target)
                        .trim_end_matches('@'),
                )
                .decode_utf8_lossy();
                notes
                    .iter()
                    .find(|note| legacy_link_name(&note.name()) == legacy_link_name(&name))
                    .map(|note| note.relative_path.clone())
            }
            NoteLinkTarget::Wiki(target) => {
                let target = target.split('|').next().unwrap_or(target).trim();
                let target = target.split('#').next().unwrap_or(target).trim();
                let root_relative = target.starts_with('/');
                let target = target.trim_start_matches('/');
                let path = Path::new(target);
                let qualified = path.components().count() > 1;
                let base = self
                    .current_note
                    .as_deref()
                    .and_then(Path::parent)
                    .unwrap_or_else(|| Path::new(""));
                let expected = if qualified {
                    normalize_relative_path(if root_relative { Path::new("") } else { base }, path)
                        .map(|path| path.with_extension(""))
                } else {
                    None
                };
                notes
                    .iter()
                    .filter(|note| {
                        note.relative_path.file_stem().is_some_and(|stem| {
                            stem.to_string_lossy().eq_ignore_ascii_case(
                                path.file_stem()
                                    .unwrap_or(path.as_os_str())
                                    .to_string_lossy()
                                    .as_ref(),
                            )
                        })
                    })
                    .min_by_key(|note| {
                        if expected.as_ref().is_some_and(|expected| {
                            paths_equal_case_insensitive(
                                &note.relative_path.with_extension(""),
                                expected,
                            )
                        }) {
                            0
                        } else if note.relative_path.parent() == Some(base) {
                            1
                        } else {
                            2
                        }
                    })
                    .map(|note| note.relative_path.clone())
            }
        }
    }

    fn start_scan(&mut self, scans: &Sender<ScanResult>) {
        self.loading = true;
        self.status = format!("Scanning {}...", self.note_folders[self.active_folder].name);
        spawn_scan(self.active_folder, self.root().to_path_buf(), scans.clone());
    }

    fn create_note(&mut self) {
        let title = naming::new_note_title(Local::now().naive_local());
        let relative = PathBuf::from(format!("{title}.md"));
        let path = self.root().join(&relative);
        let content = naming::initial_content(&title);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                if let Err(error) = file.write_all(content.as_bytes()) {
                    let _ = fs::remove_file(&path);
                    self.status = format!("Unable to create {}: {error}", relative.display());
                    return;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                self.status = format!("Unable to create {}: {error}", relative.display());
                return;
            }
        }

        self.search_query.clear();
        self.searching = false;
        match scan::scan(self.root()) {
            Ok(mut notes) => {
                sort_notes(&mut notes, self.note_sort);
                self.all_notes = notes;
                self.notes = self.all_notes.clone();
                self.selected_note = self
                    .notes
                    .iter()
                    .position(|note| note.relative_path == relative)
                    .unwrap_or(0);
                self.note_list_offset = 0;
                if self.load_selected_note() {
                    self.pane = Pane::Viewer;
                    self.editing = true;
                    self.editor_cursor = self.content.len();
                    self.editor_scroll = 0;
                    self.editor_horizontal_scroll = 0;
                    self.status = format!("Created {}", relative.display());
                }
            }
            Err(error) => self.status = format!("Unable to refresh notes: {error:#}"),
        }
    }

    fn delete_current_note(&mut self) {
        self.confirm_delete = false;
        let Some(relative) = self.current_note.clone() else {
            return;
        };
        let path = self.root().join(&relative);
        let status = match trash::delete(&path) {
            Ok(()) => format!("Moved {} to trash", relative.display()),
            Err(trash_error) => match fs::remove_file(&path) {
                Ok(()) => format!("Permanently deleted {}", relative.display()),
                Err(delete_error) => {
                    self.status = format!(
                        "Unable to delete {}: trash: {trash_error}; file: {delete_error}",
                        relative.display()
                    );
                    return;
                }
            },
        };

        self.all_notes.retain(|note| note.relative_path != relative);
        self.notes.retain(|note| note.relative_path != relative);
        let previous_history_path = self.note_history_index.and_then(|index| {
            self.note_history[..index]
                .iter()
                .rev()
                .find(|entry| entry.path != relative)
                .map(|entry| entry.path.clone())
        });
        self.note_history.retain(|entry| entry.path != relative);
        self.note_history_index = previous_history_path.and_then(|path| {
            self.note_history
                .iter()
                .rposition(|entry| entry.path == path)
        });
        self.selected_note = self.selected_note.min(self.notes.len().saturating_sub(1));
        if self.notes.is_empty() {
            self.content.clear();
            self.persisted_content.clear();
            self.current_note = None;
            self.viewer_scroll = 0;
        } else {
            self.current_note = None;
            self.load_selected_note();
        }
        self.status = status;
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
                    self.update_search_text(&relative);
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
        let renamed = naming::automatic_relative_path(self.root(), &relative, &self.content);
        if renamed != relative {
            if let Err(error) = fs::rename(self.root().join(&relative), self.root().join(&renamed))
            {
                self.status = format!(
                    "Unable to rename {} to {}: {error}",
                    relative.display(),
                    renamed.display()
                );
                return;
            }
        }
        let path = self.root().join(&renamed);
        match fs::write(&path, self.content.as_bytes()) {
            Ok(()) => {
                self.persisted_content.clone_from(&self.content);
                self.dirty = false;
                self.dirty_since = None;
                self.current_note = Some(renamed.clone());
                self.update_note_after_save(&relative, &renamed);
                self.status = format!("Saved {}", renamed.display());
            }
            Err(error) => {
                if renamed != relative {
                    let _ = fs::rename(&path, self.root().join(&relative));
                }
                self.status = format!("Unable to save {}: {error}", renamed.display());
            }
        }
    }

    fn update_note_after_save(&mut self, previous: &Path, current: &Path) {
        let Ok(metadata) = fs::metadata(self.root().join(current)) else {
            return;
        };
        let search_text: std::sync::Arc<str> = self.persisted_content.to_lowercase().into();
        for note in self
            .all_notes
            .iter_mut()
            .filter(|note| note.relative_path == previous)
        {
            note.relative_path = current.to_path_buf();
            note.size = metadata.len();
            note.modified = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            note.search_text_lowercase = search_text.clone();
        }
        for entry in self
            .note_history
            .iter_mut()
            .filter(|entry| entry.path == previous)
        {
            entry.path = current.to_path_buf();
        }
        sort_notes(&mut self.all_notes, self.note_sort);
        let terms = search_terms(&self.search_query);
        self.notes = self
            .all_notes
            .iter()
            .filter(|note| note.matches_search(&terms))
            .cloned()
            .collect();
        self.selected_note = self
            .notes
            .iter()
            .position(|note| note.relative_path == current)
            .unwrap_or_else(|| self.selected_note.min(self.notes.len().saturating_sub(1)));
    }

    fn reload_note(&mut self) {
        let Some(relative) = self.current_note.clone() else {
            return;
        };
        match read_note(self.root(), &relative) {
            Ok(content) => {
                self.content = content.clone();
                self.persisted_content = content;
                self.update_search_text(&relative);
                self.editor_cursor = self.content.len();
                self.dirty = false;
                self.dirty_since = None;
                self.external_conflict = false;
                self.status = format!("Reloaded {}; local edits discarded", relative.display());
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn update_search_text(&mut self, relative: &Path) {
        let text: std::sync::Arc<str> = self.persisted_content.to_lowercase().into();
        for note in self
            .all_notes
            .iter_mut()
            .chain(self.notes.iter_mut())
            .filter(|note| note.relative_path == relative)
        {
            note.search_text_lowercase = text.clone();
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

fn normalize_relative_path(base: &Path, path: &Path) -> Option<PathBuf> {
    let mut normalized = base.to_path_buf();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                normalized.pop().then_some(())?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(normalized)
}

fn paths_equal_case_insensitive(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn legacy_link_name(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase()
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

fn search_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut term = String::new();
    let mut quoted = false;
    for character in query.chars() {
        match character {
            '"' => quoted = !quoted,
            character if character.is_whitespace() && !quoted => {
                if !term.is_empty() {
                    terms.push(term.to_lowercase());
                    term.clear();
                }
            }
            character => term.push(character),
        }
    }
    if !term.is_empty() {
        terms.push(term.to_lowercase());
    }
    terms
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
        notes::{model::Note, scan},
        theme::Theme,
    };
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    use super::{App, Pane, list_item_at, move_index, search_terms, sort_notes};
    use crate::markdown::NoteLinkTarget;

    #[test]
    fn selection_stays_in_bounds() {
        assert_eq!(move_index(0, 0, 1), 0);
        assert_eq!(move_index(0, 3, -1), 0);
        assert_eq!(move_index(2, 3, 1), 2);
        assert_eq!(move_index(1, 3, 1), 2);
    }

    #[test]
    fn search_terms_keep_quoted_phrases_together() {
        assert_eq!(
            search_terms("project \"release plan\""),
            ["project", "release plan"]
        );
    }

    #[test]
    fn searches_note_names_and_text_with_all_terms() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Project Alpha.md"), "milestones").unwrap();
        fs::write(root.path().join("meeting.md"), "Project Alpha release plan").unwrap();
        fs::write(root.path().join("unrelated.md"), "Project notes").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::Alphabetical { descending: false },
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.scan_finished(0, Ok(scan::scan(root.path()).unwrap()));

        app.search_query = "project alpha".into();
        app.apply_search();
        let paths: Vec<_> = app
            .notes
            .iter()
            .map(|note| note.relative_path.as_path())
            .collect();

        assert_eq!(
            paths,
            [Path::new("meeting.md"), Path::new("Project Alpha.md")]
        );

        app.search_query = "\"release plan\"".into();
        app.apply_search();
        assert_eq!(app.notes.len(), 1);
        assert_eq!(app.notes[0].relative_path, Path::new("meeting.md"));
    }

    #[test]
    fn escape_clears_retained_search_and_restores_all_notes() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("first.md"), "matching").unwrap();
        fs::write(root.path().join("second.md"), "other").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.scan_finished(0, Ok(scan::scan(root.path()).unwrap()));
        app.search_query = "matching".into();
        app.apply_search();
        assert_eq!(app.notes.len(), 1);
        let (scan_tx, _scan_rx) = mpsc::channel();

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &scan_tx);

        assert!(!app.searching);
        assert!(app.search_query.is_empty());
        assert_eq!(app.notes.len(), 2);
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
            theme: Theme::default(),
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
    fn resolves_qownnotes_note_link_formats() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("folder")).unwrap();
        fs::write(root.path().join("folder/Source.md"), "source").unwrap();
        fs::write(root.path().join("folder/Relative note.md"), "relative").unwrap();
        fs::write(root.path().join("Other note.md"), "other").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::Alphabetical { descending: false },
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.scan_finished(0, Ok(scan::scan(root.path()).unwrap()));
        app.selected_note = app
            .notes
            .iter()
            .position(|note| note.relative_path == Path::new("folder/Source.md"))
            .unwrap();
        app.load_selected_note();

        assert_eq!(
            app.resolve_note_link(&NoteLinkTarget::Path("Relative%20note.md#Part".into())),
            Some("folder/Relative note.md".into())
        );
        assert_eq!(
            app.resolve_note_link(&NoteLinkTarget::Legacy("note://Other_note".into())),
            Some("Other note.md".into())
        );
        assert_eq!(
            app.resolve_note_link(&NoteLinkTarget::Wiki("/Other note#Heading|label".into())),
            Some("Other note.md".into())
        );
    }

    #[test]
    fn clicking_a_wrapped_viewer_link_opens_the_note() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("Source.md"),
            "Words before a [linked note](Target%20note.md#Section%20One) that can wrap.",
        )
        .unwrap();
        let target_content = format!("{}## Section One\nbody", "preface\n".repeat(12));
        fs::write(root.path().join("Target note.md"), &target_content).unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::Alphabetical { descending: false },
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.scan_finished(0, Ok(scan::scan(root.path()).unwrap()));
        app.selected_note = app
            .notes
            .iter()
            .position(|note| note.relative_path == Path::new("Source.md"))
            .unwrap();
        app.load_selected_note();
        app.pane = Pane::Viewer;
        let mut terminal = Terminal::new(TestBackend::new(70, 12)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw(frame, &mut app))
            .unwrap();
        let (column, row, _) = app.viewer_link_cells[0];
        let (scan_tx, _scan_rx) = mpsc::channel();

        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            },
            &scan_tx,
        );

        assert_eq!(
            app.current_note.as_deref(),
            Some(Path::new("Target note.md"))
        );
        assert_eq!(app.content, target_content);
        assert_eq!(app.pane, Pane::Viewer);

        terminal
            .draw(|frame| crate::ui::draw(frame, &mut app))
            .unwrap();
        assert!(app.viewer_scroll > 0);
        assert!(app.viewer_heading.is_none());
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
            theme: Theme::default(),
        });
        app.notes = vec![
            Note {
                relative_path: "first.md".into(),
                size: 5,
                modified: SystemTime::UNIX_EPOCH,
                search_text_lowercase: "first".into(),
            },
            Note {
                relative_path: "second.md".into(),
                size: 6,
                modified: SystemTime::UNIX_EPOCH,
                search_text_lowercase: "second".into(),
            },
        ];

        app.move_selection(1);

        assert_eq!(app.selected_note, 1);
        assert_eq!(app.content, "second");
        assert_eq!(app.current_note.as_deref(), Some(Path::new("second.md")));
        assert_eq!(app.pane, Pane::Notes);
    }

    #[test]
    fn alt_arrows_navigate_history_and_restore_note_positions() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("first.md"), "first\nline").unwrap();
        fs::write(root.path().join("second.md"), "second\nline").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::Alphabetical { descending: false },
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.scan_finished(0, Ok(scan::scan(root.path()).unwrap()));
        let (scan_tx, _scan_rx) = mpsc::channel();
        app.viewer_scroll = 4;
        app.editor_cursor = 2;
        app.editor_scroll = 1;
        app.editor_horizontal_scroll = 3;

        app.move_selection(1);
        app.viewer_scroll = 7;
        app.editor_cursor = 4;
        app.editor_scroll = 2;
        app.editor_horizontal_scroll = 5;
        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT), &scan_tx);

        assert_eq!(app.current_note.as_deref(), Some(Path::new("first.md")));
        assert_eq!(app.selected_note, 0);
        assert_eq!(app.viewer_scroll, 4);
        assert_eq!(app.editor_cursor, 2);
        assert_eq!(app.editor_scroll, 1);
        assert_eq!(app.editor_horizontal_scroll, 3);

        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT), &scan_tx);

        assert_eq!(app.current_note.as_deref(), Some(Path::new("second.md")));
        assert_eq!(app.selected_note, 1);
        assert_eq!(app.viewer_scroll, 7);
        assert_eq!(app.editor_cursor, 4);
        assert_eq!(app.editor_scroll, 2);
        assert_eq!(app.editor_horizontal_scroll, 5);
    }

    #[test]
    fn selecting_a_note_after_going_back_discards_forward_history() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("first.md"), "first").unwrap();
        fs::write(root.path().join("second.md"), "second").unwrap();
        fs::write(root.path().join("third.md"), "third").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::Alphabetical { descending: false },
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.scan_finished(0, Ok(scan::scan(root.path()).unwrap()));
        let (scan_tx, _scan_rx) = mpsc::channel();
        app.move_selection(1);
        app.move_selection(1);
        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT), &scan_tx);

        app.move_selection(-1);
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT), &scan_tx);

        assert_eq!(app.current_note.as_deref(), Some(Path::new("first.md")));
        assert_eq!(app.note_history.len(), 3);
        assert_eq!(app.note_history_index, Some(2));
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
            theme: Theme::default(),
        });
        let notes = vec![Note {
            relative_path: "note.md".into(),
            size: 9,
            modified: SystemTime::UNIX_EPOCH,
            search_text_lowercase: "previewed".into(),
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
            theme: Theme::default(),
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 7,
            modified: SystemTime::UNIX_EPOCH,
            search_text_lowercase: "content".into(),
        });
        let (scan_tx, _scan_rx) = mpsc::channel();

        app.open_selection(&scan_tx);

        assert_eq!(app.content, "content");
        assert_eq!(app.pane, Pane::Viewer);
    }

    #[test]
    fn e_on_note_list_opens_the_editor() {
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
            theme: Theme::default(),
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 7,
            modified: SystemTime::UNIX_EPOCH,
            search_text_lowercase: "content".into(),
        });
        app.load_selected_note();
        let (scan_tx, _scan_rx) = mpsc::channel();

        app.handle_key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &scan_tx,
        );

        assert!(app.editing);
        assert_eq!(app.editor_cursor, app.content.len());
        assert_eq!(app.pane, Pane::Notes);
    }

    #[test]
    fn deletion_requires_confirmation_and_can_be_cancelled() {
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
            theme: Theme::default(),
        });
        app.scan_finished(0, Ok(scan::scan(root.path()).unwrap()));
        let (scan_tx, _scan_rx) = mpsc::channel();

        app.handle_key(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
            &scan_tx,
        );

        assert!(app.confirm_delete);
        assert!(root.path().join("note.md").exists());

        app.handle_key(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
            &scan_tx,
        );

        assert!(!app.confirm_delete);
        assert!(root.path().join("note.md").exists());
        assert_eq!(app.status, "Note deletion cancelled");
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
            theme: Theme::default(),
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 7,
            modified: SystemTime::UNIX_EPOCH,
            search_text_lowercase: "content".into(),
        });
        app.load_selected_note();
        app.editing = true;
        app.editor_cursor = app.content.len();

        app.handle_editor_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));
        app.save_note();

        assert_eq!(
            fs::read_to_string(root.path().join("content!.md")).unwrap(),
            "content!"
        );
        assert!(!root.path().join("note.md").exists());
        assert_eq!(app.current_note.as_deref(), Some(Path::new("content!.md")));
        assert!(!app.dirty);
    }

    #[test]
    fn creates_a_timestamped_note_and_opens_it_for_editing() {
        let root = tempfile::tempdir().unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
            theme: Theme::default(),
        });

        app.create_note();

        let relative = app.current_note.as_ref().unwrap();
        let filename = relative.file_name().unwrap().to_string_lossy();
        assert!(filename.starts_with("Note "));
        assert!(filename.ends_with(".md"));
        assert_eq!(
            app.content,
            fs::read_to_string(root.path().join(relative)).unwrap()
        );
        assert!(app.content.starts_with("# Note "));
        assert!(app.content.ends_with("\n\n"));
        assert!(app.editing);
        assert_eq!(app.pane, Pane::Viewer);
    }

    #[test]
    fn automatic_rename_adds_a_collision_suffix() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("old.md"), "# Old\n").unwrap();
        fs::write(root.path().join("Project.md"), "existing").unwrap();
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: root.path().into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.scan_finished(0, Ok(scan::scan(root.path()).unwrap()));
        app.selected_note = app
            .notes
            .iter()
            .position(|note| note.relative_path == Path::new("old.md"))
            .unwrap();
        app.load_selected_note();
        app.content = "# Project\nbody".into();
        app.mark_dirty();

        app.save_note();

        assert_eq!(
            fs::read_to_string(root.path().join("Project 1.md")).unwrap(),
            "# Project\nbody"
        );
        assert_eq!(app.current_note.as_deref(), Some(Path::new("Project 1.md")));
    }

    #[test]
    fn editor_page_keys_move_by_the_viewport_height() {
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: "/notes".into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.content = "zero\none\ntwo\nthree\nfour\nfive".into();
        app.editor_cursor = "zero\non".len();
        app.editor_page_size = 3;

        app.handle_editor_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(
            &app.content[..app.editor_cursor],
            "zero\none\ntwo\nthree\nfo"
        );

        app.handle_editor_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert_eq!(&app.content[..app.editor_cursor], "zero\non");
    }

    #[test]
    fn editor_control_home_and_end_move_to_note_boundaries() {
        let mut app = App::new(Config {
            note_folders: vec![NoteFolder {
                name: "Notes".into(),
                path: "/notes".into(),
            }],
            active_folder: 0,
            note_sort: NoteSort::LastModified,
            save_interval_seconds: 10,
            theme: Theme::default(),
        });
        app.content = "first\nsecond\nthird".into();
        app.editor_cursor = "first\nsec".len();

        app.handle_editor_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
        assert_eq!(app.editor_cursor, 0);

        app.handle_editor_key(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL));
        assert_eq!(app.editor_cursor, app.content.len());
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
            theme: Theme::default(),
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 8,
            modified: SystemTime::UNIX_EPOCH,
            search_text_lowercase: "original".into(),
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
            theme: Theme::default(),
        });
        app.notes.push(Note {
            relative_path: "note.md".into(),
            size: 8,
            modified: SystemTime::UNIX_EPOCH,
            search_text_lowercase: "original".into(),
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
                search_text_lowercase: "".into(),
            },
            Note {
                relative_path: "Alpha.md".into(),
                size: 0,
                modified: now + Duration::from_secs(1),
                search_text_lowercase: "".into(),
            },
        ];
        sort_notes(&mut notes, NoteSort::LastModified);
        assert_eq!(notes[0].relative_path, Path::new("Alpha.md"));
        sort_notes(&mut notes, NoteSort::Alphabetical { descending: true });
        assert_eq!(notes[0].relative_path, Path::new("zebra.md"));
    }
}
