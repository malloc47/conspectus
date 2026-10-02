//! Pin helpers: defaults for the pin forms and mutation targets.

use super::tree::row_pin_id;
use super::*;

impl App {
    /// After a successful pin create/adopt mutation and refresh,
    /// focus the row that represents `pin_id` in the current view.
    /// Grouped sessions/mux views prefer the synthetic Pins bucket;
    /// flat and union views fall back to the visible pinned entity
    /// row. Returns `false` when the refreshed view has no row for
    /// the pin (for example, an active filter hides it).
    pub fn select_pin_after_mutation(&mut self, pin_id: &str) -> bool {
        if self
            .tree
            .rows
            .iter()
            .any(|row| matches!(row.id, RowId::Synthetic("pins")) && row.expandable)
        {
            self.expanded.insert(RowId::Synthetic("pins"));
        }

        let visible = self.visible_rows();
        let pins_group_idx = visible
            .iter()
            .position(|row| matches!(row.id, RowId::Synthetic("pins")));
        let target = pins_group_idx
            .and_then(|idx| {
                let depth = visible[idx].depth;
                visible
                    .iter()
                    .enumerate()
                    .skip(idx + 1)
                    .take_while(|(_, row)| row.depth > depth)
                    .find_map(|(idx, row)| (row_pin_id(row) == Some(pin_id)).then_some(idx))
            })
            .or_else(|| {
                visible
                    .iter()
                    .enumerate()
                    .find_map(|(idx, row)| (row_pin_id(row) == Some(pin_id)).then_some(idx))
            });
        let Some(idx) = target else {
            return false;
        };
        let row_id = visible[idx].id.clone();
        self.last_visible_index = Some(idx);
        self.selection = Some(row_id);
        self.status_message = None;
        self.recompute_detail();
        true
    }

