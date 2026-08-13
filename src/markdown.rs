use std::ops::Range;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

use crate::theme::Theme;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoteLinkTarget {
    Path(String),
    Legacy(String),
    Wiki(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteLink {
    pub range: Range<usize>,
    pub target: NoteLinkTarget,
}

pub fn highlight(source: &str, theme: &Theme) -> Text<'static> {
    let mut in_fence = false;
    let lines = source
        .split('\n')
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
                return Line::styled(
                    line.to_owned(),
                    Style::default()
                        .fg(theme.fence.into())
                        .add_modifier(Modifier::BOLD),
                );
            }
            if in_fence {
                return Line::styled(line.to_owned(), Style::default().fg(theme.code.into()));
            }
            if trimmed.starts_with('#')
                && trimmed
                    .trim_start_matches('#')
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
            {
                return Line::styled(
                    line.to_owned(),
                    Style::default()
                        .fg(theme.heading.into())
                        .add_modifier(Modifier::BOLD),
                );
            }
            if trimmed.starts_with('>') {
                return Line::styled(
                    line.to_owned(),
                    Style::default()
                        .fg(theme.quote.into())
                        .add_modifier(Modifier::ITALIC),
                );
            }
            if let Some(marker_end) = list_marker_end(line) {
                let (marker, rest) = line.split_at(marker_end);
                let mut spans = vec![Span::styled(
                    marker.to_owned(),
                    Style::default()
                        .fg(theme.warning.into())
                        .add_modifier(Modifier::BOLD),
                )];
                spans.extend(highlight_inline(rest, theme));
                return Line::from(spans);
            }
            Line::from(highlight_inline(line, theme))
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

pub fn highlight_selection(
    source: &str,
    theme: &Theme,
    selection: Option<Range<usize>>,
) -> Text<'static> {
    let Some(selection) = selection else {
        return highlight(source, theme);
    };
    let highlighted = highlight(source, theme);
    let mut source_offset = 0;
    let lines = highlighted
        .lines
        .into_iter()
        .map(|line| {
            let line_style = line.style;
            let mut spans = Vec::new();
            for span in line.spans {
                let text = span.content.into_owned();
                let span_start = source_offset;
                let span_end = span_start + text.len();
                let selected_start = selection.start.clamp(span_start, span_end) - span_start;
                let selected_end = selection.end.clamp(span_start, span_end) - span_start;
                let style = line_style.patch(span.style);
                if selected_start > 0 {
                    spans.push(Span::styled(text[..selected_start].to_owned(), style));
                }
                if selected_end > selected_start {
                    spans.push(Span::styled(
                        text[selected_start..selected_end].to_owned(),
                        style.add_modifier(Modifier::REVERSED),
                    ));
                }
                if selected_end < text.len() {
                    spans.push(Span::styled(text[selected_end..].to_owned(), style));
                }
                source_offset = span_end;
            }
            source_offset += 1;
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn list_marker_end(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = &line[indent..];
    if ["- ", "* ", "+ "]
        .iter()
        .any(|marker| trimmed.starts_with(marker))
    {
        return Some(indent + 2);
    }
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0 && trimmed[digits..].starts_with(". ")).then_some(indent + digits + 2)
}

fn highlight_inline(mut text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    while !text.is_empty() {
        let next = ['`', '[', '<', '*']
            .iter()
            .filter_map(|character| text.find(*character))
            .min()
            .unwrap_or(text.len());
        if next > 0 {
            spans.push(Span::raw(text[..next].to_owned()));
            text = &text[next..];
            continue;
        }
        if let Some(rest) = text.strip_prefix('`') {
            if let Some(end) = rest.find('`') {
                let length = end + 2;
                spans.push(Span::styled(
                    text[..length].to_owned(),
                    Style::default().fg(theme.code.into()),
                ));
                text = &text[length..];
                continue;
            }
        }
        if text.starts_with('[') {
            if let Some(end) = text.strip_prefix("[[").and_then(|rest| rest.find("]]")) {
                let length = end + 4;
                spans.push(Span::styled(
                    text[..length].to_owned(),
                    Style::default()
                        .fg(theme.link.into())
                        .add_modifier(Modifier::UNDERLINED),
                ));
                text = &text[length..];
                continue;
            }
            if let Some(label_end) = text.find("](") {
                if let Some(target_end) = text[label_end + 2..].find(')') {
                    let length = label_end + target_end + 3;
                    spans.push(Span::styled(
                        text[..length].to_owned(),
                        Style::default()
                            .fg(theme.link.into())
                            .add_modifier(Modifier::UNDERLINED),
                    ));
                    text = &text[length..];
                    continue;
                }
            }
        }
        if let Some(end) = text.strip_prefix("<note://").and_then(|_| text.find('>')) {
            let length = end + 1;
            spans.push(Span::styled(
                text[..length].to_owned(),
                Style::default()
                    .fg(theme.link.into())
                    .add_modifier(Modifier::UNDERLINED),
            ));
            text = &text[length..];
            continue;
        }
        let delimiter = if text.starts_with("**") { "**" } else { "*" };
        if let Some(rest) = text.strip_prefix(delimiter) {
            if let Some(end) = rest.find(delimiter) {
                let length = delimiter.len() + end + delimiter.len();
                let modifier = if delimiter.len() == 2 {
                    Modifier::BOLD
                } else {
                    Modifier::ITALIC
                };
                spans.push(Span::styled(
                    text[..length].to_owned(),
                    Style::default().add_modifier(modifier),
                ));
                text = &text[length..];
                continue;
            }
        }
        let length = text.chars().next().map_or(0, char::len_utf8);
        spans.push(Span::raw(text[..length].to_owned()));
        text = &text[length..];
    }
    spans
}

pub fn note_links(source: &str) -> Vec<NoteLink> {
    let mut links = Vec::new();
    let mut offset = 0;
    let mut in_fence = false;
    for line in source.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            parse_line_links(line, offset, &mut links);
        }
        offset += line.len() + 1;
    }
    links
}

