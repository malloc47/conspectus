//! Structured row-filter predicates shared by the CLI `table`
//! projection and the TUI row-tree builders.
//!
//! Per ADR 0031, filters are persistent typed predicates that narrow
//! the visible row set along structured dimensions; they compose with
//! the transient `/` fuzzy search (T8-017) but never replace it. v1
//! covers `harness`, `max-age`, and `mux-state`; further dimensions
//! land as additional fields without amending the ADR.
//!
//! The predicate type is intentionally non-TUI so `src/output/` can
//! consume it directly. It introduces no new dependencies (ADR 0024
//! policy).
//!
//! Evaluation is driven by [`SessionMatchInputs`], a small struct the
//! caller assembles for each row. The caller owns the cost of looking
//! up mux state, recency, etc., so the predicate stays pure.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Set of independent dimension constraints. Each `None` field means
/// "no constraint on this dimension"; multiple fields AND together at
/// evaluation time.
///
/// In addition to the narrowing predicates, the struct also carries
/// view-scoped *ordering* toggles surfaced through the same Controls
/// overlay (ADR 0031, amendment 2026-05-29). These do not change which
/// rows are visible — they re-order them — and so are skipped by
/// [`Self::matches_session`]. They are included in [`Self::is_empty`]
/// so the modal's "Clear all" affordance resets them alongside the
/// narrowing dimensions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RowFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness: Option<HarnessFilter>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "duration_secs"
    )]
    pub max_age: Option<Duration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mux_state: Option<MuxStateFilter>,
    /// Sessions view: float agent sessions resolved to a mux above
    /// sessions that aren't. Within each of the two resulting groups
    /// the existing within-group sort order is preserved.
    #[serde(skip_serializing_if = "is_false")]
    pub float_muxed_sessions_top: bool,
    /// Mux view: float mux sessions with at least one attached agent
    /// session above muxes with none. Within each group the existing
    /// within-group sort order is preserved.
    #[serde(skip_serializing_if = "is_false")]
    pub float_attached_muxes_top: bool,
}

impl RowFilter {
    /// True when no constraints or ordering toggles are active. The
    /// modal uses this to decide whether "Clear all" has anything to
    /// clear; callers gating *row-narrowing work* (predicate
    /// evaluation, empty-bucket suppression) should use
    /// [`Self::has_narrowing_predicates`] instead so ordering-only
    /// state doesn't change the visible row set.
    pub fn is_empty(&self) -> bool {
        !self.has_narrowing_predicates()
            && !self.float_muxed_sessions_top
            && !self.float_attached_muxes_top
    }

    /// True when at least one narrowing dimension is set. Distinct
    /// from [`Self::is_empty`] in that ordering toggles do not count
    /// — they re-order rows but never drop them.
    pub fn has_narrowing_predicates(&self) -> bool {
        self.harness.is_some() || self.max_age.is_some() || self.mux_state.is_some()
    }

    /// Evaluate the filter against an agent session's per-row data.
    /// Returns `true` when the row passes (or no constraint applies).
    pub fn matches_session(&self, inputs: &SessionMatchInputs<'_>) -> bool {
        if let Some(harness) = &self.harness
            && !harness.matches(inputs.harness_key)
        {
            return false;
        }
        if let Some(max_age) = self.max_age
            && !matches_max_age(inputs.now_epoch, inputs.last_active_epoch, max_age)
        {
            return false;
        }
        if let Some(mux_state) = &self.mux_state
            && !mux_state.matches(inputs.mux_state)
        {
            return false;
        }
        true
    }
}

/// Set membership over harness keys. An empty set matches nothing
/// (callers should pass `None` instead to express "no constraint").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessFilter {
    Any(Vec<String>),
}

impl HarnessFilter {
    /// Construct from any iterator of strings. Trims whitespace and
    /// lowercases each entry so CLI input like `Claude` and
    /// ` codex ` are treated as the canonical lower-case form the
    /// adapters use.
    pub fn from_values<I, S>(values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut canonical: Vec<String> = values
            .into_iter()
            .map(|s| s.as_ref().trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        canonical.sort();
        canonical.dedup();
        Self::Any(canonical)
    }

    fn matches(&self, harness_key: &str) -> bool {
        let needle = harness_key.trim().to_ascii_lowercase();
        match self {
            Self::Any(values) => values.iter().any(|v| v == &needle),
        }
    }

    /// Canonical sorted values, useful for chip rendering and config
    /// round-trip.
    pub fn values(&self) -> &[String] {
        match self {
            Self::Any(values) => values,
        }
    }
}

/// Set membership over derived mux states. Empty set matches nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MuxStateFilter {
    Any(Vec<MuxStateKey>),
}

