//! What one key press in an overlay asks the host to do.

/// What an overlay asks its host to do after one key press.
///
/// An overlay never acts on the world; it reports what the key meant.
/// The host's stack does the stack's part ([`ModalStack::settle`]
/// closes the top entry for `Close` and `Back`, and for `Commit` and
/// `Dismiss` the top entry with every entry it is a step of), and the
/// host handles the message `M` that `Commit`, `Back`, and `Stay` carry.
///
/// How the close keys map (house rule 4): `Esc` steps back one level,
/// which is `Consumed` while the overlay has a level of its own to return
/// to and `Close` at its first (on a stack, a step's `Close` returns to
/// the entry under it); `Ctrl-C`, and `q` where nothing is typed, are
/// `Dismiss` from any level, which closes the whole overlay, every step
/// of it included. Closing is silent (house rule 5): a host posts no
/// message for `Close` or `Dismiss`, because cancelling is not an event.
///
/// [`ModalStack::settle`]: super::ModalStack::settle
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome<M> {
    /// The overlay took the key and stays open; nothing else happens.
    Consumed,
    /// The overlay is done and closes with nothing to report: cancelled,
    /// or left one level back (a step returns to the entry under it).
    Close,
    /// The user dismissed the whole overlay (`Ctrl-C`, or `q` where
    /// nothing is typed): it closes with nothing to report, and so does
    /// every entry it is a step of. For an overlay that is no step, the
    /// same as `Close`.
    Dismiss,
    /// The overlay is done with a result: it closes, and so does every
    /// entry it is a step of, then the host handles `M`.
    Commit(M),
    /// The overlay steps back one level with a result: it closes as
    /// `Close` closes it (a step gives the keys back to the entry under
    /// it, which stays open), then the host handles `M`. A settings
    /// panel's editor answers this way, so the change applies and the
    /// panel is there again. For an overlay that is no step, the same as
    /// `Commit`.
    Back(M),
    /// The overlay stays open and the host handles `M`: a setting applied
    /// as it changes, a notice about input it refused, or a request to
    /// open the overlay's next step over it.
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
            Self::Dismiss => Outcome::Dismiss,
            Self::Commit(message) => Outcome::Commit(f(message)),
            Self::Back(message) => Outcome::Back(f(message)),
            Self::Stay(message) => Outcome::Stay(f(message)),
        }
    }

    /// Whether the overlay closes: `Close`, `Dismiss`, `Commit`, or
    /// `Back`.
    pub fn closes(&self) -> bool {
        matches!(
            self,
            Self::Close | Self::Dismiss | Self::Commit(_) | Self::Back(_)
        )
    }

    /// The message the host handles, if the outcome carries one
    /// (`Commit`, `Back`, or `Stay`).
    pub fn message(self) -> Option<M> {
        match self {
            Self::Commit(message) | Self::Back(message) | Self::Stay(message) => Some(message),
            Self::Consumed | Self::Close | Self::Dismiss => None,
        }
    }
}
