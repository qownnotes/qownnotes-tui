use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

use crate::{
    app::{App, Pane},
    markdown,
};

const NARROW_WIDTH: u16 = 80;

pub fn draw(frame: &mut Frame, app: &mut App) {
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
        draw_help(frame);
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
    let items = app
        .note_folders
        .iter()
        .enumerate()
        .map(|(index, folder)| {
            let marker = if index == app.active_folder {
                "● "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Green)),
                Span::raw(folder.name.clone()),
            ]))
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default()
        .with_offset(app.folder_list_offset)
        .with_selected(Some(app.selected_folder));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane_block("Note folders", app.pane == Pane::Folders))
            .highlight_style(highlight_style())
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
        .map(|note| ListItem::new(note.name()))
        .collect::<Vec<_>>();
    let mut state = ListState::default()
        .with_offset(app.note_list_offset)
        .with_selected((!app.notes.is_empty()).then_some(app.selected_note));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane_block("Notes", app.pane == Pane::Notes))
            .highlight_style(highlight_style())
            .highlight_symbol("› "),
        area,
        &mut state,
    );
    app.note_list_offset = state.offset();
}

fn draw_viewer(frame: &mut Frame, app: &mut App, area: Rect) {
    app.viewer_area = area;
    let text = if app.current_note.is_some() {
        markdown::highlight(&app.content)
    } else {
        "Select a note and press Enter to open it.".into()
    };
    let paragraph = Paragraph::new(text)
        .block(pane_block("Viewer", app.pane == Pane::Viewer))
        .wrap(Wrap { trim: false });
    let viewport_width = area.width.saturating_sub(2).max(1);
    let viewport_height = area.height.saturating_sub(2);
    let line_count = paragraph.line_count(viewport_width);
    app.viewer_page_size = viewport_height.max(1);
    app.viewer_max_scroll = line_count
        .saturating_sub(viewport_height as usize)
        .min(u16::MAX as usize) as u16;
    app.viewer_scroll = app.viewer_scroll.min(app.viewer_max_scroll);
    frame.render_widget(paragraph.scroll((app.viewer_scroll, 0)), area);

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

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let mode = format!(" {:?} ", app.pane);
    let help = if app.loading {
        " scanning "
    } else if app.pane == Pane::Viewer {
        " j/k scroll  PgUp/PgDn page  ? help  q quit "
    } else {
        " ? help  R reload  q quit "
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(mode, Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(format!(" {}", app.status)),
            Span::styled(help, Style::default().fg(Color::DarkGray)),
        ])),
        area,
    );
}

fn draw_help(frame: &mut Frame) {
    let area = centered_rect(58, 17, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(
            "j / k       move selection or scroll viewer\n\
             PgUp/PgDn   scroll viewer by one page\n\
             Home / End  first or last viewer line\n\
             h / l       move between panes\n\
             Tab         next pane\n\
             Enter       activate note folder or open note\n\
             Mouse       activate items or scroll panes\n\
             R           reload active note folder\n\
             ?           toggle this help\n\
             q / Ctrl-c  quit\n\n\
             The application is read-only.",
        )
        .block(Block::default().title(" Help ").borders(Borders::ALL))
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn pane_block(title: &'static str, active: bool) -> Block<'static> {
    let style = if active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(style)
}

fn highlight_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
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
