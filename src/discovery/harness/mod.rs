//! Agent harness discovery boundaries.
//!
//! Each supported harness ships with a small adapter implementing
//! [`HarnessAdapter`]. Adapters take the active [`DiscoveryContext`] and emit a
//! provider-neutral [`GraphFragment`] containing `AgentSession` nodes and the
//! source metadata needed to preserve provider provenance. Adapters must not
//! perform rendering, fork lineage resolution, or session/mux scoring – those
//! belong to higher layers.
//!
//! Most adapters look up a single state root via
//! [`DiscoveryContext::harness_state_root`]; per-repo harnesses such as aider
//! walk the configured scan roots instead. The [`HarnessDiscovery`] coordinator
//! is a [`DiscoveryProvider`] that runs every registered adapter and merges
//! fragments deterministically through [`merge_fragments`].
//!
//! H-EXT-004: adapters also carry a [`RuntimeSignature`] that
//! describes their process / fd / session-key surface. The
//! `cross_link` module iterates registered adapters and consumes
//! signatures generically instead of hard-coding
//! per-harness match arms.

#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};

pub mod aider;
pub mod claude_code;
pub mod codex;
#[doc(hidden)]
pub mod fixtures;
pub mod opencode;

pub use aider::AiderAdapter;
pub use claude_code::ClaudeCodeAdapter;
pub use codex::CodexAdapter;
pub use opencode::OpenCodeAdapter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HarnessLaunchOption {
    pub id: &'static str,
    pub label: &'static str,
    pub argv: &'static [&'static str],
}

/// Runtime attribution surface for a single harness (H-EXT-004).
///
/// The `cross_link` module iterates registered adapters and
/// consumes signatures generically instead of hard-coding
/// per-harness match arms. Each field carries a small,
/// composable chunk of harness knowledge that `cross_link` uses
/// to attribute mux panes and process trees back to sessions
/// discovered by the adapter's `discover` method.
///
/// The struct is intentionally lean — one static value per
/// adapter — so registering a new harness is a matter of
/// filling in the fields, not writing a new attribution
/// pipeline.
pub struct RuntimeSignature {
    /// Harness key this signature belongs to; matches
    /// [`HarnessAdapter::harness_key`]. Present so
    /// registry-iteration consumers can carry the key through
    /// without a second lookup.
    pub harness_key: &'static str,
    /// Executable basenames that identify this harness in
    /// process command output. Matched case-insensitively
    /// against the first whitespace-separated token of the
    /// command's basename. For example, `claude-code` accepts
    /// both `claude` and `claude-code` because either binary
    /// may be on `PATH`.
    pub process_command_basenames: &'static [&'static str],
    /// Substrings that identify this harness in loose command
    /// scans (mux `active_pane_command` and
    /// `active_pane_start_command`). Matched case-insensitively
    /// via `contains(...)`, so keep the entries short and
    /// unambiguous.
    pub command_substrings: &'static [&'static str],
    /// Path prefixes that identify this harness in an fd path
    /// (e.g. `"/.codex/sessions/"` for codex). Matched via
    /// `contains(...)` so paths with leading directories
    /// (e.g. `/home/alice/.codex/sessions/…`) still match.
    pub fd_path_patterns: &'static [&'static str],
    /// Extract session keys from a text (fd path or command).
    /// Different harnesses use different session-id grammars —
    /// opencode's `ses_<alphanumeric>` vs. codex/claude-code's
    /// UUID-shaped keys. Defaults to
    /// [`generic_uuid_like_session_keys`] for adapters without
    /// a custom grammar.
    pub extract_session_keys: fn(&str) -> BTreeSet<String>,
    /// Recognize this harness's helper / daemon processes so
    /// `cross_link` can classify them as [`RuntimeProcessRole::Background`]
    /// instead of treating them as human-driven agent panes.
    /// Defaults to always-false.
    pub is_background_process: fn(&str) -> bool,
    /// Recognize this harness's subagent processes (opencode's
    /// nested-agent spawns, currently) so `cross_link` can
    /// classify them as [`RuntimeProcessRole::Subagent`] instead
    /// of duplicating them as human-driven rows. Defaults to
    /// always-false.
    pub is_subagent_process: fn(&str) -> bool,
}

