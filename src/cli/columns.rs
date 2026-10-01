//! `conspectus columns <ROWS>` — lists the column registry
//! for a projection.
//!
//! Shared helpers (`ColorFlag`, `resolve_color_from_env`,
//! `PagerOptions`, `print_paged`) stay in `super` at
//! `pub(super)` so this module reaches them without
//! introducing a duplicate surface.

use std::io::{self, IsTerminal};

use anyhow::Result;
use clap::Args;

use crate::config;

use super::{ColorFlag, PagerOptions, print_paged, resolve_color_from_env};

#[derive(Debug, Args)]
pub(super) struct ColumnsArgs {
    /// Row-type whose registered columns to list. Accepts the same
    /// tokens as `conspectus table <ROWS>` (sessions, mux, union,
    /// prs, forks).
    row_type: String,
    /// Skip the pager even when stdout is a TTY.
    #[arg(long)]
    no_pager: bool,
    /// Force output through a pager even when stdout is not a TTY.
    #[arg(long, conflicts_with = "no_pager")]
    pager: bool,
    /// When to colorize the output. `auto` (default) emits ANSI only
    /// when stdout is a TTY (and respects `NO_COLOR`, `CLICOLOR`,
    /// `CLICOLOR_FORCE`, `TERM=dumb`); `always` forces color on;
    /// `never` forces it off.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

impl ColumnsArgs {
    pub(super) fn run(self) -> Result<()> {
        let projection = match config::Projection::parse(&self.row_type) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        let listing = crate::output::render::render_columns_listing(projection, color);
        print_paged(
            &listing,
            PagerOptions::from_flags(self.pager, self.no_pager),
        );
        Ok(())
    }
}
