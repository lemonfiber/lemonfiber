//! The connections that are not download clients.
//!
//! Indexers pushed to the services that search them, a download client's own password,
//! and the media server made the identity source for requests — each a one-off shape
//! rather than a variation on wiring a client.

use super::{
    observe_or_skip, observe_or_untold, same_base_url, unreached, wire_one, AppSync, Application,
    Journal, MediaServer, Naming, Qbittorrent, Random, Requests, State, Wiring, ADMIN,
};
use crate::ports::media::Kind;
use crate::ports::service::{
    Credential, Endpoint, FulfilmentTarget, IdentitySource, Protocol, RegisteredApplication,
    RegisteredTarget,
};
use crate::secret;

mod telling;

pub use telling::wire_household_telling;
pub(crate) use telling::{observed_telling, said, wanted_telling, TELLING};

/// Hand the request service the \*arrs that fulfil what the household asks for.
///
/// Until it is told, the request service knows of no \*arr: a request is accepted
/// and no downloader ever hears about it. It does not discover them.
///
/// Only the \*arrs actually in the stack are offered, and that is the half worth
/// stating — the request service offers what its targets can deliver, so television
/// is not offered where no curator files it. A curator that is absent is simply
/// never handed over.
///
/// One already held is matched by where it is reached, never by its label. Held there
/// with the key it should present, it is left exactly as it is. Held there with
/// another key, or held where it was reached before (`moved_from`), it is moved in
/// place: its endpoint and key are rewritten and everything the operator chose about
/// it stays, so requests already tied to it stay tied to it.
pub async fn wire_fulfilment_targets(
    requests: &dyn Requests,
    wanted: &[FulfilmentTarget],
    journal: &mut Journal,
    at: &str,
    rehearsing: bool,
) -> Vec<Wiring> {
    // Read as the owner, which a rehearsal is not: it opens no session, so the answer
    // comes back unauthorised and each wanted target says it could not be told rather
    // than naming a credential fault nobody has.
    let existing = match observe_or_untold(
        requests.fulfilment_targets().await,
        wanted,
        described_target,
        rehearsing,
    ) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
    };

    let mut wirings = Vec::new();
    for target in wanted {
        let here = held_at(&existing, &target.at, target.kind);
        let before = target
            .moved_from
            .as_ref()
            .and_then(|from| held_at(&existing, from, target.kind));
        let state = match here.or(before) {
            Some(held) if held.at == target.at && held.key == target.key => State::AlreadyWired,
            Some(held) => {
                tested(
                    requests,
                    target,
                    moved(requests, held, target, rehearsing).await,
                )
                .await
            }
            None => {
                let added = wire_one(
                    requests.add_fulfilment_target(target),
                    requests.fulfilment_targets(),
                    |rows| held_at(rows, &target.at, target.kind).map(|have| have.id.clone()),
                    Naming {
                        service: "requests",
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
                tested(requests, target, added).await
            }
        };
        wirings.push(Wiring::settled(described_target(target), state));
    }
    wirings
}

/// `written`, where it is a target just wired, held to the request service's own test
/// of it: a target is wired only once the service has reached the \*arr with it.
async fn tested(requests: &dyn Requests, target: &FulfilmentTarget, written: State) -> State {
    if written != State::Wired {
        return written;
    }
    match requests
        .test_fulfilment_target(target.kind, &target.at, &target.key)
        .await
    {
        Ok(()) => State::Wired,
        Err(failure) => State::Failed {
            detail: format!("the request service's own test of the target failed: {failure}"),
        },
    }
}

/// Move a target the request service holds to where, and with what, it should be
/// reached.
async fn moved(
    requests: &dyn Requests,
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
    match requests
        .move_fulfilment_target(held, &target.at, &target.key)
        .await
    {
        Ok(()) => State::Wired,
        Err(failure) => unreached(&failure),
    }
}

/// The one the request service holds at `endpoint` in the list for `kind`,
/// if it holds one.
///
/// By where it is reached — never by name, so an operator who renamed it is not
/// handed a second copy of the same service.
fn held_at<'a>(
    held: &'a [RegisteredTarget],
    endpoint: &Endpoint,
    kind: Kind,
) -> Option<&'a RegisteredTarget> {
    held.iter()
        .find(|have| have.at == *endpoint && have.kind == kind)
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

