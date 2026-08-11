use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

pub fn highlight(source: &str) -> Text<'static> {
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
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                );
            }
            if in_fence {
                return Line::styled(line.to_owned(), Style::default().fg(Color::Green));
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
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            }
            if trimmed.starts_with('>') {
                return Line::styled(
                    line.to_owned(),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::ITALIC),
                );
            }
            if let Some(marker_end) = list_marker_end(line) {
                let (marker, rest) = line.split_at(marker_end);
                let mut spans = vec![Span::styled(
                    marker.to_owned(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )];
                spans.extend(highlight_inline(rest));
                return Line::from(spans);
            }
            Line::from(highlight_inline(line))
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

fn highlight_inline(mut text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    while !text.is_empty() {
        let next = ['`', '[', '*']
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
                    Style::default().fg(Color::Green),
                ));
                text = &text[length..];
                continue;
            }
        }
        if text.starts_with('[') {
            if let Some(label_end) = text.find("](") {
                if let Some(target_end) = text[label_end + 2..].find(')') {
                    let length = label_end + target_end + 3;
                    spans.push(Span::styled(
                        text[..length].to_owned(),
                        Style::default()
                            .fg(Color::LightBlue)
                            .add_modifier(Modifier::UNDERLINED),
                    ));
                    text = &text[length..];
                    continue;
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_structural_and_inline_markdown() {
        let text = highlight("# Heading\n- `code` and [link](target)\n```\nbody\n```");
        assert_eq!(text.lines.len(), 5);
        assert_eq!(text.lines[0].style.fg, Some(Color::Cyan));
        assert_eq!(text.lines[1].spans[0].style.fg, Some(Color::Yellow));
        assert_eq!(text.lines[3].style.fg, Some(Color::Green));
    }
}
