//! Where the person who made one request already receives what the service sends them.
//!
//! Two reads, because the service keeps the two halves apart and hides one of them from
//! the other. `GET /request/{id}` carries `requestedBy` and every column of that account
//! except its settings — those are stripped from a serialised member on purpose — so
//! where somebody is reached comes from `GET /user/{id}/settings/notifications`, which is
//! guarded by `isOwnProfileOrAdmin` and answers an administrator for anybody. Both read
//! off `ghcr.io/seerr-team/seerr:v3.3.0` rather than recalled.
//!
//! **Read and never written.** The write beside it builds a fresh settings row against
//! `req.user` — the account asking rather than the account named in the path — so the
//! first write for anybody would hang their notification settings off this program's own
//! account. Nothing here writes, and that defect is the reason it is worth saying so.
//!
//! **The member's own switch decides, and the arithmetic is the service's.** Its per-agent
//! mask defaults to nought for every push agent, `hasNotificationType` adds the test
//! notification's bit to a mask that lacks it before comparing, and the bit compared
//! against for a refusal is a different one — so what its arithmetic comes to for this
//! one event is whether the member switched refusals on at all. Read the other way round,
//! a member who has never chosen is reached by nothing here, which is exactly what the
//! request service does with them.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Deserialize;

use super::members::MEMBERS;
use super::Seerr;
use crate::ports::http::Method;
use crate::ports::service::{Address, Addressing, Failure};

/// Where one request's own record is read.
const REQUESTS: &str = "/request";

/// Where one member's notification settings are read, under their own id.
const NOTIFICATIONS: &str = "settings/notifications";

/// The bit the service files a refusal under.
const DECLINED: u64 = 64;

/// What the member's own switches key Pushover under.
const PUSHOVER: &str = "pushover";

/// What they key Pushbullet under.
const PUSHBULLET: &str = "pushbullet";

/// One request, in the single field this needs of it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Asked {
    /// The account that asked, by the id the service files it under.
    #[serde(default)]
    requested_by: Requester,
}

/// Who asked, by the service's own number for them.
#[derive(Default, Deserialize)]
struct Requester {
    /// The number, which is what the settings read below is keyed by.
    #[serde(default)]
    id: i64,
}

/// One member's notification settings, in the fields that are a whole address.
///
/// The four that are not are left unread rather than carried and discarded: a
/// Discord identifier, a Telegram chat, a key for encrypted mail and the browser
/// subscriptions are each half an address here, and a field read is a field that
/// eventually gets used.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reachable {
    /// Whose devices a Pushover message reaches.
    #[serde(default)]
    pushover_user_key: Option<String>,
    /// The application one arrives under.
    #[serde(default)]
    pushover_application_token: Option<String>,
    /// The sound they chose, where they chose one.
    #[serde(default)]
    pushover_sound: Option<String>,
    /// The token that is both a Pushbullet address and the authority to write to it.
    #[serde(default)]
    pushbullet_access_token: Option<String>,
    /// Which of the service's own events reach them, per agent.
    ///
    /// Read loosely because it is somebody else's document: a value that is not a
    /// number is a switch this cannot read, and reading it as nought leaves that
    /// member unreached rather than reaching them against their wishes.
    #[serde(default)]
    notification_types: BTreeMap<String, serde_json::Value>,
}

impl Reachable {
    /// Every way this member can be reached from here.
    fn addresses(&self) -> Vec<Address> {
        [self.pushover(), self.pushbullet()]
            .into_iter()
            .flatten()
            .collect()
    }

    /// Pushover, where they gave both halves and left refusals switched on there.
    fn pushover(&self) -> Option<Address> {
        if !would_send(self.switch(PUSHOVER)) {
            return None;
        }
        Some(Address::Pushover {
            user: given(self.pushover_user_key.as_deref())?,
            application: given(self.pushover_application_token.as_deref())?,
            sound: given(self.pushover_sound.as_deref()),
        })
    }

    /// Pushbullet, on the same two conditions.
    fn pushbullet(&self) -> Option<Address> {
        if !would_send(self.switch(PUSHBULLET)) {
            return None;
        }
        Some(Address::Pushbullet {
            token: given(self.pushbullet_access_token.as_deref())?,
        })
    }

    /// What this member has switched on for one agent, or nothing at all.
    fn switch(&self, agent: &str) -> u64 {
        self.notification_types
            .get(agent)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default()
    }
}

/// Whether the request service would itself send this member a refusal on that agent.
///
/// `hasNotificationType` adds the test notification's bit to a switch that does not
/// carry it before comparing, and that changes nothing here because a refusal is a
/// different bit — so what its arithmetic comes to for this one event is whether the
/// member switched refusals on at all.
const fn would_send(switch: u64) -> bool {
    switch & DECLINED != 0
}

/// A field the member actually filled in.
///
/// The service stores an untouched field as null and a cleared one as an empty string,
/// and neither is somewhere a message could go.
fn given(field: Option<&str>) -> Option<String> {
    let written = field?.trim();
    (!written.is_empty()).then(|| written.to_owned())
}

#[async_trait]
impl Addressing for Seerr {
    async fn reachable(&self, request: i64) -> Result<Vec<Address>, Failure> {
        reachable(self, request).await
    }
}

async fn reachable(seerr: &Seerr, request: i64) -> Result<Vec<Address>, Failure> {
    let path = format!("{REQUESTS}/{request}");
    let filed = seerr
        .endpoint
        .send(&seerr.request(Method::Get, &path, None))
        .await?;
    let asked: Asked = seerr
        .endpoint
        .decode(&filed, "who asked for this request could not be read")?;
    let path = format!("{MEMBERS}/{}/{NOTIFICATIONS}", asked.requested_by.id);
    let settings = seerr
        .endpoint
        .send(&seerr.request(Method::Get, &path, None))
        .await?;
    let held: Reachable = seerr
        .endpoint
        .decode(&settings, "where this member is reached could not be read")?;
    Ok(held.addresses())
}

#[cfg(test)]
mod tests;
