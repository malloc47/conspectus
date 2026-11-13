//! The documentation pages as doctests.
//!
//! Each item below takes one page under `docs/` as its documentation, so
//! every `rust` block on the page compiles and runs with the crate's own
//! doctests: an example that stops building, or whose assertions stop
//! holding, fails `cargo test` instead of rotting on the page. The module
//! exists only while doctests are collected (`cfg(doctest)`), so it adds
//! nothing to the library or its rendered documentation.
//!
//! `tests/doc_sync.rs` fails when a page under `docs/` has no item here or
//! an item names a page that is gone, so a new page joins the list in the
//! change that adds it. A page whose examples need a cargo feature sits
//! behind that feature.

macro_rules! pages {
    ($($item:ident => $path:literal,)*) => {
        $(
            #[doc = include_str!(concat!("../docs/", $path))]
            pub struct $item;
        )*
    };
}

pages! {
    Guide => "guide.md",
    Architecture => "architecture.md",
    HouseRules => "house-rules.md",
    Contributing => "contributing.md",
    Roadmap => "roadmap.md",
    ActionMenu => "controls/action_menu.md",
    Age => "controls/age.md",
    AppFrame => "controls/app_frame.md",
    Breadcrumb => "controls/breadcrumb.md",
    Checklist => "controls/checklist.md",
    Chip => "controls/chip.md",
    Columns => "controls/columns.md",
    Dialog => "controls/dialog.md",
    EmptyState => "controls/empty_state.md",
    Explorer => "controls/explorer.md",
    Feedback => "controls/feedback.md",
    Form => "controls/form.md",
    Frame => "controls/frame.md",
    Glyphs => "controls/glyphs.md",
    Help => "controls/help.md",
    Keys => "controls/keys.md",
    LogList => "controls/log_list.md",
    MasterDetail => "controls/master_detail.md",
    Overlay => "controls/overlay.md",
    Pager => "controls/pager.md",
    Palette => "controls/palette.md",
    PathField => "controls/path_field.md",
    PriorityLine => "controls/priority_line.md",
    PropertyGrid => "controls/property_grid.md",
    Scroll => "controls/scroll.md",
    Selection => "controls/selection.md",
    SettingsPanel => "controls/settings_panel.md",
    StatusBar => "controls/status_bar.md",
    TabBar => "controls/tab_bar.md",
    Testkit => "controls/testkit.md",
    Text => "controls/text.md",
    TextEdit => "controls/text_edit.md",
    TextPrompt => "controls/text_prompt.md",
    Theme => "controls/theme.md",
    TreeView => "controls/tree_view.md",
}

#[cfg(feature = "richtext")]
pages! {
    Richtext => "controls/richtext.md",
}
