//! What proving who you are answers with.
//!
//! The password is exchanged **once**, for this. Verifying one is deliberately
//! expensive — that is the whole of what makes it worth storing the way it is stored —
//! and a credential re-sent on every request is a credential with more chances to leak.
//! So what comes back is a secret with an ending on it, and the password is not asked
//! for again until that ending arrives.

use std::time::SystemTime;

use serde::Serialize;

/// A session opened, and the moment it stops being one.
///
/// The ending is carried rather than left for a client to work out, because a client
/// that guessed would be a second opinion about who is admitted — and the two would
/// disagree on the day somebody's clock is wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Admitted {
    /// The secret this session is carried by, sent in the header the per-run token is.
    pub token: String,
    /// When it stops being one, written as every other instant this product writes.
    pub until: String,
    /// The household member this session is for, where it is a member's.
    ///
    /// **Absent is the operator**, which is the whole of the discriminator. A second
    /// field naming which kind of person this is could disagree with this one, and
    /// the day they disagreed a client would have to choose which to believe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
    /// The member's display name as the media server holds it, where it is a member's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Admitted {
    /// A session opened, with its ending written the way a service writes one.
    ///
    /// Written here rather than by whoever answers with it, so the one function that
    /// turns a moment into the text this product uses stays where every other
    /// instant goes through it. Nothing where the moment is too far past the
    /// calendar to place — a session with no ending written on it would be one a
    /// client could not know the end of.
    #[must_use]
    pub fn opened(token: String, until: SystemTime, member: Option<String>) -> Option<Self> {
        Some(Self {
            token,
            until: lemonfiber_core::instant::written(until)?,
            member,
            name: None,
        })
    }

    /// This session, carrying the member's display name.
    #[must_use]
    pub fn called(self, name: Option<String>) -> Self {
        Self { name, ..self }
    }
}
