// The crate-level documentation is the README, so the two cannot drift. The
// README keeps its links relative and tags every code fence, because an
// untagged fence would compile as a doctest.
#![doc = include_str!("../README.md")]
// Documentation is part of the public API: a public module or item without
// docs fails the build. These lints live in the crate, not in the workspace
// lint table, for two reasons. Cargo does not let a package extend the
// workspace table (`lints.workspace = true` excludes any other lint table),
// and the host package does not require docs on every item. An attribute in
// the crate also survives a lift into its own repository, where the
// workspace table is gone. The two rustdoc denies repeat the workspace
// table for the same reason. `tests/documentation.rs` fails if any of the
// lines below is removed.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]

pub mod action_menu;
pub mod age;
pub mod app_frame;
pub mod breadcrumb;
pub mod checklist;
pub mod chip;
pub mod columns;
pub mod dialog;
pub mod explorer;
pub mod feedback;
pub mod form;
pub mod frame;
pub mod glyphs;
pub mod help;
pub mod keys;
pub mod log_list;
pub mod master_detail;
pub mod overlay;
pub mod pager;
pub mod palette;
pub mod path_field;
pub mod property_grid;
// Rich text pulls in the parsing crates, so it is a feature a host turns on.
#[cfg(feature = "richtext")]
pub mod richtext;
pub mod scroll;
pub mod selection;
pub mod settings_panel;
pub mod tab_bar;
pub mod text;
pub mod text_edit;
pub mod text_prompt;
pub mod theme;
pub mod tree;

// The test kit is off in production builds: a host turns on the `testkit`
// feature from its dev-dependencies, and the crate's own unit tests always
// have it.
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

mod lex;
