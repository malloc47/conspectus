//! The program's keymap: every key the main screen takes while no modal
//! is open, in one scoped table ([`keymap`]).
//!
//! Each binding names its scope: the program's ([`Scope::Global`]), the
//! left pane's row tree ([`LEFT_PANE`]), or the right pane's explorer
//! ([`RIGHT_PANE`], the framework explorer's own table taken in). A key is
//! looked up in the focused pane's scope, then the program's, so the keys
//! that mean one thing on the left pane and another on the right are two
//! bindings of this one table:
//!
//! | Key | Left pane | Right pane |
//! | --- | --- | --- |
//! | `Enter` | the row's default action | copy a field, drill a link, fold `Other` |
//! | `F` | clear every filter | Expanded Node Detail |
//! | `Backspace` | return from a reveal | back out of a drill; with none, arm, then leave the pane |
//! | `h`/`l`, `←`/`→` | fold the tree row | close or open the `Other` zone |
//! | `j`/`k`, the arrows, `PgUp`/`PgDn`, `g`/`G`, `Home`/`End` | move and page the tree | move and page the explorer |
//! | `Esc` | nothing | back, as `Backspace` |
//! | `e` | nothing | fold `Other` on its header |
//! | `o` | open the right pane's cursor value | the same |
//!
//! Building the keymap fails when two keys of one scope can match one key
//! event, so no key shadows another and the table's order is the order
//! the `?` page lists the keys in; each binding's help text is its row
//! there. The status bar names the bindings with a verb, each worded for
//! the selection. Overlays keep their own tables ([`overlay`]) and take
//! every key while they are open.

use std::sync::LazyLock;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

pub mod overlay;

use tuikit::explorer::{EXPLORER_KEYS, ExplorerKey};
use tuikit::keys::{
    ActiveScopes, Bind, KeyMatcher, KeyRow, Keymap, Scope, alt_key, bind, ch, ctrl, exact, include,
    key,
};
use tuikit::status_bar::{Annotation, KeyHintBar};

use crate::tui::Msg;
use crate::tui::View;
use crate::tui::app::{App, Focus};
use crate::tui::keymap::{Action, INSPECT_KEY};

/// The undo key. Toasts that offer an undo name it from here.
pub const UNDO_KEY: char = 'u';

/// The left pane's scope: the row tree's keys, while it has focus.
pub const LEFT_PANE: Scope = Scope::Pane("left");

/// The right pane's scope: the explorer's keys, while it has focus.
pub const RIGHT_PANE: Scope = Scope::Pane("right");

/// The scopes that have the keys with `focus` and no modal open: the
/// focused pane's, then the program's.
pub fn active_scopes(focus: Focus) -> ActiveScopes {
    match focus {
        Focus::Left => ActiveScopes::pane("left"),
        Focus::Right => ActiveScopes::pane("right"),
    }
}

/// The right pane's keys: the framework explorer's table, which the
/// keymap takes in as the right pane's scope ([`RIGHT_PANE`]), and from
/// which the status bar's right-pane hint is generated.
pub const RIGHT_PANE_KEYS: &[KeyRow<ExplorerKey>] = &[include(EXPLORER_KEYS)];

/// How long each piece of the status bar holds on as the bar narrows, the
/// highest longest, so the bar packs what fits in this order: an
/// overlay's keys, what a narrowed view is narrowed by, the way back, and
/// the way out first; the refresh keys last. The keymap's bindings carry
/// theirs; the right pane's keys and an overlay's take one rank each, and
/// among them the earlier hint holds on longer.
pub mod status_rank {
    /// The keys of the overlay on top, which the bar names instead of the
    /// main view's: they outlast the view's settings, which the overlay
    /// covers or shows itself.
    pub const OVERLAY_KEYS: u32 = 12;
    pub const FIND: u32 = 11;
    pub const BACK: u32 = 10;
    pub const QUIT: u32 = 9;
    pub const FILTER: u32 = 8;
    /// The right pane's keys, with the right pane focused: they matter
    /// more than what the selection's keys do.
    pub const RIGHT_PANE_KEYS: u32 = 7;
    /// What the selection's keys do: its default action and the
    /// accelerators beside it.
    pub const DEFAULT_ACTION: u32 = 6;
    pub const GROUP: u32 = 5;
    pub const SORT: u32 = 4;
    pub const LEFT_PANE_KEYS: u32 = 3;
    pub const FOCUS: u32 = 2;
    pub const REFRESH: u32 = 1;
}