impl MuxStateFilter {
    pub fn from_values<I>(values: I) -> Self
    where
        I: IntoIterator<Item = MuxStateKey>,
    {
        let mut canonical: Vec<MuxStateKey> = values.into_iter().collect();
        canonical.sort();
        canonical.dedup();
        Self::Any(canonical)
    }

    fn matches(&self, state: MuxStateKey) -> bool {
        match self {
            Self::Any(values) => values.contains(&state),
        }
    }

    pub fn values(&self) -> &[MuxStateKey] {
        match self {
            Self::Any(values) => values,
        }
    }
}

/// Coarse mux state derived from active `LinkedToMux` candidate
/// count. Mirrors `MuxIndicator` in the TUI row tree but lives in the
/// shared crate so non-TUI callers can use it without pulling in the
/// TUI module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MuxStateKey {
    Attached,
    Ambiguous,
    Unmuxed,
}

impl MuxStateKey {
    /// Parse from the CLI / config text form. Returns `None` for any
    /// unknown spelling so the caller can raise an actionable error.
    pub fn from_str_ci(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "attached" => Some(Self::Attached),
            "ambiguous" => Some(Self::Ambiguous),
            "unmuxed" => Some(Self::Unmuxed),
            _ => None,
        }
    }

    /// Stable string form used in chips, config, and CLI output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Attached => "attached",
            Self::Ambiguous => "ambiguous",
            Self::Unmuxed => "unmuxed",
        }
    }

    /// Derive from the active mux-candidate count for one session.
    /// Mirrors the TUI's `MuxIndicator` mapping so both surfaces
    /// agree.
    pub fn from_candidate_count(count: usize) -> Self {
        match count {
            0 => Self::Unmuxed,
            1 => Self::Attached,
            _ => Self::Ambiguous,
        }
    }
}

/// Per-row data the predicate evaluates against. Callers assemble
/// one of these for each candidate row; the predicate is read-only.
#[derive(Clone, Copy, Debug)]
pub struct SessionMatchInputs<'a> {
    pub harness_key: &'a str,
    /// Wall-clock epoch (Unix seconds) used as the recency anchor.
    /// `None` disables age-based filtering for the row (the predicate
    /// treats absent `now` as "no constraint can be evaluated" and
    /// admits the row).
    pub now_epoch: Option<i64>,
    /// Session's last-active timestamp (Unix seconds).
    pub last_active_epoch: Option<i64>,
    pub mux_state: MuxStateKey,
}

fn matches_max_age(
    now_epoch: Option<i64>,
    last_active_epoch: Option<i64>,
    max_age: Duration,
) -> bool {
    // Without either side of the comparison we can't evaluate the
    // constraint; admit the row rather than silently dropping it.
    let (Some(now), Some(last)) = (now_epoch, last_active_epoch) else {
        return true;
    };
    if now < last {
        // Clock skew — treat as fresh.
        return true;
    }
    let age_secs = (now - last) as u64;
    let max_secs = max_age.as_secs();
    age_secs <= max_secs
}

/// Serde helper that serializes `Option<Duration>` as optional u64
/// seconds for human-readable state files.
mod duration_secs {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(dur: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match dur {
            Some(d) => d.as_secs().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs: Option<u64> = Option::deserialize(deserializer)?;
        Ok(secs.map(Duration::from_secs))
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs<'a>(
        harness: &'a str,
        mux: MuxStateKey,
        last_active: Option<i64>,
    ) -> SessionMatchInputs<'a> {
        SessionMatchInputs {
            harness_key: harness,
            now_epoch: Some(1_000_000),
            last_active_epoch: last_active,
            mux_state: mux,
        }
    }

