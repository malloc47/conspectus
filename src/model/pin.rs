//! Session pins as the resolver sees them: candidates, their mux reference, and the binding it computes.

use super::*;

/// ADR 0058 §Q5: the recorded last-bound session a `PinUnbound`
/// diagnostic optionally carries. Drives the launch path's
/// "Enter to resume X" hint surface.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PinLastSession {
    pub session_id: String,
    pub observed_epoch: i64,
}

/// ADR 0102: a session a pin's mux had evidence for, but which a
/// different pin bound first on stronger evidence. Carried on
/// [`Diagnostic::PinStaleMux`].
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PinSessionClaim {
    pub session: AgentSessionId,
    pub claimed_by_pin: String,
}

/// Snapshot-resident pin record loaded from `[[pins.entries]]` TOML
/// per ADR 0057. Carries the declarative fields (ADR schema) plus the
/// loader-side provenance and store path. `binding` is populated by
/// the resolver pass ([`crate::resolve::pins`]) once mux lookup and
/// harness attribution have run; pre-resolve snapshots leave it as
/// `None`.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PinCandidate {
    /// Pin id from the TOML entry; unique within its store.
    pub id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux: PinMuxRef,
    /// Per-pin launch argv override. `None` means the harness
    /// adapter's `launch_argv` default applies at launch time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_argv: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `Provenance::LocalPin` or `Provenance::GlobalPin`.
    pub provenance: Provenance,
    /// Absolute path of the TOML file the pin was loaded from.
    pub store_path: String,
    /// Resolver-populated binding state per ADR 0057. `None` after
    /// loader-only discovery; populated by the resolver pass.
    /// `Some(PinBinding::Unbound)` means the resolver ran and found
    /// no live mux matching `mux.native_id()`; `Some(PinBinding::Bound)`
    /// means the resolver found a live mux and attributed a harness
    /// session to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<PinBinding>,
}

/// Resolver-determined binding state for a [`PinCandidate`].
/// Ambiguous-binding cases resolve to `Bound` for the resolver's
/// preferred candidate and emit a parallel `Diagnostic::PinAmbiguous`
/// listing the competing candidates so the operator can override.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PinBinding {
    /// The mux exists and a `pin.harness` session was attributed to
    /// it. The resolver synthesizes a `LinkedToMux` candidate carrying
    /// the pin's provenance (`LocalPin` / `GlobalPin`).
    Bound {
        mux: MuxSessionId,
        session: AgentSessionId,
    },
    /// The mux exists but no live `pin.harness` session is attributed
    /// to it. Operator action: relaunch the harness inside the
    /// existing mux.
    StaleMux { mux: MuxSessionId },
    /// No mux matching `pin.mux.native_id()` exists in the current
    /// snapshot. Operator action: launch the pin to create the mux.
    Unbound,
}

/// Mux backend coordinates carried inside [`PinCandidate`]. Mirrors
/// the schema in `crate::pins::PinMux` but lives in the model
/// crate so consumers can read it without depending on the
/// pin-schema module.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct PinMuxRef {
    pub backend: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_name: Option<String>,
}

impl PinMuxRef {
    /// Mux native id encoding per ADR 0057 (mirrors
    /// `crate::pins::PinMux::native_id` without taking a
    /// schema-module dependency).
    pub fn native_id(&self) -> String {
        match self.effective_socket() {
            None => format!("{}:{}", self.backend, self.name),
            Some(socket) => format!("{}:{}:{}", self.backend, socket, self.name),
        }
    }

    /// Collapses `None` and the `"default"` sentinel to `None`.
    pub fn effective_socket(&self) -> Option<&str> {
        match self.socket_name.as_deref() {
            None | Some("default") => None,
            Some(name) => Some(name),
        }
    }
}
