//! The style-spec grammar: the text a host's configuration uses to name a
//! color, a set of modifiers, or both.
//!
//! The grammar is small on purpose, so a configuration file reads the same
//! in every host:
//!
//! - a color is a named ANSI color (`red`, `bright_green`, `dark_gray`,
//!   `light_gray`), `default` or `reset` for the terminal's own color,
//!   `ansi256:N` for an indexed color, or `#RRGGBB` for a true color;
//! - a modifier list is comma-joined names (`bold`, `dim,italic`);
//! - a style spec is colors, a modifier list, or colors followed by
//!   modifiers. Its colors are a foreground (`green`), a foreground on a
//!   background (`white on dark_gray`), or a background alone
//!   (`on dark_gray`): `black on white,bold`, `green,bold`, `dim`.
//!
//! Names are matched without regard to case, and whitespace around each
//! token is ignored. Every parser returns the reason it rejected a value,
//! so a host can show it next to the key that carried it.

use ratatui::style::{Color, Modifier, Style};

/// Colors and modifiers together, the value of a style token.
///
/// A color left `None` keeps the color the text would otherwise have; an
/// empty `modifier` adds nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StyleSpec {
    /// The foreground color, if the spec sets one.
    pub fg: Option<Color>,
    /// The background color, if the spec sets one.
    pub bg: Option<Color>,
    /// The modifiers the spec adds.
    pub modifier: Modifier,
}

impl StyleSpec {
    /// A spec that sets only a foreground color.
    pub const fn fg(color: Color) -> Self {
        Self {
            fg: Some(color),
            bg: None,
            modifier: Modifier::empty(),
        }
    }

    /// A spec that adds only modifiers.
    pub const fn modifier(modifier: Modifier) -> Self {
        Self {
            fg: None,
            bg: None,
            modifier,
        }
    }

    /// A spec that sets a foreground color and adds modifiers.
    pub const fn fg_mod(color: Color, modifier: Modifier) -> Self {
        Self {
            fg: Some(color),
            bg: None,
            modifier,
        }
    }

    /// A spec that sets a foreground color on a background color.
    pub const fn on(fg: Color, bg: Color) -> Self {
        Self {
            fg: Some(fg),
            bg: Some(bg),
            modifier: Modifier::empty(),
        }
    }

    /// The spec as a ratatui [`Style`], ready to draw a span with.
    pub fn into_style(self) -> Style {
        let mut style = Style::default();
        if let Some(color) = self.fg {
            style = style.fg(color);
        }
        if let Some(color) = self.bg {
            style = style.bg(color);
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
        "light_gray" | "light_grey" => Color::Gray,
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

/// Parses a style spec: colors, colors followed by modifiers, or modifiers
/// alone, comma-joined. The first token may be colors (`fg`, `fg on bg`,
/// or `on bg`) or a modifier; every later token must be a modifier.
///
/// # Errors
///
/// Returns the reason when the first token is neither colors nor a
/// modifier, or a later token is not a modifier.
pub fn parse_style_spec(raw: &str) -> Result<StyleSpec, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(StyleSpec::default());
    }
    let mut parts = trimmed.split(',').map(str::trim);
    let first = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();

    // A first token that is not colors falls through to modifier-only
    // parsing, so `"bold"` and `"dim,italic"` work without a color.
    let colors = match parse_colors(first) {
        Some(colors) => colors?,
        None => {
            return Ok(StyleSpec {
                modifier: parse_modifier(trimmed)?,
                ..StyleSpec::default()
            });
        }
    };
    let modifier = if rest.is_empty() {
        Modifier::empty()
    } else {
        parse_modifier(&rest.join(","))?
    };
    Ok(StyleSpec {
        fg: colors.0,
        bg: colors.1,
        modifier,
    })
}

/// A foreground and a background, either of them optional.
type Colors = (Option<Color>, Option<Color>);

/// The foreground and background named by `token`: `fg`, `fg on bg`, or
/// `on bg`. `None` when the token is not colors at all (it may be a
/// modifier); `Some(Err)` when it uses `on` but a side names no color.
fn parse_colors(token: &str) -> Option<Result<Colors, String>> {
    let words: Vec<&str> = token.split_whitespace().collect();
    let is_on = |word: &str| word.eq_ignore_ascii_case("on");
    match words.as_slice() {
        [fg] => parse_color(fg).ok().map(|fg| Ok((Some(fg), None))),
        [on, bg] if is_on(on) => Some(parse_color(bg).map(|bg| (None, Some(bg)))),
        [fg, on, bg] if is_on(on) => {
            Some(parse_color(fg).and_then(|fg| parse_color(bg).map(|bg| (Some(fg), Some(bg)))))
        }
        _ if words.iter().any(|word| is_on(word)) => Some(Err(format!(
            "`{token}`: expected `fg on bg`, `on bg`, or one color"
        ))),
        _ => None,
    }
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
