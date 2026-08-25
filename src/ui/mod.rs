use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Widget, Wrap,
    },
};

use crate::{
    app::{App, FolderEntry, FolderFilter, Pane, TextCell},
    markdown,
};

const NARROW_WIDTH: u16 = 80;

pub fn draw(frame: &mut Frame, app: &mut App) {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .fg(app.theme.foreground.into())
                .bg(app.theme.background.into()),
        ),
        frame.area(),
    );
    app.folder_area = Rect::default();
    app.notes_area = Rect::default();
    app.viewer_area = Rect::default();
    let [main, status] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    if main.width < NARROW_WIDTH {
        draw_narrow(frame, app, main);
    } else {
        let [folders, notes, viewer] = Layout::horizontal([
            Constraint::Percentage(22),
            Constraint::Percentage(30),
            Constraint::Percentage(48),
        ])
        .areas(main);
        draw_folders(frame, app, folders);
        draw_notes(frame, app, notes);
        draw_viewer(frame, app, viewer);
    }
    draw_status(frame, app, status);
    if app.show_help {
        draw_help(frame, app);
    }
    if app.show_settings {
        draw_settings(frame, app);
    }
    if app.confirm_delete {
        draw_delete_confirmation(frame, app);
    }
}

fn draw_narrow(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.pane {
        Pane::Folders => draw_folders(frame, app, area),
        Pane::Notes => draw_notes(frame, app, area),
        Pane::Viewer => draw_viewer(frame, app, area),
    }
}

fn draw_folders(frame: &mut Frame, app: &mut App, area: Rect) {
    app.folder_area = area;
    let rows = app.folder_rows();
    let items = rows
        .iter()
        .map(|row| {
            let indent = "  ".repeat(row.depth);
            let disclosure = if row.has_children {
                if row.expanded { "▾ " } else { "▸ " }
            } else {
                "  "
            };
            let (active, label) = match &row.entry {
                FolderEntry::NoteFolder(index) => (
                    *index == app.active_folder,
                    app.note_folders[*index].name.clone(),
                ),
                FolderEntry::AllNotes => (
                    app.folder_filter == FolderFilter::AllNotes,
                    "All notes".into(),
                ),
                FolderEntry::Directory(path) => (
                    app.folder_filter == FolderFilter::Directory(path.clone()),
                    if path.as_os_str().is_empty() {
                        "/".into()
                    } else {
                        path.file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy()
                            .into_owned()
                    },
                ),
            };
            let marker = if active { "● " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::raw(indent),
                Span::styled(marker, Style::default().fg(app.theme.success.into())),
                Span::raw(disclosure),
                Span::raw(label),
            ]))
        })
        .collect::<Vec<_>>();
    app.selected_folder_row = app.selected_folder_row.min(rows.len().saturating_sub(1));
    let mut state = ListState::default()
        .with_offset(app.folder_list_offset)
        .with_selected((!rows.is_empty()).then_some(app.selected_folder_row));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane_block("Folders", app.pane == Pane::Folders, app))
            .highlight_style(highlight_style(app))
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol("› "),
        area,
        &mut state,
    );
    app.folder_list_offset = state.offset();
}

fn draw_notes(frame: &mut Frame, app: &mut App, area: Rect) {
    app.notes_area = area;
    let items = app
        .notes
        .iter()
        .map(|note| ListItem::new(app.note_label(note)))
        .collect::<Vec<_>>();
    let mut state = ListState::default()
        .with_offset(app.note_list_offset)
        .with_selected((!app.notes.is_empty()).then_some(app.selected_note));
    let title = if app.searching {
        format!("Search: {}_", app.search_query)
    } else if !app.search_query.is_empty() {
        format!("Search: {}", app.search_query)
    } else {
        "Notes".into()
    };
    frame.render_stateful_widget(
        List::new(items)
            .block(pane_block(&title, app.pane == Pane::Notes, app))
            .highlight_style(highlight_style(app))
            .highlight_symbol("› "),
        area,
        &mut state,
    );
    app.note_list_offset = state.offset();
}

