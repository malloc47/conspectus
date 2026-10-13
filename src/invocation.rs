//! The name Conspectus was run as (ADR 0118): `conspectus`, or `csp`, its
//! sanctioned short name. Messages that suggest a follow-up command use
//! it, so the suggestion reads the way the operator types.

use std::ffi::OsStr;
use std::path::Path;

/// The full name, which docs and log prefixes always use.
pub const FULL_NAME: &str = "conspectus";

/// The sanctioned short name, installed alongside the full one.
pub const SHORT_NAME: &str = "csp";

/// [`SHORT_NAME`] when this process was started under it, otherwise
/// [`FULL_NAME`]. A private alias or a test harness gets the full name.
pub fn invoked_name() -> &'static str {
    name_for(std::env::args_os().next().as_deref())
}

fn name_for(argv0: Option<&OsStr>) -> &'static str {
    let short = argv0
        .and_then(|argv0| Path::new(argv0).file_stem())
        .is_some_and(|stem| stem == SHORT_NAME);
    if short { SHORT_NAME } else { FULL_NAME }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_short_name_is_recognized_on_any_path() {
        for argv0 in ["csp", "/home/u/.cargo/bin/csp", "csp.exe"] {
            assert_eq!(name_for(Some(OsStr::new(argv0))), "csp", "{argv0}");
        }
    }

    #[test]
    fn every_other_name_is_the_full_name() {
        for argv0 in [
            "conspectus",
            "/usr/bin/conspectus",
            "cs",
            "cspx",
            "conspectus-0a1b2c",
        ] {
            assert_eq!(name_for(Some(OsStr::new(argv0))), "conspectus", "{argv0}");
        }
        assert_eq!(name_for(None), "conspectus");
    }
}
