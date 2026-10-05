//! The connections that are not download clients.
//!
//! Indexers pushed to the services that search them, a download client's own password,
//! and the media server made the identity source for requests — each a one-off shape
//! rather than a variation on wiring a client.

use super::drift::{reconcile, Observed};
use super::{
    observe_or_skip, observe_or_untold, same_base_url, unreached, unread, wire_one, AppSync,
    Application, Journal, MediaServer, Naming, Qbittorrent, Random, Requests, State, Wiring, ADMIN,
};
use crate::baseline::Record;
use crate::ports::service::{
    Endpoint, FulfilmentTarget, RegisteredApplication, RegisteredTarget, Telling,
};
use crate::secret;
use crate::seerr::OCCASIONS;

/// The field lemonfiber records what it set the household's telling to under.
pub(crate) const TELLING: &str = "notifications.household";

/// What lemonfiber would have the request service tell the household.
#[must_use]
pub(crate) const fn wanted_telling() -> Telling {
    Telling {
        enabled: true,
        occasions: OCCASIONS,
    }
}

/// A telling written down, so the three-way comparison has one shape to read.
///
/// The occasions are a set and the baseline holds strings, so the set is written out
/// rather than the number alone — a record that said only `222` would be a number
/// nobody reading the file could place.
#[must_use]
pub(crate) fn said(telling: Telling) -> String {
    let sending = if telling.enabled { "on" } else { "off" };
    format!("{sending}:{}", telling.occasions)
}

/// Make sure the request service will tell the household what became of what they
/// asked for, and say which way it was left.
///
/// **Its own step**, rather than part of pointing the service at the media server:
/// that one stops at a service already initialised, which is every install after the
/// first — exactly the ones this would otherwise never reach.
///
/// Its own connection in the report too, named for what it does rather than for the
/// agent it does it through: an operator reading the pass wants to know whether the
/// people in the house will hear back, not which of the service's notifiers carries
/// it. Hands back what the service holds as well as the state, because a value the
/// operator set before lemonfiber ever ran is theirs to adopt and the caller needs it
/// to write the baseline down.
pub async fn wire_household_telling(
    seerr: &dyn Requests,
    recorded: Option<&Record>,
    rehearsing: bool,
) -> (Wiring, Telling) {
    let (state, held) = tell_the_household(seerr, recorded, rehearsing).await;
    (
        Wiring::settled("What the household is told".to_owned(), state),
        held,
    )
}

/// What lemonfiber sees for the telling, read from the three values.
///
/// Shared with the diagnosis that reads the same field without writing it, so the
/// two cannot come to different opinions about whose value is on the service — the
/// division `observe_client` makes for a download client, for the same reason.
///
/// A setting is always *there*, so there is no absent value the way an unregistered
/// download client is absent. The nearest thing is the service's untouched default
/// with nothing recorded against it: nobody has set this, lemonfiber included.
/// Without that, a service nobody has configured reads as the operator's own
/// pre-existing choice, and a diagnosis would tell them they had switched off
/// something they had never been offered. An operator who turned it off *after*
/// lemonfiber turned it on has a baseline, so that still reads as their edit.
#[must_use]
pub(crate) fn observed_telling(recorded: Option<&Record>, held: Telling) -> Observed {
    if recorded.is_none() && held == Telling::default() {
        Observed::Absent
    } else {
        reconcile(recorded, Some(said(held).as_str()), &said(wanted_telling()))
    }
}

/// The comparison and the write, apart from the reporting shape around them.
pub(crate) async fn tell_the_household(
    seerr: &dyn Requests,
    recorded: Option<&Record>,
    rehearsing: bool,
) -> (State, Telling) {
    let held = match seerr.telling().await {
        Ok(held) => held,
        Err(failure) => return (unread(&failure, rehearsing), Telling::default()),
    };
    let want = wanted_telling();
    let holding = said(held);
    let observed = observed_telling(recorded, held);

    let state = match observed {
        // `Unavailable` cannot arrive here — it is what a pass says about a service
        // that would not answer, and one that would not answer returned above with
        // its own words. Grouped the way the wiring check groups it, rather than
        // given an arm that nothing can reach.
        Observed::Absent | Observed::Unavailable if rehearsing => State::WouldWire {
            yours: Some(holding.clone()),
            ours: Some(said(want)),
        },
        Observed::Absent | Observed::Unavailable => match seerr.tell(&want).await {
            Ok(()) => State::Wired,
            Err(failure) => unreached(&failure),
        },
        Observed::Present => State::AlreadyWired,
        // Theirs. Said, and no more than said — somebody who turned this off turned it
        // off, and a household that stopped being told is a thing to report rather
        // than a thing to correct.
        Observed::Drifted => State::Drifted,
        Observed::Stale => State::Stale,
        Observed::Conflicted => State::Conflicted {
            yours: Some(holding),
            ours: said(want),
        },
        Observed::Adopted => State::Adopted,
        Observed::Unmanaged => State::Unmanaged,
    };
    (state, held)
}

