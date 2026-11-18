//! The sessions this run has opened, and the answer that hands one over.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tokio::sync::Mutex;

use crate::admission::Credential;
use crate::instant::Instant;
use crate::ports::random::Random;
use crate::ports::service::Signed;

/// How many unpredictable bytes a session's secret is drawn from.
const WIDTH: usize = 32;

/// How long a session lasts.
pub const LASTS: Duration = Duration::from_secs(12 * 60 * 60);

/// A session opened, and the moment it stops being one.
///
/// The ending is carried rather than left for a client to work out, because a client
/// that guessed would be a second opinion about who is admitted — and the two would
/// disagree on the day somebody's clock is wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Admitted {
    /// The secret this session is carried by, sent in the header the per-run token is.
    pub token: String,
    /// When it stops being one.
    pub until: Instant,
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

/// Who a session was opened for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// The operator, against the credential that was kept when they signed in.
    Operator(Credential),
    /// A household member, as the media server signed them in.
    Member(Signed),
}

impl Admitted {
    /// This session, carrying the member's display name.
    #[must_use]
    pub fn called(self, name: Option<String>) -> Self {
        Self { name, ..self }
    }
}

impl Opened {
    fn member(&self) -> Option<String> {
        match self {
            Self::Operator(_) => None,
            Self::Member(signed) => Some(signed.id.clone()),
        }
    }
}

struct Session {
    until: SystemTime,
    who: Opened,
}

/// The sessions this run has opened.
#[derive(Default)]
pub struct Sessions {
    held: Mutex<HashMap<String, Session>>,
}

impl Sessions {
    /// A session opened for `who` at `now`; nothing where this machine supplies no
    /// randomness.
    pub async fn opened(
        &self,
        random: &dyn Random,
        now: SystemTime,
        who: Opened,
    ) -> Option<Admitted> {
        let bytes = random.bytes(WIDTH).filter(|bytes| bytes.len() == WIDTH)?;
        let token = crate::secret::render(&bytes);
        let until = now.checked_add(LASTS)?;
        let opened = Admitted {
            token: token.clone(),
            until: Instant::of(until),
            member: who.member(),
            name: None,
        };
        let mut held = self.held.lock().await;
        held.retain(|_, session| session.until > now);
        held.insert(token, Session { until, who });
        Some(opened)
    }

    /// Who the session `offered` was opened for, where it is one still open at `now` and
    /// still stands against the credential `against`.
    pub async fn holds(
        &self,
        offered: Option<&str>,
        now: SystemTime,
        against: Option<&Credential>,
    ) -> Option<Opened> {
        still(self.opened_for(offered, now).await?, against)
    }

    /// Who the session `offered` was opened for, where it is one still open at `now`.
    pub async fn opened_for(&self, offered: Option<&str>, now: SystemTime) -> Option<Opened> {
        let offered = offered?;
        let mut held = self.held.lock().await;
        held.retain(|_, session| session.until > now);
        Some(held.get(offered)?.who.clone())
    }
}

/// `opened`, where it still stands against the credential kept now: an operator's session
/// stands only while the password it was opened against does.
#[must_use]
pub fn still(opened: Opened, against: Option<&Credential>) -> Option<Opened> {
    match opened {
        Opened::Operator(held) if Some(&held) == against => Some(Opened::Operator(held)),
        Opened::Operator(_) => None,
        member @ Opened::Member(_) => Some(member),
    }
}