/// UUID-shaped session-key grammar used by codex, claude-code,
/// and every future harness whose session ids follow the
/// canonical `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` shape.
/// Public so per-adapter `RuntimeSignature` values can point
/// their `extract_session_keys` field at it directly.
pub fn generic_uuid_like_session_keys(value: &str) -> BTreeSet<String> {
    const UUID_LEN: usize = 36;

    if value.len() < UUID_LEN {
        return BTreeSet::new();
    }

    let bytes = value.as_bytes();
    (0..=bytes.len() - UUID_LEN)
        .filter(|start| {
            is_uuid_like_bytes(&bytes[*start..*start + UUID_LEN])
                && uuid_boundary(bytes.get(start.wrapping_sub(1)).copied())
                && uuid_boundary(bytes.get(*start + UUID_LEN).copied())
        })
        .filter_map(|start| value.get(start..start + UUID_LEN).map(str::to_string))
        .collect()
}

fn is_uuid_like_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(idx, byte)| match idx {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn uuid_boundary(byte: Option<u8>) -> bool {
    !byte.is_some_and(|byte| byte.is_ascii_hexdigit())
}

/// Placeholder for `is_background_process` /
/// `is_subagent_process` on adapters that don't ship with
/// helper daemons or subagent surfaces. Adapters point their
/// signature at this instead of writing a stub.
pub fn no_match(_command: &str) -> bool {
    false
}

/// Per-adapter aux-attribution surface (H-EXT-007). Passed to
/// [`HarnessAdapter::apply_aux_attribution`] so an adapter with
/// a state / log DB (ADR 0048's codex-log shape today) can
/// mutate the merged snapshot without special-casing at the
/// caller. The context is built once per warm-start invocation
/// in `discovery::mod::apply_mutators` and reused across
/// every registered adapter.
pub struct AuxAttributionContext<'a> {
    /// This harness's state root (typically
    /// `LocalDiscoveryConfig.harness_state_roots.get(harness_key)`).
    /// Adapters that need to open a per-harness DB build the
    /// concrete path from this root.
    pub state_root: &'a Path,
    /// Per-mux `(harness_key, pid)` set produced by
    /// `cross_link::active_harness_pids_per_mux` — the same input
    /// the pre-H-EXT-007 codex-log branch consumed. Adapters
    /// that need to correlate their state DB to a live process
    /// walk this by `harness_key` and use the paired pids.
    pub harness_pids_per_mux:
        &'a std::collections::BTreeMap<crate::model::MuxSessionId, Vec<(String, i64)>>,
    /// Wall-clock epoch at which the mutator pass started. Aux
    /// readers use this as the `ts` floor for time-bounded
    /// state / log queries (see
    /// `crate::discovery::codex_log::DEFAULT_WINDOW_SECONDS`
    /// for the codex-log rationale).
    pub now_epoch: i64,
}

const CODEX_SKIP_PERMISSIONS_ARGV: &[&str] = &["--dangerously-bypass-approvals-and-sandbox"];
const CLAUDE_SKIP_PERMISSIONS_ARGV: &[&str] = &["--dangerously-skip-permissions"];

const CODEX_LAUNCH_OPTIONS: &[HarnessLaunchOption] = &[HarnessLaunchOption {
    id: "skip-permissions",
    label: "skip permissions",
    argv: CODEX_SKIP_PERMISSIONS_ARGV,
}];

const CLAUDE_CODE_LAUNCH_OPTIONS: &[HarnessLaunchOption] = &[HarnessLaunchOption {
    id: "skip-permissions",
    label: "skip permissions",
    argv: CLAUDE_SKIP_PERMISSIONS_ARGV,
}];

pub trait HarnessAdapter: Send + Sync {
    /// Stable identifier for this harness. Every production impl
    /// returns a `&'static str` literal (typically the module's
    /// `HARNESS_KEY` const), which matches how registry-derived
    /// consumers expect to consume the key (TUI filter menu items,
    /// resume dispatch keys, provider stamps).
    fn harness_key(&self) -> &'static str;

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment>;

