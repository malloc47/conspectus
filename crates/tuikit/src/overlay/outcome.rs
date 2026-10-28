//! What one key press in an overlay asks the host to do.

/// What an overlay asks its host to do after one key press.
///
/// An overlay never acts on the world; it reports what the key meant.
/// The host's stack does the stack's part ([`ModalStack::settle`]
/// closes the top entry for `Close` and `Commit`), and the host handles
/// the message `M` that `Commit` and `Stay` carry.
///
/// How the close keys map (house rule 4): `Esc` steps back one level,
/// which is `Consumed` while the overlay has a level to return to and
/// `Close` at its first; `Ctrl-C`, and `q` where nothing is typed, are
/// `Close` from any level. Closing is silent (house rule 5): a host
/// posts no message for `Close`, because cancelling is not an event.
///
/// [`ModalStack::settle`]: super::ModalStack::settle
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub enum Outcome<M> {
    /// The overlay took the key and stays open; nothing else happens.
    Consumed,
    /// The overlay is done and closes with nothing to report: cancelled,
    /// dismissed, or left.
    Close,
    /// The overlay is done with a result: it closes, then the host
    /// handles `M`.
    Commit(M),
    /// The overlay stays open and the host handles `M`: a setting applied
    /// as it changes, or a notice about input it refused.
    Stay(M),
}

impl<M> Outcome<M> {
    /// The same outcome with its message mapped by `f`: how a control
    /// that reports its own result type becomes one that reports the
    /// host's message.
    pub fn map<N>(self, f: impl FnOnce(M) -> N) -> Outcome<N> {
        match self {
            Self::Consumed => Outcome::Consumed,
            Self::Close => Outcome::Close,
            Self::Commit(message) => Outcome::Commit(f(message)),
            Self::Stay(message) => Outcome::Stay(f(message)),
        }
    }

    /// Whether the overlay closes: `Close` or `Commit`.
    pub fn closes(&self) -> bool {
        matches!(self, Self::Close | Self::Commit(_))
    }

    /// The message the host handles, if the outcome carries one (`Commit`
    /// or `Stay`).
    pub fn message(self) -> Option<M> {
        match self {
            Self::Commit(message) | Self::Stay(message) => Some(message),
            Self::Consumed | Self::Close => None,
        }
    }
}
