//! Where one household member already receives what the request service sends them.
//!
//! Apart from [`Approving`](super::Approving) because it is a different errand carrying a
//! different risk. That port decides what a household may ask for and what becomes of one
//! request, and every call it makes stays inside this house. This one reads a credential
//! somebody else chose and hands it to a service that is not on this machine, which is a
//! thing an operator is owed a switch for.
//!
//! **These are addresses the member already gave the request service.** Somebody who
//! wants to hear about what they asked for has said where; asking them a second time
//! would be a second answer able to disagree with the one the service acts on, and a
//! household that has to be set up twice.
//!
//! **Two of the service's own agents and no more.** Its per-person settings carry six
//! kinds of address and four of them cannot be reached with what this program holds: a
//! Discord identifier needs a bot sharing a server with the person, a Telegram chat needs
//! the bot the chat was opened with, an address for electronic mail needs a mail server,
//! and a browser subscription needs the key the browser was subscribed under. Pushover
//! and Pushbullet are the two whose whole address is what the member typed, so they are
//! the two that can be reached from here.

use async_trait::async_trait;

use super::Failure;

/// Somewhere one member already receives what the request service sends them.
///
/// Carries the credential that reaches them, so it is never printed: [`Debug`] names the
/// service and stops there. A line recording a delivery that included the token would be
/// the leak the delivery itself is careful not to be.
#[derive(Clone, PartialEq, Eq)]
pub enum Address {
    /// Pushover, by the two halves the member gave: whose devices this reaches, and the
    /// application it arrives under.
    ///
    /// Both are the member's own. The request service falls back to no application of the
    /// household's for a per-person message either, and one sent under the household's
    /// would arrive from somewhere the member never agreed to hear from.
    Pushover {
        /// The key naming whose devices this reaches.
        user: String,
        /// The application it arrives under.
        application: String,
        /// The sound they chose, where they chose one.
        sound: Option<String>,
    },
    /// Pushbullet, by the token that is both the address and the authority to write to it.
    Pushbullet {
        /// The token the member gave.
        token: String,
    },
}

impl Address {
    /// Which service this is, in the name the member would recognise it by.
    #[must_use]
    pub const fn service(&self) -> &'static str {
        match self {
            Self::Pushover { .. } => "Pushover",
            Self::Pushbullet { .. } => "Pushbullet",
        }
    }
}

impl std::fmt::Debug for Address {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.service())
    }
}

/// Reading where the person who made one request can be reached.
#[async_trait]
pub trait Addressing: Send + Sync {
    /// Where whoever asked for this already receives what the service sends them.
    ///
    /// Keyed by the request rather than by the person, because the request is the name
    /// both sides know one by — the same key a reason is filed under — and a second
    /// naming would be a second chance to tell one member's address from another's
    /// wrongly.
    ///
    /// **Only where the service itself would reach them.** A member's own switch says
    /// which of the service's events arrive on which agent, and a message sent to an
    /// agent they left switched off is one they said they did not want. So an address is
    /// answered with only where its agent carries a refusal for them, which is the same
    /// test the service applies before it sends them anything there.
    ///
    /// Nothing is not a failure. A household member with no address of these two kinds is
    /// the ordinary case, and chasing one is not this program's to do.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable, holds no such request, or
    /// refuses.
    async fn reachable(&self, request: i64) -> Result<Vec<Address>, Failure>;
}

#[cfg(test)]
mod tests {
    use super::Address;

    /// An address is named by its service and never by what reaches it.
    ///
    /// The rendering is asserted rather than left to the eye, because a derived one would
    /// put a member's own token into every line that ever formatted one.
    #[test]
    fn an_address_prints_its_service_and_never_its_token() {
        let pushover = Address::Pushover {
            user: "the-user-key".to_owned(),
            application: "the-application-token".to_owned(),
            sound: Some("pushover".to_owned()),
        };
        let pushbullet = Address::Pushbullet {
            token: "the-access-token".to_owned(),
        };

        assert_eq!(format!("{pushover:?}"), "Pushover");
        assert_eq!(format!("{pushbullet:?}"), "Pushbullet");
        assert_eq!(pushover.service(), "Pushover");
        assert_eq!(pushbullet.service(), "Pushbullet");
        assert_ne!(pushover, pushbullet);
        assert_eq!(pushbullet.clone(), pushbullet);
    }
}