    /// Snapshot of the live state the controls overlay renders
    /// against. Borrowed each frame so the overlay never lags
    /// behind the app.
    pub fn controls_context(&self) -> crate::tui::widgets::controls::ControlsContext<'_> {
        crate::tui::widgets::controls::ControlsContext {
            view: self.active_view,
            grouping: self.grouping,
            filter: &self.filter,
            sort: self.sort,
            mux_recency: self.mux_recency,
            preview_wrap: self.preview_wrap,
        }
    }

    pub(super) fn pin_bind_options(&self) -> Vec<PinBindOption> {
        crate::tui::actions::selected_pin_diagnostics(self)
            .into_iter()
            .find_map(|diagnostic| match diagnostic {
                crate::tui::actions::PinDiagnosticView::Ambiguous {
                    pin_id,
                    chosen,
                    competing,
                } => Some(
                    std::iter::once(chosen)
                        .chain(competing)
                        .map(|session| PinBindOption {
                            pin_id: pin_id.clone(),
                            session_key: session.session_key.clone(),
                            label: format!("{}:{}", session.harness_key, session.session_key),
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default()
    }

    pub(super) fn pin_mutation_target(&self) -> Option<PinMutationTarget> {
        let selection = self.selection.as_ref()?;
        let row = self.tree.rows.iter().find(|row| &row.id == selection)?;
        let id = match &row.kind {
            RowKind::Pin(pin) => {
                return Some(PinMutationTarget {
                    id: pin.pin_id.clone(),
                    display_name: pin.display_name.clone(),
                    harness: pin.harness.clone(),
                    cwd: pin.cwd.clone(),
                    mux_name: pin.mux_name.clone(),
                    mux_socket: pin.mux_socket.clone(),
                    launch_argv: pin.launch_argv.clone(),
                    store_path: pin.store_path.clone(),
                });
            }
            RowKind::AgentSession(session) => session.pin_id.clone()?,
            RowKind::MuxSession(mux) => mux.pin_id.clone()?,
            _ => return None,
        };
        self.handle.as_ref().and_then(|db| {
            db.snapshot()
                .pins
                .iter()
                .find(|pin| pin.id == id)
                .map(|pin| PinMutationTarget {
                    id: pin.id.clone(),
                    display_name: pin.display_name.clone(),
                    harness: pin.harness.clone(),
                    cwd: pin.cwd.clone(),
                    mux_name: pin.mux.name.clone(),
                    mux_socket: pin.mux.socket_name.clone(),
                    launch_argv: pin.launch_argv.clone().unwrap_or_default(),
                    store_path: pin.store_path.clone(),
                })
        })
    }

    pub(super) fn pin_create_defaults(&self) -> PinCreateDefaults {
        let Some(selection) = self.selection.as_ref() else {
            return PinCreateDefaults::default();
        };
        let Some(row) = self.tree.rows.iter().find(|row| &row.id == selection) else {
            return PinCreateDefaults::default();
        };
        match &row.kind {
            RowKind::AgentSession(session) => {
                let cwd = self
                    .handle
                    .as_ref()
                    .and_then(|db| {
                        db.snapshot().nodes.iter().find_map(|node| match node {
                            crate::model::GraphNode::AgentSession(node)
                                if node.id == session.session =>
                            {
                                node.cwd.clone()
                            }
                            _ => None,
                        })
                    })
                    .unwrap_or_default();
                let display = session
                    .display_label()
                    .map_or_else(|| session.session.session_key.clone(), str::to_string);
                let display = pin_create_default_name_candidate(&display);
                let raw_id = pin_id_candidate(&display);
                let series = self.next_pin_series_name(&raw_id);
                // A numeric bump (`worker-1` → `worker-2`)
                // replaces the primary display name too so all three
                // derived fields open aligned. Plain `-N` suffix
                // collisions leave the operator-visible display name
                // alone so verbose session titles stay readable.
                let display_name = if series.bumped_numerically {
                    series.name.clone()
                } else {
                    display
                };
                PinCreateDefaults {
                    id: series.name.clone(),
                    display_name,
                    harness: session.session.harness_key.clone(),
                    cwd,
                    mux_name: series.name,
                    mode: PinCreateMode::NewVariation,
                }
            }
            RowKind::Group(group) => group
                .primary_node
                .as_ref()
                .and_then(pin_cwd_from_node)
                .map(|cwd| PinCreateDefaults {
                    cwd,
                    ..PinCreateDefaults::default()
                })
                .unwrap_or_default(),
            RowKind::MuxSession(mux) => {
                let base_name =
                    pin_create_default_name_candidate(&pin_id_candidate(&mux.native_id));
                let mux_name = self.unique_pin_mux_name(&base_name);
                PinCreateDefaults {
                    id: mux_name.clone(),
                    display_name: mux_name.clone(),
                    // Mirror the CLI `pin adopt` harness inference
                    // (`src/cli.rs:3883-3902`): the first active
                    // `LinkedToMux` candidate whose source is an
                    // AgentSession wins. Seeded as a default — the
                    // operator can still edit the field before commit.
                    harness: self.infer_harness_for_mux(&mux.mux).unwrap_or_default(),
                    // Read the raw cwd from the snapshot rather than
                    // `cwd_display`, which is tilde-shortened for
                    // rendering and would be rejected by the pin
                    // validator's `is_absolute` check on commit.
                    cwd: self.mux_cwd_for(&mux.mux).unwrap_or_default(),
                    mux_name,
                    mode: PinCreateMode::NewVariation,
                }
            }
            RowKind::Pin(pin) => {
                // Creating a fresh pin with an existing pin selected
                // seeds harness / cwd from the source
                // and picks the next name in the series so `worker-1`
                // → `worker-2` needs no manual retype. Bases that do
                // not end in digits still fall back to `<name>-2`.
                let base_name = pin_create_default_name_candidate(&pin.pin_id);
                let series = self.next_pin_series_name(&base_name);
                PinCreateDefaults {
                    id: series.name.clone(),
                    display_name: series.name.clone(),
                    harness: pin.harness.clone(),
                    cwd: pin.cwd.clone(),
                    mux_name: series.name,
                    mode: PinCreateMode::NewVariation,
                }
            }
            _ => PinCreateDefaults::default(),
        }
    }

    pub fn pin_adopt_defaults(&self) -> PinCreateDefaults {
        let mut defaults = self.pin_create_defaults();
        defaults.mode = PinCreateMode::AdoptSelected;
        if let Some(selection) = self.selection.as_ref()
            && let Some(row) = self.tree.rows.iter().find(|row| &row.id == selection)
            && let RowKind::MuxSession(mux) = &row.kind
        {
            defaults.id = pin_id_candidate(&mux.native_id);
            defaults.display_name = mux.native_id.clone();
            defaults.mux_name = mux.native_id.clone();
        }
        defaults
    }

    pub(super) fn pin_adopt_defaults_if_available(&self) -> Option<PinCreateDefaults> {
        (self.selection_is_live_mux() && self.selected_pin_id().is_none())
            .then(|| self.pin_adopt_defaults())
    }

    pub(super) fn known_pin_cwd_candidates(&self) -> Vec<PathCandidate> {
        let mut candidates = Vec::new();
        let mut push = |path: &str, source: &str, rank: i32| {
            let path = path.trim();
            if !path.is_empty() {
                candidates.push(PathCandidate::new(path, source, rank));
            }
        };
        let defaults = self.pin_create_defaults();
        push(&defaults.cwd, "selected", 1_000);
        if let Some(adopt) = self.pin_adopt_defaults_if_available() {
            push(&adopt.cwd, "adopt", 900);
        }
        if let Some(handle) = self.handle.as_ref() {
            for pin in &handle.snapshot().pins {
                push(&pin.cwd, "pin", 500);
            }
            for node in &handle.snapshot().nodes {
                match node {
                    crate::model::GraphNode::AgentSession(node) => {
                        if let Some(cwd) = node.cwd.as_deref() {
                            push(cwd, "agent", 450);
                        }
                    }
                    crate::model::GraphNode::MuxSession(node) => {
                        if let Some(cwd) = node.cwd.as_deref() {
                            push(cwd, "mux", 440);
                        }
                        if let Some(cwd) = node.active_pane_current_path.as_deref() {
                            push(cwd, "mux", 430);
                        }
                    }
                    crate::model::GraphNode::RuntimeProcess(node) => {
                        if let Some(cwd) = node.cwd.as_deref() {
                            push(cwd, "process", 400);
                        }
                    }
                    crate::model::GraphNode::Checkout(node) => {
                        push(&node.root, "checkout", 350);
                    }
                    crate::model::GraphNode::Repo(node) => {
                        let root = node
                            .common_dir
                            .strip_suffix("/.git")
                            .unwrap_or(&node.common_dir);
                        push(root, "repo", 320);
                    }
                    crate::model::GraphNode::Workspace(node) => {
                        push(&node.root, "workspace", 300);
                    }
                    crate::model::GraphNode::Pin(node) => {
                        push(&node.cwd, "pin", 500);
                    }
                    crate::model::GraphNode::Branch(_)
                    | crate::model::GraphNode::Fork(_)
                    | crate::model::GraphNode::ForgePr(_) => {}
                }
            }
        }
        candidates
    }

    pub(super) fn used_pin_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        if let Some(handle) = self.handle.as_ref() {
            for pin in &handle.snapshot().pins {
                ids.insert(pin.id.clone());
            }
        }
        ids
    }

    pub(super) fn unique_pin_mux_name(&self, base: &str) -> String {
        self.next_pin_series_name(base).name
    }

    /// Next available name in the pin/mux "series" for `base`.
    ///
    /// When `base` ends in one or more digits (e.g. `worker-1`), bumps
    /// the trailing number until it lands on a free variant. Otherwise
    /// falls back to appending `-2`, `-3`, ... The `bumped_numerically`
    /// flag lets callers know whether they should propagate the new
    /// value to the primary `display_name` — a numeric bump changes
    /// the series index, so `worker-1` → `worker-2` should replace
    /// the operator-visible name; a `-N` suffix appended to a
    /// non-numeric base keeps the human-readable label intact.
    pub(super) fn next_pin_series_name(&self, base: &str) -> PinSeriesName {
        let base = pin_id_candidate(base);
        let used = self.used_pin_series_names();
        if !used.contains(&base) {
            return PinSeriesName {
                name: base,
                bumped_numerically: false,
            };
        }
        if let Some((stem, num)) = split_trailing_number(&base) {
            let mut next = num.saturating_add(1);
            loop {
                let candidate = format!("{stem}{next}");
                if !used.contains(&candidate) {
                    return PinSeriesName {
                        name: candidate,
                        bumped_numerically: true,
                    };
                }
                next = next.saturating_add(1);
            }
        }
        for idx in 2.. {
            let candidate = format!("{base}-{idx}");
            if !used.contains(&candidate) {
                return PinSeriesName {
                    name: candidate,
                    bumped_numerically: false,
                };
            }
        }
        unreachable!("unbounded suffix search must find a free pin series name")
    }

    /// Union of names that a new pin's id/mux name must avoid: live
    /// mux native ids, existing pinned mux names, and existing pin
    /// ids. Kept together so numeric-bump variants land on a value
    /// that is free in every namespace the create form will check.
    pub(super) fn used_pin_series_names(&self) -> BTreeSet<String> {
        let mut names = self.used_pin_mux_names();
        names.extend(self.used_pin_ids());
        names
    }

    pub(super) fn used_pin_mux_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        if let Some(handle) = self.handle.as_ref() {
            for node in &handle.snapshot().nodes {
                if let crate::model::GraphNode::MuxSession(mux) = node {
                    names.insert(mux.native_id.clone());
                }
            }
            for pin in &handle.snapshot().pins {
                names.insert(pin.mux.name.clone());
            }
        }
        names
    }

    pub(super) fn used_mux_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        if let Some(handle) = self.handle.as_ref() {
            for node in &handle.snapshot().nodes {
                if let crate::model::GraphNode::MuxSession(mux) = node {
                    names.insert(mux.native_id.clone());
                }
            }
        }
        names
    }

    pub(super) fn selected_pin_id(&self) -> Option<String> {
        let selection = self.selection.as_ref()?;
        let row = self.tree.rows.iter().find(|row| &row.id == selection)?;
        row_pin_id(row).map(str::to_string)
    }

    pub(super) fn known_harness_keys(&self) -> BTreeSet<String> {
        let mut keys: BTreeSet<String> = harness_options()
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        if let Some(handle) = self.handle.as_ref() {
            for node in &handle.snapshot().nodes {
                if let crate::model::GraphNode::AgentSession(session) = node
                    && !session.harness_key.trim().is_empty()
                {
                    keys.insert(session.harness_key.clone());
                }
            }
            for pin in &handle.snapshot().pins {
                if !pin.harness.trim().is_empty() {
                    keys.insert(pin.harness.clone());
                }
            }
        }
        keys
    }

    /// Walk active `LinkedToMux` candidates whose target is `mux` and
    /// return the harness key of the first AgentSession source.
    /// Mirrors `PinAdoptArgs::run`'s inference path in `src/cli.rs`
    /// so the TUI `A` shortcut seeds the same harness the CLI's
    /// `pin adopt` would pick. Returns `None` when no active link
    /// attributes a harness to the mux.
    pub(super) fn infer_harness_for_mux(&self, mux: &MuxSessionId) -> Option<String> {
        let db = self.handle.as_ref()?;
        let snapshot = db.snapshot();
        let target = NodeId::MuxSession(mux.clone());
        snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == crate::model::RelationKind::LinkedToMux
                    && matches!(link.state, crate::model::LinkState::Active)
                    && link.target_node_id() == Some(&target)
            })
            .find_map(|link| match &link.source {
                NodeId::AgentSession(session) => Some(session.harness_key.clone()),
                _ => None,
            })
    }

    /// Look up the absolute `cwd` for a mux node from the persisted
    /// snapshot. Used to seed the pin-create form so the cwd field
    /// holds an absolute path that survives the validator in
    /// `pins::validate_entry`.
    pub(super) fn mux_cwd_for(&self, mux: &MuxSessionId) -> Option<String> {
        let db = self.handle.as_ref()?;
        db.snapshot().nodes.iter().find_map(|node| match node {
            crate::model::GraphNode::MuxSession(node) if &node.id == mux => node.cwd.clone(),
            _ => None,
        })
    }
}

