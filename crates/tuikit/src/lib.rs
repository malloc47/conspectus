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

pub mod age;
pub mod chip;
pub mod feedback;
pub mod frame;
pub mod glyphs;
pub mod help;
pub mod keys;
pub mod overlay;
pub mod path_field;
pub mod scroll;
pub mod selection;
pub mod text;
pub mod text_edit;
pub mod text_prompt;
pub mod theme;

// The test kit is off in production builds: a host turns on the `testkit`
// feature from its dev-dependencies, and the crate's own unit tests always
// have it.
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

mod lex;
