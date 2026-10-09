//! How long a member's grant to play lasts, and ending the ones that ran out.
//!
//! The media server's sessions do not lapse on their own, so the core keeps the one
//! fact that bounds them: the last day each member's client spoke to it. A grant lasts
//! [`LASTS`] days from then. One that ran out has the devices that played on it signed
//! out, and only those — a member who also signs in to the media server's own app keeps
//! that session, which this never granted, and the core keeps its own.

use std::collections::BTreeMap;

use lemonfiber_manifest::Date;

use crate::app::Ctx;
use crate::ports::service::{Household, Screening, PLAYER};

/// How many days a grant lasts after the member's client last spoke to the core.
pub const LASTS: i64 = 30;

/// The file the grants are kept in, beside the configuration.
const KEPT: &str = "grants.json";

/// The last day each member's client spoke to the core, by the id the media server
/// files them under.
type Spoken = BTreeMap<String, String>;

/// A date as it is kept and said: `YYYY-MM-DD`.
#[must_use]
pub fn written(date: Date) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

/// The last day a grant renewed on `renewed` holds.
#[must_use]
pub fn until(renewed: Date) -> Option<Date> {
    let days = EPOCH.days_until(renewed).saturating_add(LASTS);
    Date::from_unix_seconds(days.saturating_mul(SECONDS_A_DAY))
}

/// The day the Unix epoch began, which [`Date::from_unix_seconds`] counts from.
const EPOCH: Date = Date {
    year: 1970,
    month: 1,
    day: 1,
};

/// Seconds in a day.
const SECONDS_A_DAY: i64 = 86_400;

/// What is kept, or nothing kept where there is no file or it will not read.
fn read(ctx: &Ctx) -> Spoken {
    crate::app::targets::beside_env(ctx, KEPT)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write what is kept.
fn write(ctx: &Ctx, spoken: &Spoken) {
    let Some(path) = crate::app::targets::beside_env(ctx, KEPT) else {
        return;
    };
    let text = serde_json::to_string_pretty(spoken).unwrap_or_default();
    let _ = crate::config::store::write(&path, &text);
}

/// Record that `member`'s client spoke to the core today, renewing their grant.
///
/// Written only where the day moved, so a client asking every few seconds does not
/// rewrite the file each time.
pub(crate) fn renewed(ctx: &Ctx, member: &str) -> Date {
    let today = ctx.today();
    let mut spoken = read(ctx);
    let said = written(today);
    if spoken.get(member) != Some(&said) {
        spoken.insert(member.to_owned(), said);
        write(ctx, &spoken);
    }
    today
}

/// Forget `member`'s grant, where they left the household.
pub(crate) fn forgotten(ctx: &Ctx, member: &str) {
    let mut spoken = read(ctx);
    if spoken.remove(member).is_some() {
        write(ctx, &spoken);
    }
}

/// The members whose grant ran out on or before `today`.
#[must_use]
fn lapsed(spoken: &Spoken, today: Date) -> Vec<String> {
    spoken
        .iter()
        .filter(|(_, said)| {
            Date::parse(said).is_none_or(|renewed| renewed.days_until(today) > LASTS)
        })
        .map(|(member, _)| member.clone())
        .collect()
}

/// Sign out every device that played on a grant that ran out, and forget the grant.
///
/// A grant whose devices could not all be signed out is kept, so the next run tries
/// again rather than forgetting a session that is still open.
pub(crate) async fn ended<S: Household + Screening>(ctx: &Ctx, server: &S) {
    let mut spoken = read(ctx);
    let mut changed = false;
    for member in lapsed(&spoken, ctx.today()) {
        if signed_out(server, &member).await {
            spoken.remove(&member);
            changed = true;
        }
    }
    if changed {
        write(ctx, &spoken);
    }
}

/// Sign out every device `member` played on through a grant; whether all of them went.
async fn signed_out<S: Household + Screening>(server: &S, member: &str) -> bool {
    let Ok(sessions) = server.sessions(member).await else {
        return false;
    };
    let mut all = true;
    for session in sessions.iter().filter(|session| session.client == PLAYER) {
        all &= server.sign_out(&session.device_id).await.is_ok();
    }
    all
}

#[cfg(test)]
mod tests;