/// What a key of the keymap asks for: one variant per binding (the
/// explorer's keys, and `o` from either pane, are [`Command::Explorer`]).
/// [`Command::action`] is what the loops dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `f`: the Controls overlay.
    Controls,
    /// `!`: the Messages log.
    Messages,
    /// `?`: the help page.
    Help,
    /// `Enter` on the left pane: the selected row's default action.
    Default,
    /// `a`: attach the selection's mux.
    Attach,
    /// `i`: copy the selected session's id.
    CopyId,
    Rename,
    WorktreeMenu,
    WorktreeCloseDown,
    /// `v`: open the selection's transcript.
    View,
    Refresh,
    /// `Ctrl-R`: refresh and ask the forges again.
    HardRefresh,
    /// `S`: resume an un-muxed session.
    Resume,
    Quit,
    NewMux,
    LaunchMux,
    MuxMenu,
    TogglePin,
    Undo,
    /// `.`: the selection's action menu.
    Menu,
    SwitchView(View),
    NextView,
    PreviousView,
    ClearFilters,
    CycleGrouping,
    QuickSelect,
    ContentSearch,
    OlderSearch,
    NewerSearch,
    SearchGranularity,
    SearchOptions,
    KeepSearch,
    Up,
    Down,
    /// `←`: fold the selected row.
    Collapse,
    /// `→`: unfold the selected row.
    Expand,
    PageUp,
    PageDown,
    First,
    Last,
    Reveal,
    /// `Backspace` on the left pane: return from a reveal.
    Back,
    /// `Tab`: the other pane.
    Focus,
    PreviewDown,
    PreviewUp,
    ToggleEdgeMeta,
    /// An explorer key: the right pane's, or `o` from either pane.
    Explorer(ExplorerKey),
}

impl Command {
    /// What the loops dispatch for this command.
    pub fn action(self) -> Action {
        let msg = |msg: Msg| Action::Msg(Box::new(msg));
        match self {
            Command::Controls => Action::OpenControls,
            Command::Messages => Action::OpenMessages,
            Command::Help => Action::OpenHelp,
            Command::Default => Action::DefaultAction,
            Command::Attach => Action::Attach,
            Command::CopyId => Action::CopySessionId,
            Command::Rename => Action::OpenRename,
            Command::WorktreeMenu => Action::OpenWorktreeMenu,
            Command::WorktreeCloseDown => Action::OpenWorktreeCloseDown,
            Command::View => Action::View,
            Command::Refresh => Action::Refresh,
            Command::HardRefresh => Action::HardRefresh,
            Command::Resume => Action::Resume,
            Command::Quit => msg(Msg::Quit),
            Command::NewMux => Action::OpenNewMuxForm,
            Command::LaunchMux => msg(Msg::OpenMuxLaunchForm),
            Command::MuxMenu => Action::OpenMuxMenu,
            Command::TogglePin => msg(Msg::TogglePin),
            Command::Undo => msg(Msg::Undo),
            Command::Menu => msg(Msg::OpenRowMenu),
            Command::SwitchView(view) => Action::SwitchView(view),
            Command::NextView => Action::CycleView(1),
            Command::PreviousView => Action::CycleView(-1),
            Command::ClearFilters => Action::ClearFilters,
            Command::CycleGrouping => Action::CycleGrouping(1),
            Command::QuickSelect => Action::OpenSearch,
            Command::ContentSearch => Action::OpenContentSearch,
            Command::OlderSearch => msg(Msg::PageSearchHistory(-1)),
            Command::NewerSearch => msg(Msg::PageSearchHistory(1)),
            Command::SearchGranularity => msg(Msg::ToggleSearchGranularity),
            Command::SearchOptions => msg(Msg::OpenSearchOptions),
            Command::KeepSearch => msg(Msg::ApplySearchEntry),
            Command::Up => msg(Msg::NavUp),
            Command::Down => msg(Msg::NavDown),
            Command::Collapse => msg(Msg::CollapseRow),
            Command::Expand => msg(Msg::ExpandRow),
            Command::PageUp => msg(Msg::PageUp),
            Command::PageDown => msg(Msg::PageDown),
            Command::First => msg(Msg::Home),
            Command::Last => msg(Msg::End),
            Command::Reveal => msg(Msg::RevealSelected),
            Command::Back => msg(Msg::NavBack),
            Command::Focus => msg(Msg::CycleFocus),
            Command::PreviewDown => msg(Msg::ScrollPreviewBy(1)),
            Command::PreviewUp => msg(Msg::ScrollPreviewBy(-1)),
            Command::ToggleEdgeMeta => msg(Msg::ToggleEdgeMeta),
            Command::Explorer(key) => msg(Msg::Explorer(key)),
        }
    }
}