    /// Short display label rendered in the TUI's row label column
    /// and filter chips (H-EXT-002). Defaults to the harness key;
    /// adapters override when the key is longer than the display
    /// budget — `claude-code` collapses to `claude` per H-TBL-014.
    fn display_label(&self) -> &'static str {
        self.harness_key()
    }

    /// Default argv for spawning a fresh session of this harness when
    /// no per-pin `launch.argv` override is supplied (ADR 0057
    /// §Launch). Pure data; no I/O. Each adapter returns the bare
    /// binary invocation — model selection, prompt injection, and
    /// other harness-specific flags are intentionally out of v1 scope
    /// per ADR 0057 §Deferred. Operators who need richer launch
    /// commands use `pin.launch.argv` (a per-pin override).
    fn launch_argv(&self) -> Vec<std::ffi::OsString> {
        Vec::new()
    }

    /// Launch-time options surfaced in the pin-create form
    /// (H-EXT-002). Each entry is a stable-id + label + argv
    /// fragment; the pin editor lets the operator toggle each on
    /// or off, and the resulting argv appends every toggled
    /// option's fragment to [`Self::launch_argv`]. Defaults to
    /// no options — most adapters ship without any and rely on
    /// per-pin `launch.argv` overrides for exotic invocations.
    fn launch_options(&self) -> &'static [HarnessLaunchOption] {
        &[]
    }

    /// Argv for resuming a specific session of this harness (ADR 0058).
    /// Returns `None` when the harness has no single-command resume
    /// CLI; callers fall back to `launch_argv` plus a status hint.
    ///
    /// `cwd` is provided for adapters that need context-aware resume
    /// (e.g., aider-style "load history from this directory"); the
    /// default-shape adapters that take `--resume <id>` ignore it.
    /// The default implementation returns `None` so adapters opt in
    /// explicitly.
    fn resume_argv(&self, session_id: &str, cwd: &Path) -> Option<Vec<std::ffi::OsString>> {
        let _ = (session_id, cwd);
        None
    }

    /// Runtime attribution surface (H-EXT-004). Returned
    /// reference is `'static` so `cross_link` can carry it
    /// across per-adapter iteration without allocation.
    /// Adapters return a module-level `const` value that
    /// packages together process / fd / session-key patterns.
    ///
    /// The default returns a stub signature keyed on the
    /// adapter's `harness_key` with no process / fd / session-key
    /// patterns — appropriate for test adapters and future
    /// harnesses whose attribution surface hasn't been wired
    /// yet. Production adapters override.
    fn runtime_signature(&self) -> &'static RuntimeSignature {
        &STUB_RUNTIME_SIGNATURE
    }

    /// Build a viewer `SessionLocator` for the given
    /// [`crate::model::AgentSessionId`] (H-EXT-006). Returning
    /// `None` marks this harness as having no native transcript
    /// source — the viewer bridge falls through to its
    /// escape-hatch external launch (currently only `aider` does
    /// this).
    ///
    /// Adapters that emit a locator populate its `state_root`
    /// according to their parser's expectations: claude-code +
    /// codex point at the harness state root; opencode resolves
    /// the SQLite database path (or its containing directory)
    /// so the parser can open it directly.
    fn transcript_source(
        &self,
        session: &crate::model::AgentSessionId,
    ) -> Option<crate::viewer::model::SessionLocator> {
        let _ = session;
        None
    }

    /// Native transcript parser for this harness (H-EXT-006).
    /// Returning `None` marks the harness as unsupported by
    /// the native viewer; the bridge falls back to the
    /// escape-hatch external viewer.
    fn transcript_parser(&self) -> Option<&'static dyn crate::viewer::parser::HarnessParser> {
        None
    }

    /// Apply this harness's optional aux-attribution mutator
    /// pass (H-EXT-007). Called once per warm-start invocation
    /// after the primary mutators run. Adapters with a state /
    /// log DB (ADR 0048's codex-log shape) implement this to
    /// stamp additional candidate links onto the merged
    /// snapshot; the default is a no-op, appropriate for
    /// adapters without an aux surface.
    ///
    /// The pre-H-EXT-007 codex-log special case in
    /// `discovery::mod::apply_mutators` moves into the codex
    /// adapter's override, so a fifth harness with an aux
    /// reader needs only its own trait impl — no new
    /// `LocalDiscoveryConfig` field, no named branch at the
    /// caller.
    fn apply_aux_attribution(
        &self,
        snapshot: &mut crate::model::GraphSnapshot,
        ctx: &AuxAttributionContext<'_>,
    ) {
        let _ = (snapshot, ctx);
    }

    /// Build a hook sidecar record from a harness's SessionStart
    /// hook payload (H-EXT-005). The default implementation
    /// reads the ADR 0028 canonical `session_id` string and
    /// stamps the record with `self.harness_key()`; adapters
    /// that use a different payload shape (a nested field, an
    /// alternate key) override.
    ///
    /// This is the single dispatch surface behind
    /// `conspectus hook write <harness-key>`, replacing the
    /// pre-H-EXT-005 per-harness `*_record_from_payload`
    /// writers in `src/hook.rs`.
    fn hook_record_from_payload(
        &self,
        payload: &serde_json::Value,
        pid: Option<i64>,
        ppid: Option<i64>,
        tmux: Option<crate::hook::HookTmuxRecord>,
        harness_version: Option<String>,
        observed_epoch: i64,
    ) -> anyhow::Result<crate::hook::HookRecord> {
        use anyhow::bail;
        let Some(session_id) = payload
            .get("session_id")
            .and_then(serde_json::Value::as_str)
        else {
            bail!(
                "{} hook payload missing string `session_id`",
                self.display_label()
            );
        };
        if session_id.is_empty() {
            bail!(
                "{} hook payload has empty `session_id`",
                self.display_label()
            );
        }
        Ok(crate::hook::HookRecord {
            schema_version: crate::hook::SCHEMA_VERSION,
            harness_key: self.harness_key().to_string(),
            session_key: session_id.to_string(),
            cwd: crate::hook::optional_payload_string(payload, "cwd"),
            pid,
            ppid,
            tmux: tmux.filter(|tmux| !tmux.is_empty()),
            transcript_path: crate::hook::optional_payload_string(payload, "transcript_path"),
            hook_event_name: crate::hook::optional_payload_string(payload, "hook_event_name"),
            observed_epoch,
            harness_version,
        })
    }
}