fn draw_viewer(frame: &mut Frame, app: &mut App, area: Rect) {
    app.viewer_area = area;
    app.viewer_links.clear();
    app.viewer_link_cells.clear();
    app.viewer_text_cells.clear();
    app.editor_text_cells.clear();
    if app.editing {
        draw_editor(frame, app, area);
        return;
    }
    let text = if app.current_note.is_some() {
        markdown::highlight_selection(&app.content, &app.theme, app.viewer_selection())
    } else {
        "Select a note to preview it.".into()
    };
    let viewport_width = area.width.saturating_sub(2).max(1);
    let viewport_height = area.height.saturating_sub(2);
    if let Some(heading) = app.viewer_heading.take() {
        if let Some(offset) = markdown::heading_source_offset(&app.content, &heading) {
            let preceding = app.content[..offset].strip_suffix('\n').unwrap_or("");
            app.viewer_scroll = if offset == 0 {
                0
            } else {
                Paragraph::new(markdown::highlight(preceding, &app.theme))
                    .wrap(Wrap { trim: false })
                    .line_count(viewport_width)
                    .min(u16::MAX as usize) as u16
            };
        } else {
            app.status = format!("Heading not found: {heading}");
        }
    }
    let paragraph = Paragraph::new(text)
        .block(pane_block("Viewer", app.pane == Pane::Viewer, app))
        .wrap(Wrap { trim: false });
    let line_count = paragraph.line_count(viewport_width);
    app.viewer_page_size = viewport_height.max(1);
    app.viewer_max_scroll = line_count
        .saturating_sub(viewport_height as usize)
        .min(u16::MAX as usize) as u16;
    app.viewer_scroll = app.viewer_scroll.min(app.viewer_max_scroll);
    if app.viewer_follow_selection && app.current_note.is_some() {
        app.viewer_scroll = scroll_showing_offset(
            &app.content,
            area,
            app.viewer_scroll,
            app.viewer_max_scroll,
            app.viewer_cursor,
        );
    }
    frame.render_widget(paragraph.scroll((app.viewer_scroll, 0)), area);
    if app.current_note.is_some() {
        app.viewer_text_cells = text_cells(
            &app.content,
            area,
            Some(Wrap { trim: false }),
            (app.viewer_scroll, 0),
        );
        if app.pane == Pane::Viewer {
            if let Some(position) =
                cursor_cell_position(&app.viewer_text_cells, app.viewer_cursor, area)
            {
                frame.set_cursor_position(position);
            }
        }
    }

    if app.current_note.is_some() {
        app.viewer_links = markdown::note_links(&app.content);
        let metadata = markdown::link_metadata(&app.content, &app.viewer_links);
        let mut metadata_buffer = Buffer::empty(area);
        Paragraph::new(metadata)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: false })
            .scroll((app.viewer_scroll, 0))
            .render(area, &mut metadata_buffer);
        for row in area.y..area.bottom() {
            for column in area.x..area.right() {
                if let Some(index) = metadata_buffer
                    .cell((column, row))
                    .and_then(|cell| markdown::color_link_id(cell.fg))
                    .filter(|index| *index < app.viewer_links.len())
                {
                    app.viewer_link_cells.push((column, row, index));
                }
            }
        }
    }

    if app.viewer_max_scroll > 0 {
        let mut scrollbar = ScrollbarState::new(line_count)
            .position(app.viewer_scroll as usize)
            .viewport_content_length(viewport_height as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar,
        );
    }
}