/// Hand the request service the \*arrs that fulfil what the household asks for.
///
/// Until it is told, the request service knows of no \*arr: a request is accepted
/// and no downloader ever hears about it. It does not discover them.
///
/// Only the \*arrs actually in the stack are offered, and that is the half worth
/// stating — the request service offers what its targets can deliver, so television
/// is not offered where Sonarr is not running. An \*arr that is absent is simply
/// never handed over.
///
/// One already held is matched by where it is reached, never by its label. Held there
/// with the key it should present, it is left exactly as it is. Held there with
/// another key, or held where it was reached before (`moved_from`), it is moved in
/// place: its endpoint and key are rewritten and everything the operator chose about
/// it stays, so requests already tied to it stay tied to it.
pub async fn wire_fulfilment_targets(
    seerr: &dyn Requests,
    wanted: &[FulfilmentTarget],
    journal: &mut Journal,
    at: &str,
    rehearsing: bool,
) -> Vec<Wiring> {
    // Read as the owner, which a rehearsal is not: it opens no session, so the answer
    // comes back unauthorised and each wanted target says it could not be told rather
    // than naming a credential fault nobody has.
    let existing = match observe_or_untold(
        seerr.fulfilment_targets().await,
        wanted,
        described_target,
        rehearsing,
    ) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
    };

    let mut wirings = Vec::new();
    for target in wanted {
        let here = held_at(&existing, &target.at, target.television);
        let before = target
            .moved_from
            .as_ref()
            .and_then(|from| held_at(&existing, from, target.television));
        let state = match here.or(before) {
            Some(held) if held.at == target.at && held.key == target.key => State::AlreadyWired,
            Some(held) => tested(seerr, target, moved(seerr, held, target, rehearsing).await).await,
            None => {
                let added = wire_one(
                    seerr.add_fulfilment_target(target),
                    seerr.fulfilment_targets(),
                    |rows| held_at(rows, &target.at, target.television).map(|have| have.id.clone()),
                    Naming {
                        service: "seerr",
                        resource: "fulfilment target",
                        noun: "request target",
                    },
                    journal,
                    at,
                    rehearsing.then(|| State::WouldWire {
                        yours: None,
                        ours: Some(reached_at(&target.at)),
                    }),
                )
                .await;
                tested(seerr, target, added).await
            }
        };
        wirings.push(Wiring::settled(described_target(target), state));
    }
    wirings
}

/// `written`, where it is a target just wired, held to the request service's own test
/// of it: a target is wired only once the service has reached the \*arr with it.
async fn tested(seerr: &dyn Requests, target: &FulfilmentTarget, written: State) -> State {
    if written != State::Wired {
        return written;
    }
    match seerr
        .test_fulfilment_target(target.television, &target.at, &target.key)
        .await
    {
        Ok(()) => State::Wired,
        Err(failure) => State::Failed {
            detail: format!("Seerr's own test of the target failed: {failure}"),
        },
    }
}

/// Move a target the request service holds to where, and with what, it should be
/// reached.
async fn moved(
    seerr: &dyn Requests,
    held: &RegisteredTarget,
    target: &FulfilmentTarget,
    rehearsing: bool,
) -> State {
    if rehearsing {
        // Where only the key moves, the endpoint said twice would read as no change.
        return State::WouldWire {
            yours: (held.at != target.at).then(|| reached_at(&held.at)),
            ours: Some(reached_at(&target.at)),
        };
    }
    match seerr
        .move_fulfilment_target(held, &target.at, &target.key)
        .await
    {
        Ok(()) => State::Wired,
        Err(failure) => unreached(&failure),
    }
}

/// The one the request service holds at `endpoint` in the list `television` names,
/// if it holds one.
///
/// By where it is reached — never by name, so an operator who renamed it is not
/// handed a second copy of the same service.
fn held_at<'a>(
    held: &'a [RegisteredTarget],
    endpoint: &Endpoint,
    television: bool,
) -> Option<&'a RegisteredTarget> {
    held.iter()
        .find(|have| have.at == *endpoint && have.television == television)
}

/// Where a target is reached, as the report says it: the request gate by name, and
/// anything else by host and port.
pub(crate) fn reached_at(endpoint: &Endpoint) -> String {
    if endpoint.host == crate::app::gating::SERVICE {
        return "at the request gate".to_owned();
    }
    format!("at {}:{}", endpoint.host, endpoint.port)
}

/// A fulfilment target's description for the report.
pub(crate) fn described_target(target: &FulfilmentTarget) -> String {
    as_request_target(&target.name)
}