fn parse_line_links(line: &str, line_offset: usize, links: &mut Vec<NoteLink>) {
    let mut index = 0;
    while index < line.len() {
        let rest = &line[index..];
        if let Some(code) = rest.strip_prefix('`') {
            index += code.find('`').map_or(1, |end| end + 2);
            continue;
        }
        if let Some((wiki, end)) = rest
            .strip_prefix("[[")
            .and_then(|wiki| wiki.find("]]").map(|end| (wiki, end)))
        {
            let length = end + 4;
            links.push(NoteLink {
                range: line_offset + index..line_offset + index + length,
                target: NoteLinkTarget::Wiki(wiki[..end].to_owned()),
            });
            index += length;
            continue;
        }
        if let Some((label_end, target_end)) = rest.strip_prefix('[').and_then(|_| {
            rest.find("](").and_then(|label_end| {
                rest[label_end + 2..]
                    .find(')')
                    .map(|target_end| (label_end, target_end))
            })
        }) {
            let target_start = label_end + 2;
            let length = target_start + target_end + 1;
            let target = rest[target_start..target_start + target_end]
                .trim()
                .trim_matches(['<', '>']);
            let target_kind = if target.starts_with("note://") {
                Some(NoteLinkTarget::Legacy(target.to_owned()))
            } else if is_note_path(target) {
                Some(NoteLinkTarget::Path(target.to_owned()))
            } else {
                None
            };
            if let Some(target) = target_kind {
                links.push(NoteLink {
                    range: line_offset + index..line_offset + index + length,
                    target,
                });
            }
            index += length;
            continue;
        }
        if let Some(end) = rest
            .strip_prefix("<note://")
            .and_then(|target| target.find('>'))
        {
            let length = end + "<note://>".len();
            links.push(NoteLink {
                range: line_offset + index..line_offset + index + length,
                target: NoteLinkTarget::Legacy(rest[1..length - 1].to_owned()),
            });
            index += length;
            continue;
        }
        index += rest.chars().next().map_or(1, char::len_utf8);
    }
}

fn is_note_path(target: &str) -> bool {
    let path = target.split('#').next().unwrap_or(target);
    ["md", "txt", "markdown"].iter().any(|extension| {
        path.rsplit_once('.')
            .is_some_and(|(_, actual)| actual.eq_ignore_ascii_case(extension))
    })
}