/// The headings of the `?` page's sections, in the order the keymap
/// first reaches each.
mod heading {
    pub const DISCOVERABLE: &str = "Discoverable controls";
    pub const ACTIONS: &str = "Actions";
    pub const MUX: &str = "Mux";
    pub const ROW_ACTIONS: &str = "Row actions";
    pub const VIEWS: &str = "View switching";
    pub const FILTERS: &str = "Filters & grouping";
    pub const SEARCH: &str = "Search";
    pub const NAVIGATION: &str = "Navigation";
    pub const EXPLORER: &str = "Detail-pane graph explorer (right focus)";
}

/// A character pressed alone or with any modifier but Control, so its
/// Control chord stays free for a binding of its own: how the program's
/// letters bind (`Alt-a` attaches as `a` does).
const fn unless_ctrl(c: char) -> KeyMatcher {
    KeyMatcher::AnyModExceptCtrl(c)
}

/// A character pressed with no modifier at all: the digits, `q`, and `.`.
const fn plain(c: char) -> KeyMatcher {
    KeyMatcher::Exact {
        modifiers: KeyModifiers::NONE,
        code: KeyCode::Char(c),
    }
}

/// A character whatever modifiers come with it: the tree's vi keys.
const fn any_mod(c: char) -> KeyMatcher {
    KeyMatcher::AnyMod(KeyCode::Char(c))
}

const GLOBAL: Scope = Scope::Global;

/// `Shift-Enter`, which needs a terminal that reports it.
const SHIFT_ENTER: KeyMatcher = KeyMatcher::Exact {
    modifiers: KeyModifiers::SHIFT,
    code: KeyCode::Enter,
};