/// Stub signature used as the [`HarnessAdapter::runtime_signature`]
/// default. Its `harness_key` field is intentionally left as
/// `"unknown"` — callers that need the adapter's real key read
/// it from [`HarnessAdapter::harness_key`] instead.
static STUB_RUNTIME_SIGNATURE: RuntimeSignature = RuntimeSignature {
    harness_key: "unknown",
    process_command_basenames: &[],
    command_substrings: &[],
    fd_path_patterns: &[],
    extract_session_keys: generic_uuid_like_session_keys,
    is_background_process: no_match,
    is_subagent_process: no_match,
};

/// Canonical set of registered harness adapters (H-EXT-002).
/// The six parallel-table functions below iterate this list
/// instead of a hand-rolled match. Registering a new harness is
/// a two-step operation: add the adapter to
/// [`HarnessDiscovery::with_default_adapters`] and to
/// [`registered_adapters`]; the launch / resume / label / filter
/// / options surfaces pick it up automatically.
///
/// The list is a static `LazyLock` because adapter values are
/// `Send + Sync` zero-state and outlive the process.
static REGISTERED_ADAPTERS: std::sync::LazyLock<Vec<Box<dyn HarnessAdapter>>> =
    std::sync::LazyLock::new(|| {
        // Order matches the pre-H-EXT-002 hardcoded
        // `HARNESS_OPTIONS` slice so the TUI filter menu order
        // stays byte-identical. Discovery iteration order
        // (via `HarnessDiscovery::with_default_adapters`)
        // uses a separate registration and can differ if a new
        // adapter is added mid-list; keep both in sync when
        // adding.
        vec![
            Box::new(ClaudeCodeAdapter::new()),
            Box::new(CodexAdapter::new()),
            Box::new(OpenCodeAdapter::new()),
            Box::new(AiderAdapter::new()),
        ]
    });

/// H-REF-007: shared state-root discovery envelope. Codex,
/// claude-code, and openCode all repeat the same three steps:
/// (1) look up their state root through the discovery context;
/// (2) if it's absent, return an empty fragment; (3) run the
/// harness-specific fs walk and stamp the resulting fragment
/// with the harness key + wall-clock epoch.
///
/// This helper folds that envelope so each adapter's
/// `HarnessAdapter::discover` becomes a one-line delegation to
/// its harness-specific `discover_state` fn.
pub fn discover_with_state_root<F>(
    context: &DiscoveryContext,
    harness_key: &'static str,
    inner: F,
) -> Result<GraphFragment>
where
    F: FnOnce(&Path) -> Result<GraphFragment>,
{
    let Some(state_root) = context.harness_state_root(harness_key) else {
        return Ok(GraphFragment::empty());
    };
    let mut fragment = inner(state_root)?;
    crate::discovery::stamp_fragment(
        &mut fragment,
        harness_key,
        crate::discovery::current_epoch(),
    );
    Ok(fragment)
}

