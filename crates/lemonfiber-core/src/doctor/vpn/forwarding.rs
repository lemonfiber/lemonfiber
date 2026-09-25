//! What a forwarded port needs to be true, checked one thing at a time.
//!
//! Four separate questions, because they fail separately and an operator told
//! only that "port forwarding is not working" learns nothing about which one to
//! look at. The provider may have granted nothing; it may have granted a port the
//! client is not listening on; the client may be listening correctly and still be
//! unreachable from outside; and the whole arrangement may have been true this
//! morning and stopped being true when the tunnel reconnected on a new port.
//!
//! Only the first two can be established from here. Reachability needs somebody
//! outside to try, and lemonfiber does not have one — so it says so rather than
//! inferring it from the two facts it does have, which would be the comfortable
//! falsehood this whole subsystem exists to remove.

use serde::{Deserialize, Serialize};

/// What is known about the forwarded port, one fact at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forwarding {
    /// The port the provider granted, where it granted one.
    pub granted: Option<u16>,
    /// The port the download client is listening on, where it could be asked.
    pub listening: Option<u16>,
}

/// Whether one of the four questions is settled, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Answer {
    /// True, and established rather than assumed.
    Yes,
    /// False, and established.
    No,
    /// Not established either way from here.
    Unknown,
}

impl Forwarding {
    /// Whether the provider granted a port at all.
    #[must_use]
    pub const fn assigned(self) -> Answer {
        match self.granted {
            Some(_) => Answer::Yes,
            None => Answer::No,
        }
    }

    /// Whether the client is listening on the port that was granted.
    ///
    /// Unknown where either side could not be read: a client that would not answer
    /// says nothing about which port it is on, and reporting that as a mismatch
    /// would send the operator to a setting that may already be right.
    #[must_use]
    pub const fn configured(self) -> Answer {
        match (self.granted, self.listening) {
            (Some(granted), Some(listening)) if granted == listening => Answer::Yes,
            (Some(_), Some(_)) => Answer::No,
            _ => Answer::Unknown,
        }
    }

    /// Whether the port is reachable from outside.
    ///
    /// Always unknown. Establishing it needs somebody on the other side of the
    /// tunnel to try the port, which lemonfiber has no way to arrange — and
    /// inferring it from a granted port and a matching client would be asserting
    /// the very thing that fails when a provider quietly stops forwarding.
    #[must_use]
    pub const fn reachable(self) -> Answer {
        Answer::Unknown
    }

    /// Whether the port still matches what it did before, given what it was.
    ///
    /// The check that catches a reconnect: a tunnel that drops and comes back is
    /// commonly granted a different port, and everything else goes on looking
    /// correct while the client listens on yesterday's.
    #[must_use]
    pub fn unchanged(self, previous: Option<u16>) -> Answer {
        match (previous, self.granted) {
            (Some(before), Some(now)) if before == now => Answer::Yes,
            (Some(_), Some(_)) => Answer::No,
            _ => Answer::Unknown,
        }
    }

    /// The port the client should be told to listen on, where it should be told
    /// anything.
    ///
    /// Nothing where there is no grant to push, and nothing where the client is
    /// already on it — a write that changes nothing is still a write, and one made
    /// every run is a client restarted every run.
    #[must_use]
    pub(crate) fn to_push(self) -> Option<u16> {
        let granted = self.granted?;
        (self.listening != Some(granted)).then_some(granted)
    }
}

#[cfg(test)]
mod tests;
