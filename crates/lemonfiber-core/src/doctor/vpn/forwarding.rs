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
    pub fn to_push(self) -> Option<u16> {
        let granted = self.granted?;
        (self.listening != Some(granted)).then_some(granted)
    }
}

#[cfg(test)]
mod tests {
    use super::{Answer, Forwarding};

    /// What is known, given a grant and what the client says.
    const fn known(granted: Option<u16>, listening: Option<u16>) -> Forwarding {
        Forwarding { granted, listening }
    }

    #[test]
    fn the_four_questions_are_answered_separately() {
        // An operator told only that port forwarding is not working learns nothing
        // about which of these to look at.
        let matched = known(Some(51413), Some(51413));
        assert_eq!(matched.assigned(), Answer::Yes);
        assert_eq!(matched.configured(), Answer::Yes);
        assert_eq!(matched.reachable(), Answer::Unknown);
        assert_eq!(matched.unchanged(Some(51413)), Answer::Yes);
    }

    #[test]
    fn a_client_listening_elsewhere_is_the_one_that_is_wrong() {
        // Everything else is true and nobody can reach it — the failure that looks
        // healthy from inside.
        let mismatched = known(Some(51413), Some(6881));
        assert_eq!(mismatched.assigned(), Answer::Yes);
        assert_eq!(mismatched.configured(), Answer::No);
    }

    #[test]
    fn a_client_that_would_not_answer_settles_nothing_about_its_port() {
        // Reporting silence as a mismatch would send the operator to a setting
        // that may already be right.
        assert_eq!(known(Some(51413), None).configured(), Answer::Unknown);
        assert_eq!(known(None, Some(51413)).configured(), Answer::Unknown);
    }

    #[test]
    fn no_grant_is_a_settled_no_rather_than_an_unknown() {
        // The provider answered; it granted nothing. That is a fact, and a
        // different one from not having been able to ask.
        assert_eq!(known(None, Some(6881)).assigned(), Answer::No);
    }

    #[test]
    fn reachability_is_never_claimed_from_the_inside() {
        // It needs somebody on the other side of the tunnel to try the port.
        // Inferring it from a grant and a matching client would assert the very
        // thing that fails when a provider quietly stops forwarding.
        for known in [
            known(Some(51413), Some(51413)),
            known(Some(51413), Some(6881)),
            known(None, None),
        ] {
            assert_eq!(known.reachable(), Answer::Unknown);
        }
    }

    #[test]
    fn a_reconnect_on_a_new_port_is_caught_by_comparing_with_before() {
        // A tunnel that drops and returns is commonly granted a different port,
        // and everything else goes on looking correct while the client listens on
        // yesterday's.
        assert_eq!(known(Some(51999), None).unchanged(Some(51413)), Answer::No);
        assert_eq!(known(Some(51413), None).unchanged(Some(51413)), Answer::Yes);
        // Nothing to compare with is not a change.
        assert_eq!(known(Some(51413), None).unchanged(None), Answer::Unknown);
        assert_eq!(known(None, None).unchanged(Some(51413)), Answer::Unknown);
    }

    #[test]
    fn a_client_already_on_the_granted_port_is_left_alone() {
        // A write that changes nothing is still a write, and one made every run is
        // a client restarted every run.
        assert_eq!(known(Some(51413), Some(51413)).to_push(), None);
        assert_eq!(known(Some(51413), Some(6881)).to_push(), Some(51413));
        assert_eq!(known(Some(51413), None).to_push(), Some(51413));
        assert_eq!(known(None, Some(6881)).to_push(), None, "nothing to push");
    }
}