/// The program's own bindings and the left pane's, in the order the `?`
/// page lists them; the right pane's come from its table after them
/// ([`keymap`]).
pub const PROGRAM_KEYS: &[Bind<Command, App>] = &[
    bind("controls", GLOBAL, &[unless_ctrl('f')], Command::Controls).help(
        heading::DISCOVERABLE,
        "Open the controls overlay (view / grouping / filters / sort / preview wrap)",
    ),
    bind("messages", GLOBAL, &[unless_ctrl('!')], Command::Messages)
        .verb("messages")
        .help(
            heading::DISCOVERABLE,
            "Open the messages log: launch, attach and rename outcomes with full output",
        ),
    bind("help", GLOBAL, &[unless_ctrl('?')], Command::Help).help(heading::DISCOVERABLE, "This help"),
    bind("row.default", LEFT_PANE, &[exact(KeyCode::Enter)], Command::Default)
        .verb("open")
        .rank(status_rank::DEFAULT_ACTION)
        .help(
            heading::ACTIONS,
            "Default action on the selected row: attach mux/muxed sessions, view un-muxed sessions, rebuild a stopped mux, pin, or history record, expand groups",
        ),
    bind("attach", GLOBAL, &[unless_ctrl('a')], Command::Attach)
        .verb("attach")
        .rank(status_rank::DEFAULT_ACTION)
        .help(heading::ACTIONS, "Attach to the selected mux"),
    bind("row.copy-id", GLOBAL, &[unless_ctrl('i')], Command::CopyId).help(
        heading::ACTIONS,
        "Copy the selected agent or mux session's full id to the clipboard",
    ),
    bind(
        "value.inspect",
        GLOBAL,
        &[unless_ctrl(INSPECT_KEY)],
        Command::Explorer(ExplorerKey::Inspect),
    )
    .help(
        heading::ACTIONS,
        "Open the full untruncated value of the right pane's cursor row in a modal, from either pane",
    ),
    bind("rename", GLOBAL, &[ch('R')], Command::Rename).help(
        heading::ACTIONS,
        "Rename the selected agent session (alias) or mux (tmux + pin cascade)",
    ),
    bind("worktree.menu", GLOBAL, &[unless_ctrl('w')], Command::WorktreeMenu).help(
        heading::ACTIONS,
        "Open the worktree action menu for the selected repo / worktree / mux (new, merge, remove, close-down, prune, reveal; mutations need a backend)",
    ),
    bind("worktree.close-down", GLOBAL, &[ch('X')], Command::WorktreeCloseDown).help(
        heading::ACTIONS,
        "Close down the selected stream: merge or discard the branch, end its sessions, remove the worktree, forget its history",
    ),
    bind("transcript.view", GLOBAL, &[unless_ctrl('v')], Command::View)
        .verb("view")
        .rank(status_rank::DEFAULT_ACTION)
        .help(
            heading::ACTIONS,
            "Open the selected session's transcript (or the mux row's linked session)",
        ),
    bind("refresh", GLOBAL, &[unless_ctrl('r')], Command::Refresh)
        .verb("refresh")
        .rank(status_rank::REFRESH)
        .help(heading::ACTIONS, "Refresh discovery now"),
    bind("refresh.hard", GLOBAL, &[ctrl('r')], Command::HardRefresh)
        .verb("hard refresh")
        .rank(status_rank::REFRESH)
        .help(
            heading::ACTIONS,
            "Hard refresh: rescan and re-query forges (gh) for pull requests",
        ),
    bind("resume", GLOBAL, &[ch('S')], Command::Resume)
        .verb("resume")
        .rank(status_rank::DEFAULT_ACTION)
        .help(
            heading::ACTIONS,
            "Resume the selected un-muxed agent session in a new terminal",
        ),
    bind("quit", GLOBAL, &[plain('q'), ctrl('c')], Command::Quit)
        .verb("quit")
        .rank(status_rank::QUIT)
        .help(heading::ACTIONS, "Quit"),
    bind("mux.new", GLOBAL, &[unless_ctrl('n')], Command::NewMux).help(
        heading::MUX,
        "Create a bare tmux session (no agent, no worktree) — shell in the selected cwd",
    ),
    bind("mux.launch", GLOBAL, &[ch('N')], Command::LaunchMux).help(
        heading::MUX,
        "Launch a harness in a new mux (the form can also create a worktree for it)",
    ),
    bind("mux.menu", GLOBAL, &[unless_ctrl('m')], Command::MuxMenu).help(
        heading::MUX,
        "Open the Mux action menu — pick between new bare session and launching a harness in a fresh mux",
    ),
    bind("pin.toggle", GLOBAL, &[unless_ctrl('p')], Command::TogglePin).help(
        heading::ROW_ACTIONS,
        "Pin the selected row, or unpin it when it is already pinned; pinned rows carry ◉ (◎ when the pinned entity isn't running)",
    ),
    bind("undo", GLOBAL, &[unless_ctrl(UNDO_KEY)], Command::Undo).help(
        heading::ROW_ACTIONS,
        "Undo the last pin or unpin, as far back as this session goes; a removed pin comes back with its overrides and its place in the order",
    ),
    bind("row.menu", GLOBAL, &[plain('.')], Command::Menu)
        .verb("menu")
        .rank(status_rank::DEFAULT_ACTION)
        .help(
            heading::ROW_ACTIONS,
            "Open the row action menu — pin or unpin; rebuild, forget, or show the rebuild plan for a history record; edit, rename, retarget, refresh, or reveal a pin in the Pinned view; Clean up… in the History view",
        ),
    bind("view.sessions", GLOBAL, &[plain('1')], Command::SwitchView(View::Sessions))
        .help(heading::VIEWS, "Switch directly to the sessions view"),
    bind("view.mux", GLOBAL, &[plain('2')], Command::SwitchView(View::Mux))
        .help(heading::VIEWS, "Switch directly to the mux view"),
    bind("view.prs", GLOBAL, &[plain('3')], Command::SwitchView(View::Prs))
        .help(heading::VIEWS, "Switch directly to the PRs view"),
    bind("view.pinned", GLOBAL, &[plain('4')], Command::SwitchView(View::Pinned))
        .help(heading::VIEWS, "Switch directly to the pinned view"),
    bind("view.history", GLOBAL, &[plain('5')], Command::SwitchView(View::History))
        .help(heading::VIEWS, "Switch directly to the history view"),
    bind("view.search", GLOBAL, &[plain('6')], Command::SwitchView(View::Search))
        .help(heading::VIEWS, "Switch directly to the Search view"),
    bind("view.next", GLOBAL, &[unless_ctrl(']')], Command::NextView)
        .help(heading::VIEWS, "Cycle to the next view"),
    bind("view.previous", GLOBAL, &[unless_ctrl('[')], Command::PreviousView)
        .help(heading::VIEWS, "Cycle to the previous view"),
    bind("filters.clear", LEFT_PANE, &[ch('F')], Command::ClearFilters)
        .help(heading::FILTERS, "Clear all active filters for the visible view"),
    bind("grouping.cycle", GLOBAL, &[ctrl('g')], Command::CycleGrouping)
        .help(heading::FILTERS, "Cycle grouping forward for the active view"),
    bind("search.quick", GLOBAL, &[unless_ctrl('/')], Command::QuickSelect).help(
        heading::SEARCH,
        "Quick select: find a row of this view by name (rows under collapsed groups too)",
    ),
    // `Ctrl-/` reaches the program as one of three keys, depending on the
    // terminal: a terminal using the kitty keyboard protocol sends it as
    // such; the rest send the byte 0x1F, which decodes as `Ctrl-7` (and
    // which is also what `Ctrl-_` sends).
    bind(
        "search.contents",
        GLOBAL,
        &[unless_ctrl('s'), ctrl('/'), ctrl('_'), ctrl('7')],
        Command::ContentSearch,
    )
    .help(
        heading::SEARCH,
        "Search contents: what was said in the sessions and what is in mux scrollback, in this view's sessions and muxes",
    ),
    bind("search.older", GLOBAL, &[unless_ctrl('<')], Command::OlderSearch).help(
        heading::SEARCH,
        "Search view: show the previous (older) remembered search without searching again; Controls lists them",
    ),
    bind("search.newer", GLOBAL, &[unless_ctrl('>')], Command::NewerSearch).help(
        heading::SEARCH,
        "Search view: show the next (newer) remembered search without searching again; Controls lists them",
    ),
    bind("search.granularity", GLOBAL, &[ctrl('l')], Command::SearchGranularity).help(
        heading::SEARCH,
        "Search view: list sessions or messages (the same matches, no new scan)",
    ),
    bind("search.options", GLOBAL, &[ctrl('o')], Command::SearchOptions).help(
        heading::SEARCH,
        "Search view: choose what a search looks in (tool calls, thinking, pane scrollback)",
    ),
    bind("search.keep", GLOBAL, &[ctrl('f')], Command::KeepSearch).help(
        heading::SEARCH,
        "Search view: keep the search shown as the filter on the lists (paging does not change the filter; F clears it)",
    ),
    bind("tree.up", LEFT_PANE, &[key(KeyCode::Up), any_mod('k')], Command::Up)
        .verb("move")
        .rank(status_rank::LEFT_PANE_KEYS)
        .help(heading::NAVIGATION, "Move the selection up"),
    bind("tree.down", LEFT_PANE, &[key(KeyCode::Down), any_mod('j')], Command::Down)
        .verb("move")
        .rank(status_rank::LEFT_PANE_KEYS)
        .help(heading::NAVIGATION, "Move the selection down"),
    bind(
        "tree.collapse",
        LEFT_PANE,
        &[key(KeyCode::Left), unless_ctrl('h')],
        Command::Collapse,
    )
    .verb("fold")
    .rank(status_rank::LEFT_PANE_KEYS)
    .help(
        heading::NAVIGATION,
        "Collapse the selected left-tree row (vi-style fold)",
    ),
    bind(
        "tree.expand",
        LEFT_PANE,
        &[key(KeyCode::Right), unless_ctrl('l')],
        Command::Expand,
    )
    .verb("fold")
    .rank(status_rank::LEFT_PANE_KEYS)
    .help(
        heading::NAVIGATION,
        "Expand the selected left-tree row (vi-style fold)",
    ),
    // The tree pages by the rows it drew in the last frame, which its
    // state keeps.
    bind("tree.page-down", LEFT_PANE, &[key(KeyCode::PageDown)], Command::PageDown)
        .help(heading::NAVIGATION, "Page down through the row tree"),
    bind("tree.page-up", LEFT_PANE, &[key(KeyCode::PageUp)], Command::PageUp)
        .help(heading::NAVIGATION, "Page up through the row tree"),
    // `g` leaves its Control chord to the grouping key.
    bind(
        "tree.first",
        LEFT_PANE,
        &[key(KeyCode::Home), unless_ctrl('g')],
        Command::First,
    )
    .help(heading::NAVIGATION, "Jump to the first row"),
    bind("tree.last", LEFT_PANE, &[key(KeyCode::End), any_mod('G')], Command::Last)
        .help(heading::NAVIGATION, "Jump to the last row"),
    bind(
        "row.reveal",
        GLOBAL,
        &[SHIFT_ENTER, alt_key(KeyCode::Enter), ch('O')],
        Command::Reveal,
    )
    .help(
        heading::NAVIGATION,
        "Reveal the row's entity in its own view (Sessions, Mux, PRs, History); Backspace on the left tree returns. Shift-Enter needs a terminal that reports it",
    ),
    bind("reveal.back", LEFT_PANE, &[key(KeyCode::Backspace)], Command::Back)
        .verb("back")
        .rank(status_rank::BACK)
        .available_when(App::can_nav_back)
        .help(heading::NAVIGATION, "Return to where the last reveal started"),
    bind("focus.cycle", GLOBAL, &[key(KeyCode::Tab)], Command::Focus)
        .verb("focus")
        .rank(status_rank::FOCUS)
        .help(
            heading::NAVIGATION,
            "Cycle focus between left tree and right panel",
        ),
    bind("preview.down", GLOBAL, &[any_mod('J')], Command::PreviewDown)
        .help(heading::NAVIGATION, "Scroll the right-panel preview down"),
    bind("preview.up", GLOBAL, &[any_mod('K')], Command::PreviewUp)
        .help(heading::NAVIGATION, "Scroll the right-panel preview up"),
];

