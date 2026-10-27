//! Pair snapshot sessions with the harness that can read their transcripts.

use crate::discovery::harness::registered_adapters;
use crate::model::AgentSessionId;

use super::{ScanSession, SessionKey};

/// The sessions a scan will read, and how many were left out.
#[derive(Clone, Default)]
pub struct Plan {
    pub sessions: Vec<ScanSession>,
    /// Sessions whose harness has no transcript parser. They can't be
    /// searched and are reported, not hidden.
    pub parserless: usize,
}

/// Build the scan list from the agent sessions in a snapshot.
pub fn plan_sessions<'a>(sessions: impl IntoIterator<Item = &'a AgentSessionId>) -> Plan {
    let mut plan = Plan::default();
    for id in sessions {
        let Some(adapter) = registered_adapters().find(|a| a.harness_key() == id.harness_key)
        else {
            plan.parserless += 1;
            continue;
        };
        let (Some(locator), Some(parser)) =
            (adapter.transcript_source(id), adapter.transcript_parser())
        else {
            plan.parserless += 1;
            continue;
        };
        plan.sessions.push(ScanSession {
            key: SessionKey {
                harness_key: id.harness_key.clone(),
                session_key: id.session_key.clone(),
            },
            locator,
            parser,
        });
    }
    plan
}
