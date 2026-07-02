//! Effects (ADR 0085 contract 2): data values the reducer emits
//! to schedule side effects. The runtime owns the effect executor,
//! which is the only code that touches `&mut Terminal`, the mux
//! runner, `std::process`, or the filesystem. `App::update` returns
//! `Vec<Effect>`; the reducer never blocks on I/O.
//!
//! The initial catalog covers the effects the H-TUI-001 / H-TUI-002
//! landing wave needed. The reserved variants below are named so
//! future migrations (`H-TUI-003+`, terminal-suspending exec, mux
//! ops, store writes, preview capture) only add executor cases
//! rather than growing the enum shape.

/// A side effect the reducer wants the runtime to schedule.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Ask the runtime to run a fresh discovery pass.
    ///
    /// `force_local` bypasses the daemon and runs a full cold scan
    /// — used after store-mutation effects so the resulting graph
    /// reflects the just-written change without waiting for the
    /// daemon's next tick.
    SpawnRefresh { force_local: bool },
    /// Post a transient status-bar toast. Non-blocking; input
    /// continues to flow to the underlying view.
    Toast(String),
    /// Persist the current TUI state to disk (F8-013). Best-effort;
    /// failures are silently swallowed by the executor.
    Persist,
    /// Quit the event loop after the current frame.
    Quit,
    // Reserved for later H-TUI-002 waves — leaving them out of the
    // enum until the executor learns to run them. See the module
    // doc comment for the roadmap.
}
