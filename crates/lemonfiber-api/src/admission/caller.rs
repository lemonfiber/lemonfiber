//! Who a request proved itself to be, and what the guard learned when it knocked.

use std::time::Duration;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::Response;

use super::Keyed;
use crate::refusal::Refusal;

/// What the guard learned when somebody knocked.
///
/// Three answers rather than two, and the third is the whole point of the type.
/// **Nobody** is a secret this run does not admit, or an identity the household no
/// longer holds — both are settled facts and both refuse. **Unconfirmed** is neither
/// of those: the question was asked and could not be answered, so nobody has been
/// identified, which is a different thing from having identified nobody. A guard
/// that answered both with the same silence would tell a household their accounts
/// were gone on the day their media server was restarting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Knocking {
    /// Proved to be this caller.
    Known(Caller),
    /// Proved nothing this run admits, or proved an identity that no longer stands.
    Nobody,
    /// Could not be established, which is not the same as nobody.
    Unconfirmed,
    /// A key arrived while the wrong answers so far have earned a wait, and was not
    /// looked at. How long is left.
    Held(Duration),
    /// A key arrived from another machine over a connection its pin does not verify,
    /// and was not looked at.
    Exposed,
}

/// Who a request proved itself to be.
///
/// Who, rather than only whether: a surface that must refuse one person what it
/// offers another cannot be built on an indistinguishable *yes*. It would have to
/// decide for itself which person is looking, and a control withheld on that basis
/// is withheld by whoever drew the screen.
///
/// So the question the guard answers is the subject rather than the verdict. What
/// each caller may then do is decided where it is known — never here, and never by
/// a client reading this and drawing its own conclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// Somebody at this machine's terminal, carrying the token printed there.
    Machine,
    /// Somebody who proved the password this machine keeps.
    Operator,
    /// Somebody the media server holds an account for, by the id it files them
    /// under. What they may then do is the core's answer and is decided where it is
    /// known — never from this, and never by a client reading it.
    Member(String),
    /// A program holding a key the operator minted for it.
    Key(Keyed),
}

impl Caller {
    /// The household member this caller acts as, where it acts as one: a member's own
    /// session, or a key scoped to them.
    #[must_use]
    pub fn member(&self) -> Option<&str> {
        match self {
            Self::Member(id) => Some(id),
            Self::Key(keyed) => keyed.scope.member(),
            Self::Machine | Self::Operator => None,
        }
    }
}

/// Who is asking, taken from what the guard admitted.
///
/// One impl rather than a branch per handler: the guard puts the subject on the
/// request, and a handler that needs it says so in its signature. Absent means the
/// request reached a handler without the guard having named anybody, which past the
/// guard happens only at the one door that opens without a secret — so it is
/// answered as what it is, a request carrying nothing this run admits, rather than
/// served as though somebody had proved something.
impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or_else(|| Refusal::NotAdmitted.answered())
    }
}
