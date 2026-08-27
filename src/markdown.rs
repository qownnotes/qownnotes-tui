use std::ops::Range;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::theme::Theme;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoteLinkTarget {
    Path(String),
    Legacy(String),
    Wiki(String),
    Uri(String),
    SourceOffset(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteLink {
    pub range: Range<usize>,
    pub target: NoteLinkTarget,
}

pub fn highlight(source: &str, theme: &Theme) -> Text<'static> {
    let mut in_fence = false;
    let mut previous_is_setext_title = false;
    let source_lines = source.split('\n').collect::<Vec<_>>();
    let lines = source_lines
        .iter()
        .enumerate()
        .map(|(index, &line)| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
                previous_is_setext_title = false;
                return Line::styled(
                    line.to_owned(),
                    Style::default()
                        .fg(theme.fence.into())
                        .add_modifier(Modifier::BOLD),
                );
            }
            if in_fence {
                previous_is_setext_title = false;
                return Line::styled(line.to_owned(), Style::default().fg(theme.code.into()));
            }
            let is_setext_title = is_setext_title(line)
                && source_lines
                    .get(index + 1)
                    .is_some_and(|line| is_setext_underline(line));
            let is_setext_heading = previous_is_setext_title || is_setext_title;
            previous_is_setext_title = is_setext_title;
            if is_setext_heading {
                return Line::styled(
                    line.to_owned(),
                    Style::default()
                        .fg(theme.heading.into())
                        .add_modifier(Modifier::BOLD),
                );
            }
            if is_atx_heading(trimmed) {
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
                let rest = if let Some((checkbox_end, state)) = checkbox_marker(rest) {
                    let (checkbox, rest) = rest.split_at(checkbox_end);
                    let color = match state {
                        CheckboxState::Unchecked => theme.warning,
                        CheckboxState::Partial => theme.muted,
                        CheckboxState::Checked => theme.success,
                    };
                    spans.push(Span::styled(
                        checkbox.to_owned(),
                        Style::default()
                            .fg(color.into())
                            .add_modifier(Modifier::BOLD),
                    ));
                    rest
                } else {
                    rest
                };
                spans.extend(highlight_inline(rest, theme));
                return Line::from(spans);
            }
            Line::from(highlight_inline(line, theme))
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn is_setext_title(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.is_empty()
        && !is_setext_underline(line)
        && !is_atx_heading(trimmed)
        && !trimmed.starts_with('>')
        && list_marker_end(line).is_none()
}

fn is_atx_heading(trimmed: &str) -> bool {
    trimmed.starts_with('#')
        && trimmed
            .trim_start_matches('#')
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
}

fn is_setext_underline(line: &str) -> bool {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    if indent > 3 {
        return false;
    }
    let marker = line[indent..].trim_end_matches([' ', '\t']);
    !marker.is_empty()
        && (marker.bytes().all(|byte| byte == b'=') || marker.bytes().all(|byte| byte == b'-'))
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

pub fn selection_metadata(source: &str) -> Text<'static> {
    let mut line_offset = 0;
    let lines = source
        .split('\n')
        .map(|line| {
            let spans = line
                .grapheme_indices(true)
                .filter_map(|(offset, grapheme)| {
                    let start = line_offset + offset;
                    let end = start + grapheme.len();
                    Some(Span::styled(
                        grapheme.to_owned(),
                        Style::default()
                            .fg(encode_source_offset(start)?)
                            .bg(encode_source_offset(end)?),
                    ))
                })
                .collect::<Vec<_>>();
            line_offset += line.len() + 1;
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

pub fn decode_source_offset(color: Color) -> Option<usize> {
    let Color::Rgb(red, green, blue) = color else {
        return None;
    };
    let encoded = (usize::from(red) << 16) | (usize::from(green) << 8) | usize::from(blue);
    encoded.checked_sub(1)
}

fn encode_source_offset(offset: usize) -> Option<Color> {
    let encoded = offset.checked_add(1)?;
    (encoded <= 0x00ff_ffff).then_some({
        Color::Rgb(
            ((encoded >> 16) & 0xff) as u8,
            ((encoded >> 8) & 0xff) as u8,
            (encoded & 0xff) as u8,
        )
    })
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

enum CheckboxState {
    Unchecked,
    Partial,
    Checked,
}

fn checkbox_marker(text: &str) -> Option<(usize, CheckboxState)> {
    let (state, rest) = if let Some(rest) = text.strip_prefix("[ ]") {
        (CheckboxState::Unchecked, rest)
    } else if let Some(rest) = text.strip_prefix("[-]") {
        (CheckboxState::Partial, rest)
    } else {
        let rest = text
            .strip_prefix("[x]")
            .or_else(|| text.strip_prefix("[X]"))?;
        (CheckboxState::Checked, rest)
    };
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then_some((3, state))
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
            if let Some(length) = footnote_label_length(text) {
                spans.push(Span::styled(
                    text[..length].to_owned(),
                    Style::default()
                        .fg(theme.link.into())
                        .add_modifier(Modifier::UNDERLINED),
                ));
                text = &text[length..];
                continue;
            }
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
        if let Some(length) = markdown_autolink_length(text) {
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

fn markdown_autolink_length(text: &str) -> Option<usize> {
    let destination = text.strip_prefix('<')?;
    let end = destination.find('>')?;
    let destination = &destination[..end];
    let (scheme, _) = destination.split_once(':')?;
    let valid_scheme = (2..=32).contains(&scheme.len())
        && scheme.starts_with(|character: char| character.is_ascii_alphabetic())
        && scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        });
    let valid_destination = destination.chars().all(|character| {
        !character.is_ascii_control() && !character.is_ascii_whitespace() && character != '<'
    });
    (valid_scheme && valid_destination).then_some(end + 2)
}

pub fn note_links(source: &str) -> Vec<NoteLink> {
    let definitions = footnote_definitions(source);
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
    links.extend(footnote_links(source, &definitions));
    links.sort_by_key(|link| link.range.start);
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
            } else if is_web_uri(target) {
                Some(NoteLinkTarget::Uri(target.to_owned()))
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
        if let Some(length) = markdown_autolink_length(rest) {
            let target = &rest[1..length - 1];
            if is_web_uri(target) {
                links.push(NoteLink {
                    range: line_offset + index..line_offset + index + length,
                    target: NoteLinkTarget::Uri(target.to_owned()),
                });
            }
            index += length;
            continue;
        }
        index += rest.chars().next().map_or(1, char::len_utf8);
    }
}

fn footnote_label_length(text: &str) -> Option<usize> {
    let label = text.strip_prefix("[^")?;
    let end = label.find(']')?;
    (end > 0 && !label[..end].contains(['\r', '\n'])).then_some(end + 3)
}

fn footnote_definitions(source: &str) -> Vec<(String, Range<usize>)> {
    let mut definitions = Vec::new();
    let mut offset = 0;
    let mut in_fence = false;
    for line in source.split('\n') {
        let trimmed = line.trim_start_matches([' ', '\t']);
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            let indent = line.len() - trimmed.len();
            if indent <= 3 {
                if let Some(length) = footnote_label_length(trimmed) {
                    if trimmed[length..].starts_with(':') {
                        definitions.push((
                            trimmed[2..length - 1].to_owned(),
                            offset + indent..offset + indent + length,
                        ));
                    }
                }
            }
        }
        offset += line.len() + 1;
    }
    definitions
}

fn footnote_links(source: &str, definitions: &[(String, Range<usize>)]) -> Vec<NoteLink> {
    let mut references = Vec::new();
    let mut offset = 0;
    let mut in_fence = false;
    for line in source.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            let mut index = 0;
            while index < line.len() {
                let rest = &line[index..];
                if let Some(code) = rest.strip_prefix('`') {
                    index += code.find('`').map_or(1, |end| end + 2);
                    continue;
                }
                if let Some(length) = footnote_label_length(rest) {
                    let range = offset + index..offset + index + length;
                    let label = &rest[2..length - 1];
                    if !rest[length..].starts_with(':')
                        && definitions
                            .iter()
                            .any(|(definition, _)| definition == label)
                    {
                        references.push((label.to_owned(), range));
                    }
                    index += length;
                    continue;
                }
                index += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
        offset += line.len() + 1;
    }

    let mut links = references
        .iter()
        .filter_map(|(label, range)| {
            let (_, definition) = definitions
                .iter()
                .find(|(definition, _)| definition == label)?;
            Some(NoteLink {
                range: range.clone(),
                target: NoteLinkTarget::SourceOffset(definition.start),
            })
        })
        .collect::<Vec<_>>();
    links.extend(definitions.iter().filter_map(|(label, range)| {
        let (_, reference) = references
            .iter()
            .find(|(reference, _)| reference == label)?;
        Some(NoteLink {
            range: range.clone(),
            target: NoteLinkTarget::SourceOffset(reference.start),
        })
    }));
    links
}

fn is_web_uri(target: &str) -> bool {
    target.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
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
    fn highlights_setext_headings() {
        let text = highlight(
            "First level\n============\nSecond level\n---\n#Not ATX\n===",
            &Theme::default(),
        );

        for line in &text.lines {
            assert_eq!(line.style.fg, Some(Color::Cyan));
            assert!(line.style.add_modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn does_not_highlight_setext_like_lines_outside_paragraphs() {
        let text = highlight(
            "---\n    Indented\n    ===\n```\nCode heading\n---\n```",
            &Theme::default(),
        );

        assert_eq!(text.lines[0].style.fg, None);
        assert_eq!(text.lines[1].style.fg, None);
        assert_eq!(text.lines[2].style.fg, None);
        assert_eq!(text.lines[4].style.fg, Some(Color::Green));
        assert_eq!(text.lines[5].style.fg, Some(Color::Green));
    }

    #[test]
    fn highlights_checkbox_list_markers() {
        let text = highlight(
            "- [ ] pending `task`\n* [x] complete\n1. [X] complete\n- [-] partial\n- [no] plain",
            &Theme::default(),
        );

        assert_eq!(text.lines[0].spans[0].content, "- ");
        assert_eq!(text.lines[0].spans[1].content, "[ ]");
        assert_eq!(text.lines[0].spans[1].style.fg, Some(Color::Yellow));
        assert_eq!(text.lines[0].spans[3].content, "`task`");
        assert_eq!(text.lines[0].spans[3].style.fg, Some(Color::Green));
        assert_eq!(text.lines[1].spans[1].content, "[x]");
        assert_eq!(text.lines[1].spans[1].style.fg, Some(Color::Green));
        assert_eq!(text.lines[2].spans[1].content, "[X]");
        assert_eq!(text.lines[2].spans[1].style.fg, Some(Color::Green));
        assert_eq!(text.lines[3].spans[1].content, "[-]");
        assert_eq!(text.lines[3].spans[1].style.fg, Some(Color::DarkGray));
        assert_eq!(
            text.lines[4]
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            "- [no] plain"
        );
        assert!(
            text.lines[4].spans[1..]
                .iter()
                .all(|span| span.style.fg.is_none())
        );
    }

    #[test]
    fn highlights_markdown_uri_autolinks() {
        let text = highlight(
            "See <https://github.com/pbek/QOwnNotes/issues/3690> and <span>.",
            &Theme::default(),
        );

        assert_eq!(
            text.lines[0].spans[1].content,
            "<https://github.com/pbek/QOwnNotes/issues/3690>"
        );
        assert_eq!(text.lines[0].spans[1].style.fg, Some(Color::LightBlue));
        assert!(
            text.lines[0].spans[1]
                .style
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
        assert_eq!(text.lines[0].spans[3].content, "<");
        assert_eq!(text.lines[0].spans[3].style.fg, None);
    }

    #[test]
    fn finds_http_links_with_encoded_query_parameters() {
        let source = "- [ ] all issues sorted by likes: <https://github.com/pbek/QOwnNotes/issues?q=is%3Aissue%20state%3Aopen%20sort%3Areactions-%2B1-desc>\n\
                      - [x] bugs: <https://github.com/pbek/QOwnNotes/issues?q=state%3Aopen%20label%3A%22Type%3A%20Bug%22>";

        let links = note_links(source);

        assert_eq!(
            links.iter().map(|link| &link.target).collect::<Vec<_>>(),
            [
                &NoteLinkTarget::Uri("https://github.com/pbek/QOwnNotes/issues?q=is%3Aissue%20state%3Aopen%20sort%3Areactions-%2B1-desc".into()),
                &NoteLinkTarget::Uri("https://github.com/pbek/QOwnNotes/issues?q=state%3Aopen%20label%3A%22Type%3A%20Bug%22".into()),
            ]
        );
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
    fn selection_metadata_tracks_grapheme_byte_ranges() {
        let text = selection_metadata("a\u{301}b\n");

        let first = &text.lines[0].spans[0];
        assert_eq!(decode_source_offset(first.style.fg.unwrap()), Some(0));
        assert_eq!(decode_source_offset(first.style.bg.unwrap()), Some(3));
        let second = &text.lines[0].spans[1];
        assert_eq!(decode_source_offset(second.style.fg.unwrap()), Some(3));
        assert_eq!(decode_source_offset(second.style.bg.unwrap()), Some(4));
        assert!(
            text.lines[1].spans.is_empty(),
            "empty source lines must stay empty so wrapping matches the rendered text"
        );
    }

    #[test]
    fn finds_supported_note_links_outside_code() {
        let source = "[relative](folder/Note%20one.md#Part) [[Other#Heading|label]]\n\
                      `[[ignored]]` <note://Legacy_note> [web](https://example.com)";

        let links = note_links(source);

        assert_eq!(links.len(), 4);
        assert_eq!(
            links.iter().map(|link| &link.target).collect::<Vec<_>>(),
            [
                &NoteLinkTarget::Path("folder/Note%20one.md#Part".into()),
                &NoteLinkTarget::Wiki("Other#Heading|label".into()),
                &NoteLinkTarget::Legacy("note://Legacy_note".into()),
                &NoteLinkTarget::Uri("https://example.com".into()),
            ]
        );
    }

    #[test]
    fn highlights_and_links_numeric_and_named_footnotes() {
        let source =
            "Text[^1] and again[^1]. Named[^source].\n\n[^1]: Number\n[^source]: Explanation";
        let text = highlight(source, &Theme::default());
        let links = note_links(source);

        let footnote_spans = text
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .filter(|span| span.content.starts_with("[^") && span.content.ends_with(']'))
            .collect::<Vec<_>>();
        assert_eq!(footnote_spans.len(), 5);
        assert!(footnote_spans.iter().all(|span| {
            span.style.fg == Some(Color::LightBlue)
                && span.style.add_modifier.contains(Modifier::UNDERLINED)
        }));
        assert_eq!(links.len(), 5);
        assert_eq!(links[0].target, NoteLinkTarget::SourceOffset(41));
        assert_eq!(links[1].target, NoteLinkTarget::SourceOffset(41));
        assert_eq!(links[2].target, NoteLinkTarget::SourceOffset(54));
        assert_eq!(links[3].target, NoteLinkTarget::SourceOffset(4));
        assert_eq!(links[4].target, NoteLinkTarget::SourceOffset(29));
    }

    #[test]
    fn ignores_undefined_and_code_footnotes() {
        let source = "Undefined[^missing] Text[^source] `Code[^source]`\n```\nFenced[^source]\n```\n[^source]: Explanation";

        let links = note_links(source);

        assert_eq!(links.len(), 2);
        assert_eq!(&source[links[0].range.clone()], "[^source]");
        assert_eq!(links[0].target, NoteLinkTarget::SourceOffset(74));
        assert_eq!(links[1].target, NoteLinkTarget::SourceOffset(24));
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
