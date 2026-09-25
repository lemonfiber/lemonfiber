//! Opening the download clients, and putting the limits to them.
//!
//! Every client is asked the same three things — what it is limited to, what it is
//! moving, and (where the run is applying) what it says after being told — and each
//! answers on its own shape. A client that will not answer is a line of its own in
//! the report rather than an absence, because an unknown limit rendered as no limit
//! is a report reading better than the stack is.
//!
//! Reading is done for every client whether or not anything is being written, so
//! the unconfirmed run and the applying one produce the same shape of answer and
//! there is no second rendering to fall out of step with the first.

use crate::app::targets::{DownloadKind, DownloadTarget};
use crate::app::Ctx;
use crate::bandwidth::{Answer, Held, Holding, Period, Pulling};
use crate::ports::service::{
    Failure, Fetching, Hours, Metering, Moved, Rates, Throttled, Throttling, Wanted,
};
use crate::qbittorrent::Qbittorrent;
use crate::sabnzbd::Sabnzbd;

/// What a run is to do about one client's fetching.
///
/// Three requests rather than a flag, because "leave it alone" is a third answer
/// and not the absence of the other two: a run with nothing to change still has to
/// say whether the client is fetching, and a flag would make that question
/// impossible to ask without answering it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fetch {
    /// Only ask, and report what it says.
    Ask,
    /// Stop it, because a spent cap says so.
    Stop,
    /// Let it fetch again, because lemonfiber stopped it and no longer has cause.
    Resume,
}

/// A download client this command can reach.
///
/// Boxed on both arms because the two clients are very different sizes and an
/// enum as large as its largest arm would be carried around at that size for
/// every one of them.
pub(super) enum Client {
    /// The torrent client, which uploads and keeps a schedule of its own.
    Torrent(Box<Qbittorrent>),
    /// The Usenet client, which does neither.
    Usenet(Box<Sabnzbd>),
}

impl Client {
    /// The name the stack knows it under, which is what the report names it by.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Torrent(_) => "qbittorrent",
            Self::Usenet(_) => "sabnzbd",
        }
    }

    /// The limits on it.
    fn throttling(&self) -> &dyn Throttling {
        match self {
            Self::Torrent(client) => client.as_ref(),
            Self::Usenet(client) => client.as_ref(),
        }
    }

    /// What it has moved.
    fn metering(&self) -> &dyn Metering {
        match self {
            Self::Torrent(client) => client.as_ref(),
            Self::Usenet(client) => client.as_ref(),
        }
    }

    /// Whether it is fetching at all.
    fn fetching(&self) -> &dyn Fetching {
        match self {
            Self::Torrent(client) => client.as_ref(),
            Self::Usenet(client) => client.as_ref(),
        }
    }

    /// What this client has moved in `month`, or nothing where it would not say.
    pub(super) async fn moved(&self, month: &str) -> Option<Moved> {
        self.metering().moved(month).await.ok()
    }
}

/// Every download client on this stack that can be opened, in the order the
/// manifest declares them.
///
/// A client lemonfiber cannot authenticate to is left out rather than reported as
/// unlimited: it is not a client with no limits, it is a client nothing here can
/// see, and the two must not render alike.
pub(super) async fn opened(ctx: &Ctx, targets: &[DownloadTarget]) -> Vec<Client> {
    let mut clients = Vec::new();
    for target in targets {
        match &target.kind {
            DownloadKind::Qbittorrent => {
                if let Some(password) = crate::app::targets::recorded_qbittorrent_password(ctx) {
                    clients.push(Client::Torrent(Box::new(Qbittorrent::authenticated(
                        ctx.seams.http.clone(),
                        &target.base,
                        password,
                    ))));
                }
            }
            DownloadKind::Sabnzbd { config } => {
                if let Some(key) = ctx
                    .seams
                    .filesystem
                    .read(config)
                    .await
                    .as_deref()
                    .and_then(crate::sabnzbd::api_key)
                {
                    clients.push(Client::Usenet(Box::new(Sabnzbd::new(
                        ctx.seams.http.clone(),
                        &target.base,
                        key,
                    ))));
                }
            }
        }
    }
    clients
}

/// Put the limits to one client where `writing`, and read back what it says.
///
/// The read-back is the same call in both cases, so the answer an unconfirmed run
/// reports and the answer an applying run reports come from one place — which is
/// what stops a rehearsal describing something the real thing would not do.
pub(super) async fn holding(
    client: &Client,
    wanted: &Wanted,
    fetch: Option<Fetch>,
    writing: bool,
) -> Holding {
    let answered = if writing {
        client.throttling().restrain(wanted).await
    } else {
        client.throttling().throttled().await
    };
    let pulling = match fetch {
        Some(fetch) => pulling(client, fetch).await,
        None => None,
    };
    Holding {
        client: client.name().to_owned(),
        answer: match answered {
            Ok(held) => {
                let moving = client.throttling().moving().await.unwrap_or_default();
                answer(wanted, &held, &moving)
            }
            Err(failure) => Answer::Silent {
                said: said(&failure),
            },
        },
        pulling,
    }
}

/// What one client is doing about fetching, once it has been told.
///
/// A client that would not answer reports nothing rather than a guess. "Stopped"
/// is exactly the wrong thing to say about a client nobody could reach, because it
/// is the answer that reads as a cap being kept — and the limits beside it already
/// say the client was silent.
async fn pulling(client: &Client, fetch: Fetch) -> Option<Pulling> {
    let asking = client.fetching();
    let answered = match fetch {
        Fetch::Ask => asking.pulling().await,
        Fetch::Stop => asking.stop().await,
        Fetch::Resume => asking.resume().await,
    };
    answered.ok().map(|pulling| match pulling {
        crate::ports::service::Pulling::Fetching => Pulling::Fetching,
        crate::ports::service::Pulling::Stopped => Pulling::Stopped,
    })
}

/// What one client's answer amounts to in both directions.
fn answer(wanted: &Wanted, held: &Throttled, moving: &Rates) -> Answer {
    // What was asked of it is whichever side of the day it says it is on, because
    // that is the limit it is under right now — comparing a client in its quiet
    // hours against the active figure would report every well-behaved stack as
    // having ignored its limits every night.
    let asked = match held.hours {
        Some(Hours::Quiet) => wanted.quiet,
        Some(Hours::Active) | None => wanted.active,
    };
    Answer::Held {
        down: Held::of(asked.down, held.rates.down, moving.down, true),
        up: Held::of(asked.up, held.rates.up, moving.up, held.uploads),
        period: held.hours.map(|hours| match hours {
            Hours::Active => Period::Active,
            Hours::Quiet => Period::Quiet,
        }),
    }
}

/// What a client that would not answer said, in its own words.
///
/// The service's own detail where it gave one, so the operator reads what refused
/// rather than an interpretation of it; the failure's own sentence otherwise.
fn said(failure: &Failure) -> String {
    match failure {
        Failure::Refused { detail, .. } | Failure::Unsupported { detail, .. } => detail.clone(),
        Failure::Unavailable { .. } | Failure::Unauthorised { .. } => failure.to_string(),
    }
}

#[cfg(test)]
mod tests;