fn scroll_showing_offset(
    content: &str,
    area: Rect,
    scroll: u16,
    max_scroll: u16,
    offset: usize,
) -> u16 {
    if shows_offset(content, area, scroll, offset) {
        return scroll;
    }
    let mut low = 0;
    let mut high = max_scroll;
    while low < high {
        let middle = low + (high - low) / 2;
        if reaches_offset(content, area, middle, offset) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn shows_offset(content: &str, area: Rect, scroll: u16, offset: usize) -> bool {
    let cells = text_cells(content, area, Some(Wrap { trim: false }), (scroll, 0));
    let (Some(first), Some(last)) = (cells.first(), cells.last()) else {
        return false;
    };
    (first.start..=last.end).contains(&offset)
}

fn reaches_offset(content: &str, area: Rect, scroll: u16, offset: usize) -> bool {
    text_cells(content, area, Some(Wrap { trim: false }), (scroll, 0))
        .last()
        .is_some_and(|cell| cell.end >= offset)
}

fn cursor_cell_position(cells: &[TextCell], cursor: usize, area: Rect) -> Option<(u16, u16)> {
    if let Some(cell) = cells.iter().find(|cell| cell.start == cursor) {
        return Some((cell.column, cell.row));
    }
    let cell = cells.iter().rev().find(|cell| cell.end == cursor)?;
    Some((
        cell.column
            .saturating_add(1)
            .min(area.right().saturating_sub(2)),
        cell.row,
    ))
}

fn draw_editor(frame: &mut Frame, app: &mut App, area: Rect) {
    let viewport_width = area.width.saturating_sub(2).max(1);
    let viewport_height = area.height.saturating_sub(2).max(1);
    app.editor_page_size = viewport_height;
    let before_cursor = &app.content[..app.editor_cursor];
    let cursor_line = before_cursor.bytes().filter(|byte| *byte == b'\n').count() as u16;
    let line_start = before_cursor.rfind('\n').map_or(0, |index| index + 1);
    let cursor_column = app.content[line_start..app.editor_cursor].chars().count() as u16;

    if cursor_line < app.editor_scroll {
        app.editor_scroll = cursor_line;
    } else if cursor_line >= app.editor_scroll.saturating_add(viewport_height) {
        app.editor_scroll = cursor_line.saturating_sub(viewport_height - 1);
    }
    if cursor_column < app.editor_horizontal_scroll {
        app.editor_horizontal_scroll = cursor_column;
    } else if cursor_column >= app.editor_horizontal_scroll.saturating_add(viewport_width) {
        app.editor_horizontal_scroll = cursor_column.saturating_sub(viewport_width - 1);
    }

    frame.render_widget(
        Paragraph::new(markdown::highlight_selection(
            &app.content,
            &app.theme,
            app.editor_selection(),
        ))
        .block(pane_block("Editor", true, app))
        .scroll((app.editor_scroll, app.editor_horizontal_scroll)),
        area,
    );
    app.editor_text_cells = text_cells(
        &app.content,
        area,
        None,
        (app.editor_scroll, app.editor_horizontal_scroll),
    );
    let position =
        cursor_cell_position(&app.editor_text_cells, app.editor_cursor, area).unwrap_or((
            area.x + 1 + cursor_column.saturating_sub(app.editor_horizontal_scroll),
            area.y + 1 + cursor_line.saturating_sub(app.editor_scroll),
        ));
    frame.set_cursor_position(position);
}

fn text_cells(source: &str, area: Rect, wrap: Option<Wrap>, scroll: (u16, u16)) -> Vec<TextCell> {
    let mut metadata_buffer = Buffer::empty(area);
    let mut paragraph = Paragraph::new(markdown::selection_metadata(source))
        .block(Block::default().borders(Borders::ALL))
        .scroll(scroll);
    if let Some(wrap) = wrap {
        paragraph = paragraph.wrap(wrap);
    }
    paragraph.render(area, &mut metadata_buffer);

    let mut rows = Vec::new();
    for row in area.y.saturating_add(1)..area.bottom().saturating_sub(1) {
        let mut cells = Vec::new();
        for column in area.x.saturating_add(1)..area.right().saturating_sub(1) {
            let Some(cell) = metadata_buffer.cell((column, row)) else {
                continue;
            };
            let (Some(start), Some(end)) = (
                markdown::decode_source_offset(cell.fg),
                markdown::decode_source_offset(cell.bg),
            ) else {
                continue;
            };
            cells.push(TextCell {
                column,
                row,
                start,
                end,
            });
        }
        rows.push((row, cells));
    }
    fill_blank_rows(source, area, &mut rows);
    rows.into_iter().flat_map(|(_, cells)| cells).collect()
}

/// Blank source lines render no cells, so their positions are derived from the
/// surrounding rows to keep every visible line selectable.
fn fill_blank_rows(source: &str, area: Rect, rows: &mut [(u16, Vec<TextCell>)]) {
    let column = area.x.saturating_add(1);
    let mut previous_end = None;
    for (row, cells) in rows.iter_mut() {
        if let Some(last) = cells.last() {
            previous_end = Some(last.end);
            continue;
        }
        let Some(offset) = previous_end.map(|end: usize| end + 1) else {
            continue;
        };
        if offset > source.len() {
            continue;
        }
        cells.push(TextCell {
            column,
            row: *row,
            start: offset,
            end: offset,
        });
        previous_end = Some(offset);
    }

    let mut next_start = None;
    for (row, cells) in rows.iter_mut().rev() {
        if let Some(first) = cells.first() {
            next_start = Some(first.start);
            continue;
        }
        let Some(offset) = next_start.and_then(|start: usize| start.checked_sub(1)) else {
            continue;
        };
        cells.push(TextCell {
            column,
            row: *row,
            start: offset,
            end: offset,
        });
        next_start = Some(offset);
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let mode = if app.editing {
        " Edit ".into()
    } else {
        format!(" {:?} ", app.pane)
    };
    let help = if app.loading {
        " scanning "
    } else if app.editing {
        " Shift-arrows select  Ctrl-Space toggle checkbox  Ctrl-x/c/v cut/copy/paste  Ctrl-s save  Esc close "
    } else if matches!(app.pane, Pane::Notes | Pane::Viewer) {
        " n new  d delete  / search  e edit  j/k scroll  s settings  ? help  q quit "
    } else {
        " Enter filter  Left/Right tree  s settings  R reload  ? help  q quit "
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                mode,
                Style::default()
                    .fg(app.theme.accent_foreground.into())
                    .bg(app.theme.accent.into()),
            ),
            Span::raw(format!(" {}", app.status)),
            Span::styled(help, Style::default().fg(app.theme.muted.into())),
        ])),
        area,
    );
}