pub fn link_metadata(source: &str, links: &[NoteLink]) -> Text<'static> {
    let mut lines = Vec::new();
    let mut line_start = 0;
    for line in source.split('\n') {
        let line_end = line_start + line.len();
        let mut spans = Vec::new();
        let mut position = line_start;
        for (index, link) in links.iter().enumerate() {
            if link.range.start < line_start || link.range.end > line_end {
                continue;
            }
            if position < link.range.start {
                spans.push(Span::raw(source[position..link.range.start].to_owned()));
            }
            spans.push(Span::styled(
                source[link.range.clone()].to_owned(),
                Style::default().fg(link_id_color(index)),
            ));
            position = link.range.end;
        }
        if position < line_end {
            spans.push(Span::raw(source[position..line_end].to_owned()));
        }
        lines.push(Line::from(spans));
        line_start = line_end + 1;
    }
    Text::from(lines)
}

fn link_id_color(index: usize) -> Color {
    let id = index + 1;
    Color::Rgb((id >> 16) as u8, (id >> 8) as u8, id as u8)
}

pub fn color_link_id(color: Color) -> Option<usize> {
    let Color::Rgb(red, green, blue) = color else {
        return None;
    };
    let id = ((red as usize) << 16) | ((green as usize) << 8) | blue as usize;
    id.checked_sub(1)
}

pub fn heading_source_offset(source: &str, target: &str) -> Option<usize> {
    let target_anchor = heading_anchor(target);
    let mut offset = 0;
    let mut in_fence = false;
    for line in source.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
            if (1..=6).contains(&hashes)
                && trimmed[hashes..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
            {
                let heading = trimmed[hashes..].trim().trim_end_matches('#').trim();
                if heading.eq_ignore_ascii_case(target) || heading_anchor(heading) == target_anchor
                {
                    return Some(offset);
                }
            }
        }
        offset += line.len() + 1;
    }
    None
}

fn heading_anchor(heading: &str) -> String {
    let mut anchor = String::new();
    let mut separator = false;
    for character in heading.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() || character == '_' {
            if separator && !anchor.is_empty() {
                anchor.push('-');
            }
            separator = false;
            anchor.push(character);
        } else if character.is_whitespace() || character == '-' {
            separator = true;
        }
    }
    anchor
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::*;

    #[test]
    fn highlights_structural_and_inline_markdown() {
        let text = highlight(
            "# Heading\n- `code` and [link](target)\n```\nbody\n```",
            &Theme::default(),
        );
        assert_eq!(text.lines.len(), 5);
        assert_eq!(text.lines[0].style.fg, Some(Color::Cyan));
        assert_eq!(text.lines[1].spans[0].style.fg, Some(Color::Yellow));
        assert_eq!(text.lines[3].style.fg, Some(Color::Green));
    }

    #[test]
    fn highlights_a_multiline_selection_over_markdown_styles() {
        let text = highlight_selection(
            "# Heading\nplain `code` text",
            &Theme::default(),
            Some(2..18),
        );

        let selected = text
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .filter(|span| span.style.add_modifier.contains(Modifier::REVERSED))
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert_eq!(selected, "Headingplain `c");
    }

    #[test]
    fn finds_supported_note_links_outside_code() {
        let source = "[relative](folder/Note%20one.md#Part) [[Other#Heading|label]]\n\
                      `[[ignored]]` <note://Legacy_note> [web](https://example.com)";

        let links = note_links(source);

        assert_eq!(links.len(), 3);
        assert_eq!(
            links.iter().map(|link| &link.target).collect::<Vec<_>>(),
            [
                &NoteLinkTarget::Path("folder/Note%20one.md#Part".into()),
                &NoteLinkTarget::Wiki("Other#Heading|label".into()),
                &NoteLinkTarget::Legacy("note://Legacy_note".into()),
            ]
        );
    }

    #[test]
    fn metadata_colors_encode_link_indices() {
        let source = "[[One]] and [[Two]]";
        let links = note_links(source);
        let text = link_metadata(source, &links);

        assert_eq!(
            color_link_id(text.lines[0].spans[0].style.fg.unwrap()),
            Some(0)
        );
        assert_eq!(
            color_link_id(text.lines[0].spans[2].style.fg.unwrap()),
            Some(1)
        );
    }

    #[test]
    fn finds_heading_text_and_markdown_anchor() {
        let source = "# First heading\nbody\n## Release Notes! ##\n";

        assert_eq!(heading_source_offset(source, "First heading"), Some(0));
        assert_eq!(heading_source_offset(source, "release-notes"), Some(21));
        assert_eq!(heading_source_offset(source, "missing"), None);
    }
}
