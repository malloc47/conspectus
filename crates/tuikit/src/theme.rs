//! Design tokens: the colors and modifiers every control draws with.
//!
//! This is a layer 1 foundation. Controls take a theme and style their
//! output from it, so a host changes the look of every control in one
//! place. The style-spec grammar a host's configuration uses to name
//! colors and modifiers lives here too.

mod spec;

pub use spec::{StyleSpec, parse_color, parse_modifier, parse_style_spec};
