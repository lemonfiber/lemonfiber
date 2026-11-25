//! Who a request proved itself to be, as the guard and every route past it read it.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::Response;
use lemonfiber_core::app::admitting::{self, Calling};
use lemonfiber_core::app::{dispatch, Command, Ctx, Surfacing};
use lemonfiber_core::keys::Scope;

use crate::refusal::Refusal;

/// What the guard learned when somebody knocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Knocking {
    /// Proved to be this caller.
    Known(Caller),
    /// Proved nothing this run admits, or an identity that no longer stands.
    Nobody,
    /// Could not be established, which is not the same as nobody.
    Unconfirmed,
    /// A key arrived while wrong answers have earned a wait; how many seconds are left.
    Held(u64),
    /// A key arrived from another machine over a connection its pin does not verify.
    Exposed,
}

impl From<Calling> for Knocking {
    fn from(calling: Calling) -> Self {
        match calling {
            Calling::Known { caller } => Self::Known(caller.into()),
            Calling::Nobody => Self::Nobody,
            Calling::Unconfirmed => Self::Unconfirmed,
            Calling::Held { retry_after } => Self::Held(retry_after),
            Calling::Exposed => Self::Exposed,
        }
    }
}

/// A key a request carried, as far as what it may do turns on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyed {
    /// What the operator called it.
    pub name: String,
    /// What it admits.
    pub scope: Scope,
}

/// Who a request proved itself to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// Whoever holds the per-run token this machine's terminal was given.
    Machine,
    /// The operator, on a session opened with the machine's password.
    Operator,
    /// A household member, by the id the media server files them under.
    Member(String),
    /// A program, on a key.
    Key(Keyed),
}

impl From<admitting::Caller> for Caller {
    fn from(caller: admitting::Caller) -> Self {
        match caller {
            admitting::Caller::Operator => Self::Operator,
            admitting::Caller::Member { id } => Self::Member(id),
            admitting::Caller::Key { name, scope } => Self::Key(Keyed { name, scope }),
        }
    }
}

impl Caller {
    /// The household member this caller acts as, where it acts as one.
    #[must_use]
    pub fn member(&self) -> Option<&str> {
        match self {
            Self::Member(id) => Some(id),
            Self::Key(keyed) => keyed.scope.member(),
            Self::Machine | Self::Operator => None,
        }
    }

    /// Renew the grant to play of the member this caller acts as, now their client has
    /// spoken, so a phone in use keeps playing and one left unused stops.
    pub(crate) async fn spoke(&self, ctx: &Ctx) {
        if let Some(member) = self.member() {
            let renewed = Command::Surface(Surfacing::Spoke {
                member: member.to_owned(),
            });
            drop(dispatch(renewed, ctx).await);
        }
    }
}

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