/// Iterate every registered harness adapter (H-EXT-002). Used by
/// TUI filter menus, launch / resume dispatch, row-label lookup,
/// and the launch-options aggregator. Order is stable and
/// matches [`HarnessDiscovery::with_default_adapters`] so
/// registry-derived UI surfaces don't reshuffle when a new
/// adapter is added mid-list.
pub fn registered_adapters() -> impl Iterator<Item = &'static dyn HarnessAdapter> {
    REGISTERED_ADAPTERS.iter().map(|b| &**b)
}

/// Find a registered adapter by key (H-EXT-002). Returns `None`
/// when the key doesn't match any registered adapter.
fn adapter_for(harness_key: &str) -> Option<&'static dyn HarnessAdapter> {
    registered_adapters().find(|a| a.harness_key() == harness_key)
}

/// Look up the per-harness default launch argv. Convenience for the
/// CLI launch path so it can resolve `pin.harness` → argv without
/// re-instantiating an adapter or walking the discovery registry.
pub fn launch_argv_for(harness_key: &str) -> Vec<std::ffi::OsString> {
    adapter_for(harness_key)
        .map(|a| a.launch_argv())
        .unwrap_or_default()
}

pub fn launch_options_for(harness_key: &str) -> &'static [HarnessLaunchOption] {
    adapter_for(harness_key).map_or(&[], |a| a.launch_options())
}

pub fn launch_option_for(harness_key: &str, option_id: &str) -> Option<HarnessLaunchOption> {
    launch_options_for(harness_key)
        .iter()
        .copied()
        .find(|option| option.id == option_id)
}

/// Registered harness keys in declaration order (H-EXT-002).
/// Used by the TUI controls overlay's harness filter menu — the
/// pre-H-EXT-002 caller was a hardcoded
/// `HARNESS_OPTIONS: &[&str]` in `tui/widgets/controls.rs`.
///
/// Wrapped in a `LazyLock<Vec<&'static str>>` so the &-of-slice
/// return type is `'static` (the multi-select widget needs
/// `&[&'static str]`).
pub fn harness_keys() -> &'static [&'static str] {
    static KEYS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
        REGISTERED_ADAPTERS
            .iter()
            .map(|a| a.harness_key())
            .collect()
    });
    KEYS.as_slice()
}

/// Short display label for a harness key. Used by the row-label
/// column (H-TBL-014, H-EXT-002). Returns the key itself as an
/// owned string for unknown harnesses (previous behavior via the
/// `harness_label` catch-all in `tui/rows/mod.rs`).
pub fn display_label_for(harness_key: &str) -> String {
    match adapter_for(harness_key) {
        Some(a) => a.display_label().to_string(),
        None => harness_key.to_string(),
    }
}

pub fn argv_contains_fragment(argv: &[String], fragment: &[&str]) -> bool {
    if fragment.is_empty() {
        return false;
    }
    argv.windows(fragment.len()).any(|window| {
        window
            .iter()
            .zip(fragment.iter())
            .all(|(actual, expected)| actual == expected)
    })
}

pub fn argv_with_fragment(mut argv: Vec<String>, fragment: &[&str]) -> Vec<String> {
    if fragment.is_empty() || argv_contains_fragment(&argv, fragment) {
        return argv;
    }
    argv.extend(fragment.iter().map(|arg| (*arg).to_string()));
    argv
}

pub fn argv_without_fragment(argv: Vec<String>, fragment: &[&str]) -> Vec<String> {
    if fragment.is_empty() {
        return argv;
    }
    let mut out = Vec::with_capacity(argv.len());
    let mut idx = 0;
    while idx < argv.len() {
        if idx + fragment.len() <= argv.len()
            && argv[idx..idx + fragment.len()]
                .iter()
                .zip(fragment.iter())
                .all(|(actual, expected)| actual == expected)
        {
            idx += fragment.len();
        } else {
            out.push(argv[idx].clone());
            idx += 1;
        }
    }
    out
}

pub fn strip_known_launch_option_fragments(argv: Vec<String>) -> Vec<String> {
    let mut stripped = argv;
    for adapter in registered_adapters() {
        for option in adapter.launch_options() {
            stripped = argv_without_fragment(stripped, option.argv);
        }
    }
    stripped
}