fn draw_help(frame: &mut Frame, app: &App) {
    let area = centered_rect(58, 24, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(
            "Arrow keys  move cursor or navigate the folder tree\n\
             Shift-Arrows select text in viewer or editor\n\
             j / k       move list selection or scroll viewer\n\
             PgUp/PgDn   scroll viewer by one page\n\
             Home / End  first or last viewer line\n\
             h / l       move between panes\n\
             Tab/Shift-Tab  next or previous pane\n\
             Alt-Left/Right  back or forward in note history\n\
             Enter       activate a folder filter, note, or viewer\n\
             Ctrl-Space  open link or toggle checkbox at the cursor\n\
             Mouse       select text, activate items/links, or scroll panes\n\
             /           search note names and text\n\
             n / Ctrl-n  create a timestamped note\n\
             d           delete the selected note\n\
             e           edit the selected note\n\
             s           open settings\n\
             Ctrl-s      save while editing\n\
             Ctrl-r      discard edits and reload from disk\n\
             PgUp/PgDn   move by one page while editing\n\
             Ctrl-Home/End  first or last editor position\n\
             Ctrl-x/c/v  cut, copy, or paste in editor\n\
             Esc         leave editor or return to note list\n\
             R           reload active note folder\n\
             ?           toggle this help\n\
             q           quit",
        )
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .style(base_style(app))
                .border_style(Style::default().fg(app.theme.accent.into())),
        )
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_settings(frame: &mut Frame, app: &App) {
    let area = centered_rect(54, 11, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .style(base_style(app))
        .border_style(Style::default().fg(app.theme.accent.into()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [tabs, body, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(" General ").style(
            Style::default()
                .fg(app.theme.accent_foreground.into())
                .bg(app.theme.accent.into())
                .add_modifier(Modifier::BOLD),
        ),
        tabs,
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Note autosave interval"),
            Line::from(vec![
                Span::raw("Seconds: "),
                Span::styled(
                    format!(" {} ", app.settings_interval),
                    Style::default()
                        .fg(app.theme.foreground.into())
                        .bg(app.theme.field_background.into()),
                ),
            ]),
        ]),
        body,
    );
    frame.render_widget(
        Paragraph::new("Enter save  Up/Down adjust  Esc cancel")
            .style(Style::default().fg(app.theme.muted.into())),
        footer,
    );
}

fn draw_delete_confirmation(frame: &mut Frame, app: &App) {
    let area = centered_rect(62, 9, frame.area());
    frame.render_widget(Clear, area);
    let note = app
        .current_note
        .as_ref()
        .map_or_else(String::new, |path| path.display().to_string());
    frame.render_widget(
        Paragraph::new(format!(
            "Delete {note}?\n\nThe note will be moved to trash when available; otherwise it will be permanently deleted.\n\ny / Enter confirm    n / Esc cancel"
        ))
        .block(
            Block::default()
                .title(" Confirm deletion ")
                .borders(Borders::ALL)
                .style(base_style(app))
                .border_style(Style::default().fg(app.theme.error.into())),
        )
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn pane_block(title: &str, active: bool, app: &App) -> Block<'static> {
    let style = if active {
        Style::default().fg(app.theme.accent.into())
    } else {
        Style::default().fg(app.theme.muted.into())
    };
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(style)
}

fn highlight_style(app: &App) -> Style {
    Style::default()
        .fg(app.theme.accent_foreground.into())
        .bg(app.theme.accent.into())
        .add_modifier(Modifier::BOLD)
}

fn base_style(app: &App) -> Style {
    Style::default()
        .fg(app.theme.foreground.into())
        .bg(app.theme.background.into())
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn selection_metadata_wraps_exactly_like_rendered_text() {
        let content = format!(
            "# Heading\n\n{}\n\n- list item with `code` and [link](target.md)\n\n{}\n",
            "wrapped words ".repeat(9),
            "trailing paragraph ".repeat(5)
        );
        let theme = Theme::default();

        for width in [24, 37, 56, 80] {
            let highlighted = Paragraph::new(markdown::highlight(&content, &theme))
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: false });
            let metadata = Paragraph::new(markdown::selection_metadata(&content))
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: false });

            assert_eq!(
                metadata.line_count(width),
                highlighted.line_count(width),
                "wrapped row count differs at width {width}"
            );
        }
    }

    #[test]
    fn text_cells_cover_every_visible_row_including_blank_lines() {
        let content = "first\n\n\nlast".to_owned();
        let area = Rect::new(0, 0, 20, 8);

        let cells = text_cells(&content, area, Some(Wrap { trim: false }), (0, 0));

        let blank_offsets = cells
            .iter()
            .filter(|cell| cell.start == cell.end)
            .map(|cell| cell.start)
            .collect::<Vec<_>>();
        assert_eq!(blank_offsets, [6, 7]);
        assert_eq!(cells.first().map(|cell| cell.start), Some(0));
        assert_eq!(cells.last().map(|cell| cell.end), Some(content.len()));
    }
}
