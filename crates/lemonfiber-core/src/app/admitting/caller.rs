//! Who the session or key a request carried proves it to be.

use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use serde::Serialize;

use super::household::opened;
use super::keyring::{Holding, Keyring};
use super::sessions::{still, Opened};
use super::Door;
use crate::app::Ctx;
use crate::keys::{Record, Scope};
use crate::ports::service::Signed;

/// Where a request came from, and whether it came over an encrypted connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrived {
    /// The address it came from.
    pub from: IpAddr,
    /// Whether it arrived over the encrypted connection the surface serves.
    pub encrypted: bool,
}

impl Arrived {
    /// Whether a key may be read from what arrived this way: over the encrypted
    /// connection, or from this machine itself.
    #[must_use]
    pub fn may_carry_a_key(&self) -> bool {
        self.encrypted || self.from.to_canonical().is_loopback()
    }
}

/// The secret a request carried, and how it arrived.
#[derive(Clone, PartialEq, Eq)]
pub struct Carried {
    /// The secret in the credential header, where there was one.
    pub offered: Option<String>,
    /// How the request arrived, where the surface was answered over a socket.
    pub arrived: Option<Arrived>,
}

impl std::fmt::Debug for Carried {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Carried")
            .field("offered", &self.offered.as_ref().map(|_| "withheld"))
            .field("arrived", &self.arrived)
            .finish()
    }
}

/// Who is calling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "as", rename_all = "kebab-case")]
pub enum Caller {
    /// The operator, on a session opened with the machine's password.
    Operator,
    /// A household member, on a session of their own.
    Member {
        /// The id the media server files them under.
        id: String,
    },
    /// A program, on a key the operator or a member minted for it.
    Key {
        /// The key's name.
        name: String,
        /// What it admits: `read`, `act` or `member:<name>`.
        #[serde(serialize_with = "written")]
        #[schemars(with = "String")]
        scope: Scope,
    },
}

/// `scope` as a key's listing writes it.
fn written<S: serde::Serializer>(scope: &Scope, to: S) -> Result<S::Ok, S::Error> {
    to.serialize_str(&scope.written())
}

/// What the session or key a request carried came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "standing", rename_all = "kebab-case")]
pub enum Calling {
    /// Somebody this run admits.
    Known {
        /// Who they are.
        caller: Caller,
    },
    /// Nobody this run admits.
    Nobody,
    /// A member the media server could not be asked about.
    Unconfirmed,
    /// A key held back by too many wrong answers.
    Held {
        /// How many whole seconds are left to wait.
        retry_after: u64,
    },
    /// A key that crossed a network in the clear.
    Exposed,
}

/// Who `carried` proves its request to be, at `now`.
pub(crate) async fn calling(ctx: &Ctx, carried: &Carried, now: SystemTime) -> Calling {
    let offered = carried.offered.as_deref();
    if let Some(key) = offered.filter(|offered| crate::keys::shaped(offered)) {
        return keyed(ctx, key, carried.arrived, now).await;
    }
    let admitting = &ctx.admitting;
    let opened = admitting.sessions.opened_for(offered, now).await;
    let against = match &opened {
        Some(Opened::Operator(_)) => super::credential_now(ctx).await,
        Some(Opened::Member(_)) | None => None,
    };
    match opened.and_then(|opened| still(opened, against.as_ref())) {
        Some(Opened::Operator(_)) => Calling::Known {
            caller: Caller::Operator,
        },
        Some(Opened::Member(signed)) => still_standing(ctx, signed).await,
        None => Calling::Nobody,
    }
}

/// Whether the household still holds the member a session was opened for.
async fn still_standing(ctx: &Ctx, signed: Signed) -> Calling {
    let Some(household) = opened(ctx).await else {
        return Calling::Unconfirmed;
    };
    match household.standing(&signed).await {
        Ok(true) => Calling::Known {
            caller: Caller::Member { id: signed.id },
        },
        Ok(false) => Calling::Nobody,
        Err(_) => Calling::Unconfirmed,
    }
}

/// Who the key `offered` proves its request to be.
async fn keyed(ctx: &Ctx, offered: &str, arrived: Option<Arrived>, now: SystemTime) -> Calling {
    let Some(arrived) = arrived.filter(Arrived::may_carry_a_key) else {
        return Calling::Exposed;
    };
    let admitting = &ctx.admitting;
    let ticket = match admitting.attempts.taken_at_a_key(arrived.from, now).await {
        Ok(ticket) => ticket,
        Err(left) => return held(left),
    };
    let kept = crate::keys::run::at(ctx);
    let record = match Keyring::holding(kept.as_deref(), offered) {
        Holding::Unknown => return Calling::Nobody,
        Holding::Known => {
            admitting.attempts.forgiven(&ticket).await;
            return Calling::Nobody;
        }
        Holding::Active(record) => *record,
    };
    let calling = match record.scope.member() {
        None => known(&record),
        Some(id) => match holding_member(ctx, id).await {
            Some(true) => known(&record),
            Some(false) => Calling::Nobody,
            None => Calling::Unconfirmed,
        },
    };
    if matches!(calling, Calling::Known { .. }) {
        admitting.attempts.right(&ticket, Door::Key, now).await;
        let used = crate::keys::run::used_at(ctx);
        admitting
            .keys
            .note(used.as_deref(), &record.name, now)
            .await;
    } else {
        admitting.attempts.forgiven(&ticket).await;
    }
    calling
}

fn known(record: &Record) -> Calling {
    Calling::Known {
        caller: Caller::Key {
            name: record.name.clone(),
            scope: record.scope.clone(),
        },
    }
}

/// Held back for `left`.
fn held(left: Duration) -> Calling {
    Calling::Held {
        retry_after: left.as_secs().max(1),
    }
}

/// Whether the household still holds member `id`, or nothing where it could not be asked.
async fn holding_member(ctx: &Ctx, id: &str) -> Option<bool> {
    let household = opened(ctx).await?;
    let members = household.household().await.ok()?;
    Some(members.iter().any(|member| member.id == id))
}