/// The program's bindings listed after the right pane's: `E`, which acts
/// on the explorer's link rows from either pane.
pub const AFTER_RIGHT_PANE_KEYS: &[Bind<Command, App>] = &[bind(
    "explorer.edge-meta",
    GLOBAL,
    &[ch('E')],
    Command::ToggleEdgeMeta,
)
.help(
    heading::EXPLORER,
    "Toggle edge meta (provenance · confidence · state) on link rows",
)];

/// The right pane's binding of each explorer key: its id and what it does
/// on this page; its keys, verb, and order come from [`RIGHT_PANE_KEYS`].
fn right_pane_binding(key: ExplorerKey) -> Bind<Command, App> {
    let (id, help) = match key {
        ExplorerKey::Down => (
            "explorer.down",
            "Move the explorer cursor down, from the Node fields into the relationship rows",
        ),
        ExplorerKey::Up => ("explorer.up", "Move the explorer cursor up"),
        ExplorerKey::PageDown => (
            "explorer.page-down",
            "Page the explorer cursor down by the rows the zone shows",
        ),
        ExplorerKey::PageUp => (
            "explorer.page-up",
            "Page the explorer cursor up by the rows the zone shows",
        ),
        ExplorerKey::First => ("explorer.first", "First explorer row"),
        ExplorerKey::Last => ("explorer.last", "Last explorer row"),
        ExplorerKey::Activate => (
            "explorer.activate",
            "Copy the value on a Node-zone field row · drill on a link row · open or close the Other zone on its header",
        ),
        ExplorerKey::Inspect => (
            "explorer.inspect",
            "Open the full untruncated value for the cursor row in a modal",
        ),
        ExplorerKey::ToggleFull => (
            "explorer.toggle-full",
            "Toggle Expanded Node Detail (every per-kind field) on the focused node",
        ),
        ExplorerKey::ToggleSection => (
            "explorer.toggle-section",
            "Open or close the Other zone (alternates, conflicts, unresolved evidence) on its header",
        ),
        ExplorerKey::OpenSection => ("explorer.open-section", "Open the Other zone on its header"),
        ExplorerKey::CloseSection => (
            "explorer.close-section",
            "Close the Other zone from its header or a row inside it",
        ),
        ExplorerKey::Back => (
            "explorer.back",
            "Back out of the most recent drilldown hop · once the stack is empty, press twice to return focus to the left pane",
        ),
    };
    bind(id, RIGHT_PANE, &[], Command::Explorer(key)).help(heading::EXPLORER, help)
}

