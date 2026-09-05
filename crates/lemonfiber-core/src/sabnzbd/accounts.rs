//! What the Usenet accounts behind `SABnzbd` have left.
//!
//! A provider publishes no quota, so the block that was bought is recorded in the
//! client's own server configuration and the bytes pulled are counted in its stats.
//! Reading both and reconciling them into one [`UsenetAccount`] is this half's job —
//! including the client's habit of resetting counters on the calendar, which is why
//! a recorded quota is read against the usage standing when it was written.

use async_trait::async_trait;
use serde::Deserialize;

use lemonfiber_manifest::Date;

use crate::ports::service::{Failure, Recorded, Standing, UsenetAccount, UsenetAccounts};

use super::Sabnzbd;

/// `SABnzbd`'s `mode=get_config&section=servers` answer: the accounts as configured.
#[derive(Deserialize)]
pub(crate) struct ConfigResponse {
    config: Servers,
}

#[derive(Deserialize)]
pub(crate) struct Servers {
    #[serde(default)]
    servers: Vec<ServerConfig>,
}

/// One configured account. The username and the starred-out password come back in
/// this answer too and are deliberately not read: nothing here needs them, and a
/// field that is never taken cannot be logged, reported, or put in a bundle.
#[derive(Deserialize)]
pub(crate) struct ServerConfig {
    /// The account's own key, which is also how the statistics are keyed.
    name: String,
    /// What the operator named it, where they named it anything.
    #[serde(default)]
    displayname: String,
    /// `SABnzbd` writes its booleans as 0 or 1. A missing flag reads as in use:
    /// over-reporting an account is a smaller error than silently dropping one.
    #[serde(default = "in_use")]
    enable: i64,
    /// The allowance, in the client's own size notation.
    #[serde(default)]
    quota: String,
    /// What had been pulled when that allowance was recorded.
    #[serde(default)]
    usage_at_start: i64,
    /// The subscription's last day, as an ISO date.
    #[serde(default)]
    expire_date: String,
}

/// The default for a missing enabled flag — see [`ServerConfig::enable`].
const fn in_use() -> i64 {
    1
}

/// `SABnzbd`'s `mode=server_stats` answer: what each account has pulled, keyed by
/// the same name the configuration uses.
#[derive(Deserialize)]
pub(crate) struct StatsResponse {
    #[serde(default)]
    servers: std::collections::BTreeMap<String, ServerStats>,
}

#[derive(Deserialize)]
pub(crate) struct ServerStats {
    /// Everything ever pulled from the account.
    #[serde(default)]
    total: u64,
    /// Bytes per day, keyed by `YYYY-MM-DD`. The client's own week and month totals
    /// are deliberately ignored: they reset on the calendar rather than rolling, so a
    /// rate taken from them depends on what day it happens to be.
    #[serde(default)]
    daily: std::collections::BTreeMap<String, u64>,
}

/// `SABnzbd`'s `mode=fullstatus` answer: how each account is doing at this moment,
/// rather than what it has pulled over its life.
///
/// The dashboard half of that answer is skipped. It resolves the machine's public
/// address and probes DNS on the client's behalf — work nothing here asks about, and
/// which would make a read of an account's standing reach the network to answer.
#[derive(Deserialize)]
pub(crate) struct StatusResponse {
    status: Statuses,
}

#[derive(Deserialize)]
pub(crate) struct Statuses {
    #[serde(default)]
    servers: Vec<ServerStatus>,
}

/// One account as the client is finding it right now.
#[derive(Deserialize)]
pub(crate) struct ServerStatus {
    /// The name the client shows for it, which is the display name it was configured
    /// with — the client fills that in from the account's own key where the operator
    /// left it blank, so it is the name the configuration answer carries too.
    #[serde(default)]
    servername: String,
    /// Whether the client still has it in rotation. A missing flag reads as in use, for
    /// the same reason a missing enabled flag does: inventing a dropped account is a
    /// worse error than missing one, and this one reports a provider as not answering.
    #[serde(default = "in_rotation")]
    serveractive: bool,
    /// Connections it currently holds ready to it.
    #[serde(default)]
    serveractiveconn: i64,
    /// Connections it is configured to open to it.
    #[serde(default)]
    servertotalconn: i64,
    /// The last trouble it recorded against the account, in the words it recorded.
    #[serde(default)]
    servererror: String,
}

/// The default for a missing rotation flag — see [`ServerStatus::serveractive`].
const fn in_rotation() -> bool {
    true
}

