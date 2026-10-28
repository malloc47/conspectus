use super::*;

#[test]
fn style_spec_into_style_combines_color_and_modifier() {
    let style = StyleSpec::fg_mod(Color::Green, Modifier::BOLD).into_style();
    assert_eq!(style.fg, Some(Color::Green));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn style_spec_modifier_only_omits_fg() {
    let style = StyleSpec::modifier(Modifier::DIM).into_style();
    assert_eq!(style.fg, None);
    assert!(style.add_modifier.contains(Modifier::DIM));
}

#[test]
fn parse_color_accepts_named_ansi() {
    assert_eq!(parse_color("magenta").unwrap(), Color::Magenta);
    assert_eq!(parse_color("Bright_Green").unwrap(), Color::LightGreen);
    assert_eq!(parse_color("default").unwrap(), Color::Reset);
    assert_eq!(parse_color(" grey ").unwrap(), Color::DarkGray);
}

#[test]
fn parse_color_accepts_ansi256_and_hex() {
    assert_eq!(parse_color("ansi256:42").unwrap(), Color::Indexed(42));
    assert_eq!(
        parse_color("#ff8800").unwrap(),
        Color::Rgb(0xff, 0x88, 0x00)
    );
    assert_eq!(parse_color("#FFFFFF").unwrap(), Color::Rgb(255, 255, 255));
}

#[test]
fn parse_color_rejects_garbage() {
    assert!(parse_color("").is_err());
    assert!(parse_color("not-a-color").is_err());
    assert!(parse_color("ansi256:999").is_err());
    assert!(parse_color("#zzzzzz").is_err());
    assert!(parse_color("#abc").is_err());
}

#[test]
fn parse_modifier_accepts_comma_list() {
    let m = parse_modifier("bold,italic").unwrap();
    assert!(m.contains(Modifier::BOLD));
    assert!(m.contains(Modifier::ITALIC));
    assert_eq!(parse_modifier("").unwrap(), Modifier::empty());
    assert_eq!(
        parse_modifier("reverse").unwrap(),
        Modifier::REVERSED,
        "`reverse` is an accepted alias for `reversed`"
    );
}

#[test]
fn parse_modifier_rejects_unknown_token() {
    assert!(parse_modifier("bold,not-a-mod").is_err());
}

#[test]
fn parse_style_spec_handles_color_plus_modifier() {
    let spec = parse_style_spec("green,bold").unwrap();
    assert_eq!(spec.color, Some(Color::Green));
    assert!(spec.modifier.contains(Modifier::BOLD));
}

#[test]
fn parse_style_spec_handles_modifier_only() {
    let spec = parse_style_spec("dim").unwrap();
    assert_eq!(spec.color, None);
    assert!(spec.modifier.contains(Modifier::DIM));
}

#[test]
fn parse_style_spec_handles_color_only() {
    let spec = parse_style_spec("#ff8800").unwrap();
    assert_eq!(spec.color, Some(Color::Rgb(0xff, 0x88, 0x00)));
    assert_eq!(spec.modifier, Modifier::empty());
}

#[test]
fn parse_style_spec_rejects_a_modifier_it_does_not_know() {
    assert!(parse_style_spec("green,shiny").is_err());
}