/// What a curator handed to the request service is called where it is reported.
pub(crate) fn as_request_target(name: &str) -> String {
    format!("{name} as a request target")
}

/// Wire Prowlarr's applications: register the media-filing \*arrs it lacks, leave
/// the ones it already has, and record each write as a change.
///
/// The same shape as [`wire_root_folders`], matched by the address Prowlarr
/// reaches an \*arr on rather than by a label, so an application an operator
/// renamed is recognised as the same connection and not registered a second time.
/// An application already present is left exactly as it is and never rewritten,
/// which is what preserves an operator's own change to its sync settings.
pub async fn wire_applications(
    prowlarr: &dyn AppSync,
    service: &str,
    wanted: &[Application],
    journal: &mut Journal,
    at: &str,
    rehearsing: bool,
) -> Vec<Wiring> {
    let existing = match observe_or_skip(prowlarr.applications().await, wanted, |application| {
        describe_application(service, application)
    }) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
    };

    let mut wirings = Vec::new();
    for application in wanted {
        let already = existing
            .iter()
            .find(|have| same_base_url(&have.base_url, &application.base_url));
        let state = if let Some(held) = already {
            current_key(prowlarr, held, application, rehearsing).await
        } else {
            wire_one(
                prowlarr.register_application(application),
                prowlarr.applications(),
                |rows| {
                    rows.iter()
                        .find(|have| same_base_url(&have.base_url, &application.base_url))
                        .map(|have| have.id.clone())
                },
                Naming {
                    service,
                    resource: "application",
                    noun: "application",
                },
                journal,
                at,
                rehearsing.then(|| State::WouldWire {
                    yours: None,
                    ours: Some(application.base_url.clone()),
                }),
            )
            .await
        };
        wirings.push(Wiring::settled(
            describe_application(service, application),
            state,
        ));
    }
    wirings
}

/// An application Prowlarr already holds, kept on the \*arr's current key.
///
/// Prowlarr shows a stored key only masked, so the only way to tell a key the \*arr
/// has since replaced is Prowlarr's own test, which runs with the key it stores. One
/// that fails is given the \*arr's current key, in place and nothing else, and tested
/// again. A rehearsal asks nothing: the test is a `POST`, which a rehearsal does not
/// send.
async fn current_key(
    prowlarr: &dyn AppSync,
    held: &RegisteredApplication,
    application: &Application,
    rehearsing: bool,
) -> State {
    if rehearsing || prowlarr.test_application(held).await.is_ok() {
        return State::AlreadyWired;
    }
    if let Err(failure) = prowlarr.rekey_application(held, &application.api_key).await {
        return unreached(&failure);
    }
    match prowlarr.test_application(held).await {
        Ok(()) => State::Wired,
        Err(failure) => unreached(&failure),
    }
}

/// An application connection's description for the report.
pub(super) fn describe_application(service: &str, application: &Application) -> String {
    format!("{} indexer sync via {service}", application.name)
}

/// Where a minted password is recorded before any service is given it, or why it could
/// not be: a service set to a password nothing recorded is one lemonfiber is locked out
/// of, so nothing here sets one this refused.
pub type Keep<'a> = &'a (dyn Fn(&str) -> Result<(), String> + Sync);

/// A connection whose minted password could not be recorded, and so was never set.
fn unkept(why: &str) -> State {
    let detail = format!(
        "the password lemonfiber generated could not be recorded, so it was not set: {why}"
    );
    State::Failed { detail }
}

/// Replace qBittorrent's temporary web UI password with a generated one, recorded
/// through `keep` before the client is given it, and hand the generated value back for
/// the connections that sign in with it next.
///
/// Unlike every other connection, this one is a credential lemonfiber mints
/// rather than reads. Generating it needs randomness the operating system might
/// withhold; without it there is nothing to set, and the connection fails rather
/// than falling back to a guessable secret on the client the forwarded port
/// authenticates to. The client sets the password and confirms it by
/// authenticating again; only a confirmed change is wired and handed back. One that
/// was recorded and then not taken is replaced by the next run, which finds the
/// recorded password refused and starts again from the temporary one.
pub async fn wire_qbittorrent_password(
    client: &Qbittorrent,
    random: &dyn Random,
    temporary: &str,
    rehearsing: bool,
    keep: Keep<'_>,
) -> (Wiring, Option<String>) {
    let connection = "qBittorrent web UI password".to_owned();
    // Above the generating, not below it. A password minted to describe a rehearsal is
    // a secret that exists because somebody asked a question, and it would then have to
    // be kept — putting it where the real one goes — or thrown away, which is worse,
    // because a thrown-away one may be the one the client has already taken.
    if rehearsing {
        return (
            Wiring::settled(
                connection,
                State::WouldWire {
                    yours: None,
                    ours: None,
                },
            ),
            None,
        );
    }
    let Some(password) = secret::generate(random) else {
        return (
            Wiring::settled(
                connection,
                State::Failed {
                    detail: "no randomness was available to generate a password".to_owned(),
                },
            ),
            None,
        );
    };

    if let Err(why) = keep(&password) {
        return (Wiring::settled(connection, unkept(&why)), None);
    }
    match client.replace_password(temporary, &password).await {
        Ok(()) => (Wiring::settled(connection, State::Wired), Some(password)),
        Err(failure) => (Wiring::settled(connection, unreached(&failure)), None),
    }
}