/// Look up the per-harness resume argv. Sibling of [`launch_argv_for`]
/// for the H-PIN-RESUME-004 launch path. Returns `None` when the
/// harness key is unknown or the adapter does not expose a resume
/// command (currently only `aider`, which tracks chat history
/// per-cwd rather than per-session).
pub fn resume_argv_for(
    harness_key: &str,
    session_id: &str,
    cwd: &Path,
) -> Option<Vec<std::ffi::OsString>> {
    adapter_for(harness_key).and_then(|a| a.resume_argv(session_id, cwd))
}

/// Splice a harness `resume_argv` into a pin's configured launch argv
/// (ADR 0098). The first token of `resume` names the harness binary;
/// the remaining tokens (`--resume <id>`, `exec --resume <id>`, ...)
/// are inserted immediately after the first `base` token whose file
/// name matches that binary. Wrapper prefixes (`atelier exec`,
/// `nono run --`) and trailing launch options
/// (`--dangerously-skip-permissions`) are preserved in place.
///
/// Returns `None` when `base` never invokes the harness binary (e.g.
/// an opaque shell script); callers fall back to a fresh launch with
/// `base` rather than discarding the operator's argv.
pub fn splice_resume_argv(
    base: &[std::ffi::OsString],
    resume: &[std::ffi::OsString],
) -> Option<Vec<std::ffi::OsString>> {
    let (binary, resume_args) = resume.split_first()?;
    let binary = Path::new(binary).file_name()?;
    let idx = base
        .iter()
        .position(|token| Path::new(token).file_name() == Some(binary))?;
    let mut out = Vec::with_capacity(base.len() + resume_args.len());
    out.extend_from_slice(&base[..=idx]);
    out.extend_from_slice(resume_args);
    out.extend_from_slice(&base[idx + 1..]);
    Some(out)
}

#[derive(Default)]
pub struct HarnessDiscovery {
    adapters: Vec<Box<dyn HarnessAdapter>>,
}

impl HarnessDiscovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_adapter(mut self, adapter: impl HarnessAdapter + 'static) -> Self {
        self.adapters.push(Box::new(adapter));
        self
    }

    pub fn with_default_adapters() -> Self {
        Self::new()
            .with_adapter(CodexAdapter::new())
            .with_adapter(ClaudeCodeAdapter::new())
            .with_adapter(OpenCodeAdapter::new())
            .with_adapter(AiderAdapter::new())
    }
}

impl DiscoveryProvider for HarnessDiscovery {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let mut fragments = Vec::with_capacity(self.adapters.len());

        for adapter in &self.adapters {
            fragments.push(adapter.discover(context)?);
        }

        Ok(GraphFragment::from(merge_fragments(fragments)))
    }
}

#[cfg(test)]
mod registry_tests {
    //! H-EXT-002 anchor tests. These pin the registry-derived
    //! surfaces (label lookup, launch options, keys list, resume
    //! argv) so a rename or accidental removal of an adapter's
    //! override breaks a targeted test rather than a downstream
    //! snapshot.
    use super::*;

    #[test]
    fn harness_keys_matches_registered_adapter_order() {
        // Ordering matches the pre-H-EXT-002 hardcoded
        // `HARNESS_OPTIONS` slice so the TUI filter menu stays
        // byte-identical.
        assert_eq!(
            harness_keys(),
            &["claude-code", "codex", "opencode", "aider"]
        );
    }

    #[test]
    fn display_label_for_collapses_claude_code() {
        // H-TBL-014 collapse lives on the adapter (H-EXT-002)
        // rather than a match table in `tui/rows`.
        assert_eq!(display_label_for("claude-code"), "claude");
    }

    #[test]
    fn display_label_for_defaults_to_key() {
        assert_eq!(display_label_for("codex"), "codex");
        assert_eq!(display_label_for("opencode"), "opencode");
        assert_eq!(display_label_for("aider"), "aider");
    }

    #[test]
    fn display_label_for_unknown_echoes_key() {
        // Preserves the pre-H-EXT-002 catch-all behavior in
        // `harness_label`.
        assert_eq!(display_label_for("never-registered"), "never-registered");
    }