#[async_trait]
impl UsenetAccounts for Sabnzbd {
    async fn accounts(&self) -> Result<Vec<UsenetAccount>, Failure> {
        let configured: ConfigResponse = self
            .read(
                "get_config&section=servers",
                "the Usenet accounts could not be read",
            )
            .await?;
        let measured: StatsResponse = self
            .read("server_stats", "the account statistics could not be read")
            .await?;
        let live: StatusResponse = self
            .read(
                "fullstatus&skip_dashboard=1",
                "how the accounts are being served could not be read",
            )
            .await?;
        Ok(configured
            .config
            .servers
            .into_iter()
            .map(|server| {
                let stats = measured.servers.get(&server.name);
                account_of(server, stats, &live.status.servers)
            })
            .collect())
    }
}

/// One configured account joined to what it has pulled and how it is being served.
///
/// An account the statistics have never mentioned has downloaded nothing yet, which
/// is a zero rather than a gap: the client would not measure an account it has not
/// used, and reporting that as unreadable would raise a fault about an idle provider.
///
/// One the live view does not mention has no standing at all, which is its own answer:
/// the client only builds a connection to an account it is set to use, so an account
/// missing from it is one nothing has been asked of.
pub(crate) fn account_of(
    server: ServerConfig,
    stats: Option<&ServerStats>,
    live: &[ServerStatus],
) -> UsenetAccount {
    let downloaded = stats.map_or(0, |stats| stats.total);
    let name = if server.displayname.trim().is_empty() {
        server.name
    } else {
        server.displayname
    };
    let standing = live
        .iter()
        .find(|status| status.servername == name)
        .map(standing_of);
    UsenetAccount {
        name,
        enabled: server.enable != 0,
        quota: recorded_quota(&server.quota, server.usage_at_start),
        downloaded,
        daily: stats.map(daily_totals).unwrap_or_default(),
        expires_on: Date::parse(server.expire_date.trim()),
        standing,
    }
}

/// How an account is being served, as the client has it this moment.
///
/// Counts that will not fit are read as none held rather than as an error: a negative
/// connection count is not a fault of the account, and the figures are evidence of it
/// working rather than of it failing, so the cautious reading is the smaller one.
pub(crate) fn standing_of(status: &ServerStatus) -> Standing {
    let words = status.servererror.trim();
    Standing {
        ready: u64::try_from(status.serveractiveconn).unwrap_or(0),
        configured: u64::try_from(status.servertotalconn).unwrap_or(0),
        serving: status.serveractive,
        trouble: (!words.is_empty()).then(|| words.to_owned()),
    }
}

/// The allowance recorded against an account, where one is recorded at all.
///
/// An unset, unreadable or unlimited quota is `None` — nothing to judge capacity
/// against, which the report says plainly rather than filling in.
pub(crate) fn recorded_quota(quota: &str, usage_at_start: i64) -> Option<Recorded> {
    let cap = size_of(quota).filter(|cap| *cap > 0)?;
    Some(Recorded {
        cap,
        from: u64::try_from(usage_at_start).unwrap_or(0),
    })
}

/// `SABnzbd`'s size notation — a number with an optional `K`/`M`/`G`/`T`/`P` — as
/// bytes.
///
/// Each step is 1024, not 1000, which is what the client itself does: reading `100G`
/// as a hundred billion bytes would put every figure lemonfiber reports seven percent
/// away from the one the operator sees in their own client, and a figure that
/// disagrees with the client is worse than no figure. The fraction is folded in by
/// integer arithmetic rather than a float, so a size the client accepts is not
/// rounded on the way through.
///
/// Both of those are true of every size this product reads, so the reading itself is
/// [`crate::bytes::read`] and this is the name it goes under here.
pub(crate) fn size_of(text: &str) -> Option<u64> {
    crate::bytes::read(text)
}

/// Every account's per-day figures together, which is what the stack pulled.
///
/// Across accounts rather than per account, because a cap belongs to the line and
/// not to any one provider on it: two blocks bought from two providers are two
/// allowances and one connection.
pub(crate) fn daily_across(stats: &StatsResponse) -> Vec<(Date, u64)> {
    stats.servers.values().flat_map(daily_totals).collect()
}

/// The per-day figures, as dates. A key that is not a date is dropped rather than
/// guessed at: a day nobody can place cannot be part of a window.
pub(crate) fn daily_totals(stats: &ServerStats) -> Vec<(Date, u64)> {
    stats
        .daily
        .iter()
        .filter_map(|(day, bytes)| Date::parse(day).map(|day| (day, *bytes)))
        .collect()
}