/// The program's keymap, built once: [`PROGRAM_KEYS`], the right pane's
/// table, then [`AFTER_RIGHT_PANE_KEYS`].
///
/// # Panics
///
/// When two keys of one scope can match one key event, two bindings
/// share an id, or a binding has no key or no help; the keymap's tests
/// build it, so such a table fails them first.
pub fn keymap() -> &'static Keymap<Command, App> {
    static KEYMAP: LazyLock<Keymap<Command, App>> = LazyLock::new(|| {
        Keymap::builder()
            .rows(PROGRAM_KEYS)
            .table(RIGHT_PANE_KEYS, right_pane_binding)
            .rows(AFTER_RIGHT_PANE_KEYS)
            .build()
            .unwrap_or_else(|conflicts| panic!("the keymap's keys conflict:\n{conflicts}"))
    });
    &KEYMAP
}

/// The key that opens the Messages log with `verb` (`! details`), from its
/// binding: what the unseen-messages chip and a failure's status message
/// say, so they name the key the keymap binds.
pub fn messages_hint(app: &App, verb: &str) -> String {
    KeyHintBar::new()
        .keymap(keymap(), ActiveScopes::global(), app, 0, |command| {
            (*command == Command::Messages).then(|| Annotation::new().verb(verb))
        })
        .text()
}

/// The key of the binding `id`, as hints write it (`Backspace`, `Ctrl-F`,
/// `r`): the binding's first key, the one its hint names. A status or
/// refusal message that tells the operator which key to press names it
/// from here, so the message names the key the keymap binds.
///
/// # Panics
///
/// When the keymap has no binding `id`; each message's test reaches its
/// ids, so a renamed binding fails them first.
pub fn key_name(id: &str) -> String {
    keymap()
        .get(id)
        .and_then(|binding| binding.keys().first())
        .map_or_else(
            || panic!("the keymap binds no `{id}`"),
            tuikit::keys::key_label,
        )
}

#[cfg(test)]
#[path = "keybindings_tests.rs"]
mod tests;
