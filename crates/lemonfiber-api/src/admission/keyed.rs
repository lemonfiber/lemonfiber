//! Who a key a request carried proves it to be.

use std::time::SystemTime;

use lemonfiber_core::keys::Scope;

use super::{Admitting, Caller, Door, Holding, Knocking};
use crate::guard::Arrived;

/// A key a request carried, as far as what it may do turns on it.
///
/// The name and the scope and nothing else: the secret has done its work by the time
/// this exists, and nothing past the guard needs to hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyed {
    /// What the operator called it.
    pub name: String,
    /// What it admits.
    pub scope: Scope,
}

impl From<&lemonfiber_core::keys::Record> for Keyed {
    fn from(record: &lemonfiber_core::keys::Record) -> Self {
        Self {
            name: record.name.clone(),
            scope: record.scope.clone(),
        }
    }
}

impl Admitting {
    /// Who a value shaped like a key proves the request to be.
    ///
    /// **Refused before it is looked at** where it crossed a network in the clear, or
    /// where nothing said which connection it came over: a key is accepted from another
    /// machine only over the TLS its pin verifies, and an arrival nobody vouched for is
    /// not one anybody verified.
    ///
    /// **Counted before it is looked at**, against the same two limits a password
    /// meets, so that every key arriving at once meets them together rather than each
    /// passing before any is counted. What turns out to be a key this machine minted,
    /// right or revoked, is given back: it is not a guess, and counting a forgotten
    /// integration's revoked key would hold whoever shares its address at the wait for
    /// good. Only a value that is no key at all stays counted, and a right key forgives
    /// nothing but itself.
    pub(super) async fn keyed(
        &self,
        offered: &str,
        arrived: Option<Arrived>,
        now: SystemTime,
    ) -> Knocking {
        let Some(arrived) = arrived.filter(Arrived::may_carry_a_key) else {
            return Knocking::Exposed;
        };
        let ticket = match self.attempts.taken_at_a_key(arrived.from, now).await {
            Ok(ticket) => ticket,
            Err(left) => return Knocking::Held(left),
        };
        let record = match self.keys.holding(offered) {
            Holding::Unknown => return Knocking::Nobody,
            Holding::Known => {
                self.attempts.forgiven(&ticket).await;
                return Knocking::Nobody;
            }
            Holding::Active(record) => record,
        };
        let knocking = match record.scope.member() {
            None => Knocking::Known(Caller::Key(Keyed::from(&record))),
            // A member's key admits what their account does, and only while the
            // household still holds them: asked on every call, as their session is.
            Some(id) => match self.holding_member(id).await {
                Some(true) => Knocking::Known(Caller::Key(Keyed::from(&record))),
                Some(false) => Knocking::Nobody,
                None => Knocking::Unconfirmed,
            },
        };
        if matches!(knocking, Knocking::Known(_)) {
            self.attempts.right(&ticket, Door::Key, now).await;
            self.keys.note(&record.name, now).await;
        } else {
            self.attempts.forgiven(&ticket).await;
        }
        knocking
    }

    /// Whether the household still holds the member filed under `id`, or nothing where
    /// it could not be asked.
    ///
    /// Asked of the whole household rather than of a sign-in, because a key was never a
    /// sign-in: there is no access the media server granted it to ask about, only an
    /// account that is there or is not.
    async fn holding_member(&self, id: &str) -> Option<bool> {
        let household = self.household_now().await?;
        let members = household.household().await.ok()?;
        Some(members.iter().any(|member| member.id == id))
    }
}