    #[test]
    fn empty_filter_admits_everything() {
        let filter = RowFilter::default();
        assert!(filter.is_empty());
        assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Attached, Some(1))));
    }

    #[test]
    fn harness_filter_set_membership() {
        let filter = RowFilter {
            harness: Some(HarnessFilter::from_values(["claude-code", "codex"])),
            ..RowFilter::default()
        };
        assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Attached, Some(1))));
        assert!(filter.matches_session(&inputs("codex", MuxStateKey::Attached, Some(1))));
        assert!(!filter.matches_session(&inputs("opencode", MuxStateKey::Attached, Some(1))));
    }

    #[test]
    fn harness_filter_is_case_insensitive_and_trimmed() {
        let filter = RowFilter {
            harness: Some(HarnessFilter::from_values(["  Claude-Code  "])),
            ..RowFilter::default()
        };
        assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Attached, Some(1))));
        assert!(filter.matches_session(&inputs("CLAUDE-CODE", MuxStateKey::Attached, Some(1))));
    }

    #[test]
    fn harness_filter_canonicalizes_values() {
        let filter = HarnessFilter::from_values(["codex", "Claude-Code", "codex", "  "]);
        match filter {
            HarnessFilter::Any(v) => assert_eq!(v, vec!["claude-code", "codex"]),
        }
    }

    #[test]
    fn max_age_filter_at_boundary() {
        // now = 1_000_000, last = 1_000_000 - 7 days = 395200
        let week = Duration::from_secs(7 * 24 * 60 * 60);
        let filter = RowFilter {
            max_age: Some(week),
            ..RowFilter::default()
        };
        // Exactly at the boundary admits the row.
        let just_in = SessionMatchInputs {
            harness_key: "claude-code",
            now_epoch: Some(1_000_000),
            last_active_epoch: Some(1_000_000 - week.as_secs() as i64),
            mux_state: MuxStateKey::Attached,
        };
        assert!(filter.matches_session(&just_in));
        // One second older fails.
        let just_out = SessionMatchInputs {
            last_active_epoch: Some(1_000_000 - week.as_secs() as i64 - 1),
            ..just_in
        };
        assert!(!filter.matches_session(&just_out));
    }

    #[test]
    fn max_age_admits_when_timestamps_missing() {
        let filter = RowFilter {
            max_age: Some(Duration::from_secs(60)),
            ..RowFilter::default()
        };
        let no_last = SessionMatchInputs {
            harness_key: "claude-code",
            now_epoch: Some(1_000_000),
            last_active_epoch: None,
            mux_state: MuxStateKey::Attached,
        };
        assert!(filter.matches_session(&no_last));
        let no_now = SessionMatchInputs {
            now_epoch: None,
            last_active_epoch: Some(0),
            ..no_last
        };
        assert!(filter.matches_session(&no_now));
    }

    #[test]
    fn max_age_admits_on_clock_skew() {
        let filter = RowFilter {
            max_age: Some(Duration::from_secs(60)),
            ..RowFilter::default()
        };
        let future = SessionMatchInputs {
            harness_key: "claude-code",
            now_epoch: Some(1_000),
            last_active_epoch: Some(2_000),
            mux_state: MuxStateKey::Attached,
        };
        assert!(filter.matches_session(&future));
    }

    #[test]
    fn mux_state_filter_set_membership() {
        let filter = RowFilter {
            mux_state: Some(MuxStateFilter::from_values([
                MuxStateKey::Unmuxed,
                MuxStateKey::Ambiguous,
            ])),
            ..RowFilter::default()
        };
        assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Unmuxed, Some(1))));
        assert!(filter.matches_session(&inputs("claude-code", MuxStateKey::Ambiguous, Some(1))));
        assert!(!filter.matches_session(&inputs("claude-code", MuxStateKey::Attached, Some(1))));
    }

    #[test]
    fn mux_state_key_round_trips() {
        for key in [
            MuxStateKey::Attached,
            MuxStateKey::Ambiguous,
            MuxStateKey::Unmuxed,
        ] {
            assert_eq!(MuxStateKey::from_str_ci(key.as_str()), Some(key));
        }
        assert_eq!(
            MuxStateKey::from_str_ci("ATTACHED"),
            Some(MuxStateKey::Attached)
        );
        assert_eq!(
            MuxStateKey::from_str_ci(" unmuxed "),
            Some(MuxStateKey::Unmuxed)
        );
        assert_eq!(MuxStateKey::from_str_ci("nope"), None);
    }

    #[test]
    fn mux_state_from_candidate_count() {
        assert_eq!(MuxStateKey::from_candidate_count(0), MuxStateKey::Unmuxed);
        assert_eq!(MuxStateKey::from_candidate_count(1), MuxStateKey::Attached);
        assert_eq!(MuxStateKey::from_candidate_count(2), MuxStateKey::Ambiguous);
        assert_eq!(MuxStateKey::from_candidate_count(7), MuxStateKey::Ambiguous);
    }

    #[test]
    fn intersection_of_all_dimensions() {
        let filter = RowFilter {
            harness: Some(HarnessFilter::from_values(["claude-code"])),
            max_age: Some(Duration::from_secs(60)),
            mux_state: Some(MuxStateFilter::from_values([MuxStateKey::Unmuxed])),
            ..RowFilter::default()
        };
        let good = SessionMatchInputs {
            harness_key: "claude-code",
            now_epoch: Some(1_000),
            last_active_epoch: Some(970),
            mux_state: MuxStateKey::Unmuxed,
        };
        assert!(filter.matches_session(&good));
        // Wrong harness.
        assert!(!filter.matches_session(&SessionMatchInputs {
            harness_key: "codex",
            ..good
        }));
        // Too old.
        assert!(!filter.matches_session(&SessionMatchInputs {
            last_active_epoch: Some(900),
            ..good
        }));
        // Wrong mux state.
        assert!(!filter.matches_session(&SessionMatchInputs {
            mux_state: MuxStateKey::Attached,
            ..good
        }));
    }
}
