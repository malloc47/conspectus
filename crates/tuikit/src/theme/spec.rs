//! The style-spec grammar: the text a host's configuration uses to name a
//! color, a set of modifiers, or both.
//!
//! The grammar is small on purpose, so a configuration file reads the same
//! in every host:
//!
//! - a color is a named ANSI color (`red`, `bright_green`, `dark_gray`),
//!   `default` or `reset` for the terminal's own color, `ansi256:N` for an
//!   indexed color, or `#RRGGBB` for a true color;
//! - a modifier list is comma-joined names (`bold`, `dim,italic`);
//! - a style spec is a color, a modifier list, or a color followed by
//!   modifiers (`green,bold`).
//!
//! Names are matched without regard to case, and whitespace around each
//! token is ignored. Every parser returns the reason it rejected a value,
//! so a host can show it next to the key that carried it.

use ratatui::style::{Color, Modifier, Style};

/// A color and modifier pair, the value of a style token.
///
/// When `color` is `None` the text keeps the color it would otherwise have;
/// when `modifier` is empty nothing is added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleSpec {
    /// The foreground color, if the spec sets one.
    pub color: Option<Color>,
    /// The modifiers the spec adds.
    pub modifier: Modifier,
}

impl StyleSpec {
    /// A spec that sets only a foreground color.
    pub const fn fg(color: Color) -> Self {
        Self {
            color: Some(color),
            modifier: Modifier::empty(),
        }
    }

    /// A spec that adds only modifiers.
    pub const fn modifier(modifier: Modifier) -> Self {
        Self {
            color: None,
            modifier,
        }
    }

    /// A spec that sets a foreground color and adds modifiers.
    pub const fn fg_mod(color: Color, modifier: Modifier) -> Self {
        Self {
            color: Some(color),
            modifier,
        }
    }

    /// The spec as a ratatui [`Style`], ready to draw a span with.
    pub fn into_style(self) -> Style {
        let mut style = Style::default();
        if let Some(color) = self.color {
            style = style.fg(color);
        }
        if !self.modifier.is_empty() {
            style = style.add_modifier(self.modifier);
        }
        style
    }
}

/// Parses one color: a named ANSI color, `ansi256:N`, or `#RRGGBB`.
///
/// # Errors
///
/// Returns the reason when `raw` is empty or names no color.
pub fn parse_color(raw: &str) -> Result<Color, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty color value".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if let Some(color) = parse_named_color(&lower) {
        return Ok(color);
    }
    if let Some(idx) = lower.strip_prefix("ansi256:") {
        return idx
            .trim()
            .parse::<u8>()
            .map(Color::Indexed)
            .map_err(|_| format!("`{trimmed}`: expected `ansi256:N` with N in 0..=255"));
    }
    if let Some(hex) = lower.strip_prefix('#') {
        return parse_hex_color(hex)
            .ok_or_else(|| format!("`{trimmed}`: expected `#RRGGBB` hex color"));
    }
    Err(format!(
        "`{trimmed}` is not a recognized color (try a named color, `ansi256:N`, or `#RRGGBB`)"
    ))
}

fn parse_named_color(lower: &str) -> Option<Color> {
    Some(match lower {
        "default" | "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" | "dark_gray" | "dark_grey" => Color::DarkGray,
        "bright_black" => Color::DarkGray,
        "bright_red" => Color::LightRed,
        "bright_green" => Color::LightGreen,
        "bright_yellow" => Color::LightYellow,
        "bright_blue" => Color::LightBlue,
        "bright_magenta" => Color::LightMagenta,
        "bright_cyan" => Color::LightCyan,
        "bright_white" => Color::White,
        _ => return None,
    })
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// Parses a comma-joined modifier list: `"bold,italic"` is
/// `BOLD | ITALIC`. An empty list is the empty modifier set.
///
/// # Errors
///
/// Returns the reason when a token names no modifier.
pub fn parse_modifier(raw: &str) -> Result<Modifier, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Modifier::empty());
    }
    let mut acc = Modifier::empty();
    for token in trimmed.split(',') {
        let token = token.trim().to_ascii_lowercase();
        let bit = match token.as_str() {
            "bold" => Modifier::BOLD,
            "dim" => Modifier::DIM,
            "italic" => Modifier::ITALIC,
            "underline" | "underlined" => Modifier::UNDERLINED,
            "reversed" | "reverse" => Modifier::REVERSED,
            "crossed_out" | "strikethrough" => Modifier::CROSSED_OUT,
            "slow_blink" => Modifier::SLOW_BLINK,
            "rapid_blink" => Modifier::RAPID_BLINK,
            "hidden" => Modifier::HIDDEN,
            "" => continue,
            other => {
                return Err(format!(
                    "`{other}` is not a recognized modifier (expected bold, dim, italic, \
                         underline, reversed, crossed_out)"
                ));
            }
        };
        acc |= bit;
    }
    Ok(acc)
}

/// Parses a style spec: `"color"`, `"color,mod1,mod2"`, or `"mod1,mod2"`.
/// The first comma-separated token may be a color or a modifier; every
/// later token must be a modifier.
///
/// # Errors
///
/// Returns the reason when a token is neither a color in first place nor
/// a modifier.
pub fn parse_style_spec(raw: &str) -> Result<StyleSpec, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(StyleSpec {
            color: None,
            modifier: Modifier::empty(),
        });
    }
    let mut parts = trimmed.split(',').map(str::trim);
    let first = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();

    // A first token that is not a color falls through to modifier-only
    // parsing, so `"bold"` and `"dim,italic"` work without a color.
    if let Ok(color) = parse_color(first) {
        let modifier = if rest.is_empty() {
            Modifier::empty()
        } else {
            parse_modifier(&rest.join(","))?
        };
        return Ok(StyleSpec {
            color: Some(color),
            modifier,
        });
    }
    let modifier = parse_modifier(trimmed)?;
    Ok(StyleSpec {
        color: None,
        modifier,
    })
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