/// Wire the indexer's applications: register the media-filing curators it lacks, leave
/// the ones it already has, and record each write as a change.
///
/// The same shape as [`wire_root_folders`], matched by the address the indexer
/// reaches an \*arr on rather than by a label, so an application an operator
/// renamed is recognised as the same connection and not registered a second time.
/// An application already present is left exactly as it is and never rewritten,
/// which is what preserves an operator's own change to its sync settings.
pub async fn wire_applications(
    indexer: &dyn AppSync,
    service: &str,
    wanted: &[Application],
    journal: &mut Journal,
    at: &str,
    rehearsing: bool,
) -> Vec<Wiring> {
    let existing = match observe_or_skip(indexer.applications().await, wanted, |application| {
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
            current_key(indexer, held, application, rehearsing).await
        } else {
            wire_one(
                indexer.register_application(application),
                indexer.applications(),
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

/// An application the indexer already holds, kept on the curator's current key.
///
/// The indexer shows a stored key only masked, so the only way to tell a key the curator
/// has since replaced is the indexer's own test, which runs with the key it stores. One
/// that fails is given the \*arr's current key, in place and nothing else, and tested
/// again. A rehearsal asks nothing: the test is a `POST`, which a rehearsal does not
/// send.
async fn current_key(
    indexer: &dyn AppSync,
    held: &RegisteredApplication,
    application: &Application,
    rehearsing: bool,
) -> State {
    if rehearsing || indexer.test_application(held).await.is_ok() {
        return State::AlreadyWired;
    }
    if let Err(failure) = indexer.rekey_application(held, &application.api_key).await {
        return unreached(&failure);
    }
    match indexer.test_application(held).await {
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
                    detail: crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned(),
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

/// What the report calls the media server being the identity the request service signs
/// in against.
pub const IDENTITY: &str = "The media server as the request service's identity";

/// The first half of making the media server the identity source for the request
/// service: the media server's admin credential.
///
/// The media server has no key to read, so its admin password is one
/// lemonfiber mints, records through `keep`, and only then sets by driving the
/// first-run wizard; a wizard already run by the household leaves its password
/// unknown, so the wiring is skipped rather than reset. One recorded and then not
/// taken is replaced by the next run, which finds the wizard still waiting and mints
/// again.
///
/// Apart from the second half, [`wire_request_identity`], because what the request service
/// is pointed at may need this credential first: the request gate's media server key is
/// minted with it.
///
/// # Errors
///
/// The state the connection rests in where there is no credential to go on with.
pub async fn wire_media_server_admin(
    server: &dyn MediaServer,
    random: &dyn Random,
    recorded: Option<&str>,
    rehearsing: bool,
    keep: Keep<'_>,
) -> Result<String, State> {
    let completed = match server.startup_completed().await {
        Ok(done) => done,
        Err(failure) => return Err(unreached(&failure)),
    };
    if completed {
        return match recorded {
            Some(password) => Ok(password.to_owned()),
            None => Err(State::Skipped {
                reason: "The media server was set up outside lemonfiber, so its admin password is unknown; a later run cannot complete this until it is set up through lemonfiber".to_owned(),
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
            detail: crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned(),
        });
    };
    keep(&password).map_err(|why| unkept(&why))?;
    match server.create_admin(ADMIN, &password).await {
        Ok(()) => Ok(password),
        Err(failure) => Err(unreached(&failure)),
    }
}

/// The second half: the request service signed in through the media server at
/// `server_url`, spoken to in `protocol`, as the administrator with `password`, which on
/// a fresh request service also creates its owner. An already-initialised one is never
/// re-pointed, since that would cost the household its existing sign-ins.
pub async fn wire_request_identity(
    requests: &dyn Requests,
    protocol: Protocol,
    password: &str,
    server_url: &str,
    rehearsing: bool,
) -> Wiring {
    let source = IdentitySource {
        at: server_url.to_owned(),
        protocol,
        credential: Credential::UserPass {
            username: ADMIN.to_owned(),
            password: password.to_owned(),
        },
    };
    let state = configure_requests(requests, &source, rehearsing).await;
    Wiring::settled(IDENTITY.to_owned(), state)
}

/// Point the request service at the media server, unless it is already initialised — which is
/// left untouched, whether lemonfiber initialised it on an earlier run or the
/// household set it up with accounts of its own. A fresh request service is signed in and
/// then read back: it must report itself initialised, or the write did not land.
async fn configure_requests(
    requests: &dyn Requests,
    source: &IdentitySource,
    rehearsing: bool,
) -> State {
    let initialized = match requests.initialized().await {
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
            ours: Some(source.at.clone()),
        };
    }
    if let Err(failure) = requests.configure_identity(source).await {
        return unreached(&failure);
    }
    match requests.initialized().await {
        Ok(true) => State::Wired,
        Ok(false) => State::Failed {
            detail:
                "the request service accepted the sign-in but did not report itself initialised"
                    .to_owned(),
        },
        Err(failure) => unreached(&failure),
    }
}

#[cfg(test)]
mod tests;