pub(super) fn pin_cwd_from_node(id: &NodeId) -> Option<String> {
    match id {
        NodeId::Checkout(checkout) => Some(checkout.root.clone()),
        NodeId::Repo(repo) => repo
            .common_dir
            .strip_suffix("/.git")
            .map(str::to_string)
            .or_else(|| Some(repo.common_dir.clone())),
        _ => None,
    }
}

/// Result of walking the pin/mux name series for a base value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PinSeriesName {
    /// Free variant chosen for the base (the base itself when unused,
    /// otherwise a numerically-bumped or `-N`-suffixed variant).
    pub(crate) name: String,
    /// True when the variant came from bumping a trailing number
    /// (`worker-1` → `worker-2`), false when the base was already
    /// free or a `-N` suffix was appended.
    pub(crate) bumped_numerically: bool,
}

/// Split `s` into (stem, trailing_number) when it ends in one or more
/// ASCII digits. `worker-1` → `("worker-", 1)`; `worker` → `None`;
/// `42` → `("", 42)`. Overflow-tolerant: returns `None` when the
/// trailing digits do not fit in a `u64` rather than panicking.
pub(super) fn split_trailing_number(s: &str) -> Option<(String, u64)> {
    let mut split = s.len();
    for (idx, ch) in s.char_indices().rev() {
        if ch.is_ascii_digit() {
            split = idx;
        } else {
            break;
        }
    }
    if split == s.len() {
        return None;
    }
    let num: u64 = s[split..].parse().ok()?;
    Some((s[..split].to_string(), num))
}

pub(super) fn pin_id_candidate(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "new-pin".to_string()
    } else {
        trimmed
    }
}

pub(super) const PIN_CREATE_DEFAULT_NAME_MAX_CHARS: usize = 48;

pub(super) fn pin_create_default_name_candidate(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= PIN_CREATE_DEFAULT_NAME_MAX_CHARS {
        return trimmed.to_string();
    }
    let capped: String = trimmed
        .chars()
        .take(PIN_CREATE_DEFAULT_NAME_MAX_CHARS)
        .collect();
    let capped = if trimmed
        .chars()
        .nth(PIN_CREATE_DEFAULT_NAME_MAX_CHARS)
        .is_some_and(|ch| !ch.is_whitespace())
    {
        capped
            .rfind(char::is_whitespace)
            .map(|idx| capped[..idx].to_string())
            .unwrap_or(capped)
    } else {
        capped
    };
    let capped = capped
        .trim_end_matches(|ch: char| ch.is_whitespace() || ch == '-' || ch == '_')
        .to_string();
    if capped.is_empty() {
        "new pin".to_string()
    } else {
        capped
    }
}