/// What the report calls Jellyfin's being the identity Seerr signs in against.
pub const IDENTITY: &str = "Jellyfin as Seerr's identity";

/// The first half of making Jellyfin the identity source for Seerr: Jellyfin's admin
/// credential.
///
/// Jellyfin has no key to read, so — like qBittorrent — its admin password is one
/// lemonfiber mints, records through `keep`, and only then sets by driving the
/// first-run wizard; a wizard already run by the household leaves its password
/// unknown, so the wiring is skipped rather than reset. One recorded and then not
/// taken is replaced by the next run, which finds the wizard still waiting and mints
/// again.
///
/// Apart from the second half, [`wire_seerr_identity`], because what Seerr is pointed
/// at may need this credential first: the request gate's Jellyfin key is minted with it.
///
/// # Errors
///
/// The state the connection rests in where there is no credential to go on with.
pub async fn wire_jellyfin_admin(
    jellyfin: &dyn MediaServer,
    random: &dyn Random,
    recorded: Option<&str>,
    rehearsing: bool,
    keep: Keep<'_>,
) -> Result<String, State> {
    let completed = match jellyfin.startup_completed().await {
        Ok(done) => done,
        Err(failure) => return Err(unreached(&failure)),
    };
    if completed {
        return match recorded {
            Some(password) => Ok(password.to_owned()),
            None => Err(State::Skipped {
                reason: "Jellyfin was set up outside lemonfiber, so its admin password is unknown; a later run cannot complete this until it is set up through lemonfiber".to_owned(),
            }),
        };
    }
    // A wizard that has not run is an account a real pass would create, with a password
    // it would mint. Reported without minting one, for the reason the torrent client's
    // is: a value made up to describe a rehearsal has to go somewhere afterwards.
    if rehearsing {
        return Err(State::WouldWire {
            yours: None,
            ours: None,
        });
    }
    let Some(password) = secret::generate(random) else {
        return Err(State::Failed {
            detail: "no randomness was available to generate a password".to_owned(),
        });
    };
    keep(&password).map_err(|why| unkept(&why))?;
    match jellyfin.create_admin(ADMIN, &password).await {
        Ok(()) => Ok(password),
        Err(failure) => Err(unreached(&failure)),
    }
}

/// The second half: Seerr signed in through Jellyfin at `server_url`, which on a fresh
/// Seerr also creates its owner. An already-initialised Seerr is never re-pointed,
/// since that would cost the household its existing sign-ins.
pub async fn wire_seerr_identity(
    seerr: &dyn Requests,
    password: &str,
    server_url: &str,
    rehearsing: bool,
) -> Wiring {
    let state = configure_seerr(seerr, password, server_url, rehearsing).await;
    Wiring::settled(IDENTITY.to_owned(), state)
}

/// Point Seerr at the media server, unless it is already initialised — which is
/// left untouched, whether lemonfiber initialised it on an earlier run or the
/// household set it up with accounts of its own. A fresh Seerr is signed in and
/// then read back: it must report itself initialised, or the write did not land.
async fn configure_seerr(
    seerr: &dyn Requests,
    password: &str,
    server_url: &str,
    rehearsing: bool,
) -> State {
    let initialized = match seerr.initialized().await {
        Ok(done) => done,
        Err(failure) => return unreached(&failure),
    };
    if initialized {
        return State::AlreadyWired;
    }
    // Below the read, because a service already initialised is left untouched on a real
    // run too — so a rehearsal of that is the run, and only the fresh one has anything
    // to report.
    if rehearsing {
        return State::WouldWire {
            yours: None,
            ours: Some(server_url.to_owned()),
        };
    }
    if let Err(failure) = seerr.configure_identity(ADMIN, password, server_url).await {
        return unreached(&failure);
    }
    match seerr.initialized().await {
        Ok(true) => State::Wired,
        Ok(false) => State::Failed {
            detail: "Seerr accepted the sign-in but did not report itself initialised".to_owned(),
        },
        Err(failure) => unreached(&failure),
    }
}

#[cfg(test)]
mod tests;
