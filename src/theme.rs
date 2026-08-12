use std::{fmt, fs, path::PathBuf};

use anyhow::Context;
use directories::ProjectDirs;
use ratatui::style::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThemeColor(Color);

impl From<ThemeColor> for Color {
    fn from(value: ThemeColor) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_color(&value)
            .map(Self)
            .ok_or_else(|| de::Error::custom(format!("invalid theme color {value:?}")))
    }
}

impl Serialize for ThemeColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl fmt::Display for ThemeColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Color::Reset => formatter.write_str("reset"),
            Color::Black => formatter.write_str("black"),
            Color::Red => formatter.write_str("red"),
            Color::Green => formatter.write_str("green"),
            Color::Yellow => formatter.write_str("yellow"),
            Color::Blue => formatter.write_str("blue"),
            Color::Magenta => formatter.write_str("magenta"),
            Color::Cyan => formatter.write_str("cyan"),
            Color::Gray => formatter.write_str("gray"),
            Color::DarkGray => formatter.write_str("dark-gray"),
            Color::LightRed => formatter.write_str("light-red"),
            Color::LightGreen => formatter.write_str("light-green"),
            Color::LightYellow => formatter.write_str("light-yellow"),
            Color::LightBlue => formatter.write_str("light-blue"),
            Color::LightMagenta => formatter.write_str("light-magenta"),
            Color::LightCyan => formatter.write_str("light-cyan"),
            Color::White => formatter.write_str("white"),
            Color::Rgb(red, green, blue) => write!(formatter, "#{red:02x}{green:02x}{blue:02x}"),
            Color::Indexed(index) => write!(formatter, "indexed-{index}"),
        }
    }
}

const fn color(value: Color) -> ThemeColor {
    ThemeColor(value)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub background: ThemeColor,
    pub foreground: ThemeColor,
    pub muted: ThemeColor,
    pub accent: ThemeColor,
    pub accent_foreground: ThemeColor,
    pub success: ThemeColor,
    pub warning: ThemeColor,
    pub error: ThemeColor,
    pub heading: ThemeColor,
    pub quote: ThemeColor,
    pub code: ThemeColor,
    pub link: ThemeColor,
    pub fence: ThemeColor,
    pub field_background: ThemeColor,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: color(Color::Reset),
            foreground: color(Color::Reset),
            muted: color(Color::DarkGray),
            accent: color(Color::Cyan),
            accent_foreground: color(Color::Black),
            success: color(Color::Green),
            warning: color(Color::Yellow),
            error: color(Color::Red),
            heading: color(Color::Cyan),
            quote: color(Color::Blue),
            code: color(Color::Green),
            link: color(Color::LightBlue),
            fence: color(Color::Magenta),
            field_background: color(Color::White),
        }
    }
}

impl Theme {
    pub fn load() -> anyhow::Result<Self> {
        let Some(path) = theme_path() else {
            return Ok(Self::default());
        };
        match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents)
                .with_context(|| format!("invalid theme file {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
        }
    }
}

fn theme_path() -> Option<PathBuf> {
    Some(
        ProjectDirs::from("org", "QOwnNotes", "qownnotes-tui")?
            .config_dir()
            .join("theme.toml"),
    )
}

fn parse_color(value: &str) -> Option<Color> {
    let normalized = value.to_ascii_lowercase();
    let named = match normalized.as_str() {
        "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "dark-gray" | "dark-grey" => Color::DarkGray,
        "light-red" => Color::LightRed,
        "light-green" => Color::LightGreen,
        "light-yellow" => Color::LightYellow,
        "light-blue" => Color::LightBlue,
        "light-magenta" => Color::LightMagenta,
        "light-cyan" => Color::LightCyan,
        "white" => Color::White,
        _ => {
            let hex = normalized.strip_prefix('#')?;
            if hex.len() != 6 {
                return None;
            }
            return Some(Color::Rgb(
                u8::from_str_radix(&hex[0..2], 16).ok()?,
                u8::from_str_radix(&hex[2..4], 16).ok()?,
                u8::from_str_radix(&hex[4..6], 16).ok()?,
            ));
        }
    };
    Some(named)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_partial_theme_with_hex_and_named_colors() {
        let theme: Theme =
            toml::from_str("accent = '#89b4fa'\nforeground = 'white'\nbackground = 'reset'\n")
                .unwrap();

        assert_eq!(Color::from(theme.accent), Color::Rgb(0x89, 0xb4, 0xfa));
        assert_eq!(Color::from(theme.foreground), Color::White);
        assert_eq!(Color::from(theme.warning), Color::Yellow);
    }

    #[test]
    fn rejects_invalid_theme_colors() {
        assert!(toml::from_str::<Theme>("accent = '#12345'\n").is_err());
        assert!(toml::from_str::<Theme>("accent = 'purple'\n").is_err());
    }
}