    #[test]
    fn launch_options_for_carries_permission_overrides() {
        // codex and claude-code both surface a skip-permissions
        // option; the others don't.
        assert_eq!(launch_options_for("codex").len(), 1);
        assert_eq!(launch_options_for("codex")[0].id, "skip-permissions");
        assert_eq!(launch_options_for("claude-code").len(), 1);
        assert_eq!(launch_options_for("claude-code")[0].id, "skip-permissions");
        assert!(launch_options_for("opencode").is_empty());
        assert!(launch_options_for("aider").is_empty());
        assert!(launch_options_for("never-registered").is_empty());
    }

    #[test]
    fn resume_argv_for_dispatches_via_registry() {
        let cwd = Path::new("/tmp");
        assert_eq!(
            resume_argv_for("claude-code", "sess-123", cwd),
            Some(vec![
                std::ffi::OsString::from("claude"),
                std::ffi::OsString::from("--resume"),
                std::ffi::OsString::from("sess-123"),
            ])
        );
        assert!(resume_argv_for("aider", "sess-123", cwd).is_none());
        assert!(resume_argv_for("never-registered", "sess", cwd).is_none());
    }

    fn os(tokens: &[&str]) -> Vec<std::ffi::OsString> {
        tokens.iter().map(std::ffi::OsString::from).collect()
    }

    #[test]
    fn splice_resume_argv_default_argv_matches_bare_resume() {
        assert_eq!(
            splice_resume_argv(&os(&["claude"]), &os(&["claude", "--resume", "s"])),
            Some(os(&["claude", "--resume", "s"]))
        );
    }

    #[test]
    fn splice_resume_argv_preserves_wrapper_and_launch_options() {
        // ADR 0098: `atelier exec` prefix and skip-permissions survive.
        assert_eq!(
            splice_resume_argv(
                &os(&[
                    "atelier",
                    "exec",
                    "claude",
                    "--dangerously-skip-permissions"
                ]),
                &os(&["claude", "--resume", "s"]),
            ),
            Some(os(&[
                "atelier",
                "exec",
                "claude",
                "--resume",
                "s",
                "--dangerously-skip-permissions",
            ]))
        );
    }

    #[test]
    fn splice_resume_argv_places_subcommand_right_after_binary() {
        // codex resume is a subcommand; options must follow it.
        assert_eq!(
            splice_resume_argv(
                &os(&["codex", "--dangerously-bypass-approvals-and-sandbox"]),
                &os(&["codex", "exec", "--resume", "s"]),
            ),
            Some(os(&[
                "codex",
                "exec",
                "--resume",
                "s",
                "--dangerously-bypass-approvals-and-sandbox",
            ]))
        );
    }

    #[test]
    fn splice_resume_argv_matches_binary_by_file_name() {
        assert_eq!(
            splice_resume_argv(
                &os(&["/nix/store/abc/bin/claude", "--verbose"]),
                &os(&["claude", "--resume", "s"]),
            ),
            Some(os(&[
                "/nix/store/abc/bin/claude",
                "--resume",
                "s",
                "--verbose"
            ]))
        );
    }

    #[test]
    fn splice_resume_argv_none_when_binary_absent() {
        assert_eq!(
            splice_resume_argv(
                &os(&["./start-agent.sh"]),
                &os(&["claude", "--resume", "s"])
            ),
            None
        );
        assert_eq!(splice_resume_argv(&os(&["claude"]), &[]), None);
    }

    #[test]
    fn launch_argv_for_dispatches_via_registry() {
        // Registered adapters return non-empty argv; unknown
        // harnesses return an empty vec (matches the pre-H-EXT-002
        // catch-all).
        for key in harness_keys() {
            assert!(
                !launch_argv_for(key).is_empty(),
                "registered adapter `{key}` returned an empty launch argv",
            );
        }
        assert!(launch_argv_for("never-registered").is_empty());
    }

    #[test]
    fn strip_known_launch_option_fragments_walks_every_registered_adapter() {
        // Given argv that contains a fragment from every adapter
        // with launch options, `strip_known_launch_option_fragments`
        // removes all of them.
        let argv = vec![
            "claude".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
            "extra".to_string(),
        ];
        let stripped = strip_known_launch_option_fragments(argv);
        assert_eq!(stripped, vec!["claude".to_string(), "extra".to_string()]);
    }
}

#[cfg(test)]
#[path = "harness_tests.rs"]
mod tests;
