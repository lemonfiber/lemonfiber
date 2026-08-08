//! The connections that are not download clients.
//!
//! Indexers pushed to the services that search them, a download client's own password,
//! and the media server made the identity source for requests — each a one-off shape
//! rather than a variation on wiring a client.

use super::{
    observe_or_skip, same_base_url, unreached, wire_one, AppSync, Application, Journal,
    MediaServer, Naming, Qbittorrent, Random, Requests, State, Wiring, ADMIN,
};
use crate::secret;

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
            .any(|have| same_base_url(&have.base_url, &application.base_url));
        let state = if already {
            State::AlreadyWired
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

/// An application connection's description for the report.
pub(super) fn describe_application(service: &str, application: &Application) -> String {
    format!("{} indexer sync via {service}", application.name)
}

/// Replace qBittorrent's temporary web UI password with a generated one, and hand
/// the generated value back so the surface can record it where the forwarded-port
/// push reads it.
///
/// Unlike every other connection, this one is a credential lemonfiber mints
/// rather than reads. Generating it needs randomness the operating system might
/// withhold; without it there is nothing to set, and the connection fails rather
/// than falling back to a guessable secret on the client the forwarded port
/// authenticates to. The client sets the password and confirms it by
/// authenticating again; only a confirmed change is wired, and only then is the
/// value returned to record — an unset or unconfirmed one records nothing.
pub async fn wire_qbittorrent_password(
    client: &Qbittorrent,
    random: &dyn Random,
    temporary: &str,
) -> (Wiring, Option<String>) {
    let connection = "qBittorrent web UI password".to_owned();
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

    match client.replace_password(temporary, &password).await {
        Ok(()) => (Wiring::settled(connection, State::Wired), Some(password)),
        Err(failure) => (Wiring::settled(connection, unreached(&failure)), None),
    }
}

/// Make Jellyfin the identity source for Seerr: mint and set Jellyfin's admin
/// account where its wizard has not run, then point Seerr's authentication at it.
///
/// Two services in order. Jellyfin has no key to read, so — like qBittorrent —
/// its admin password is one lemonfiber mints, sets by driving the first-run
/// wizard, and hands back for the surface to record; a wizard already run by the
/// household leaves its password unknown, so the wiring is skipped rather than
/// reset. With the credential in hand, Seerr is signed in through Jellyfin, which
/// on a fresh Seerr also creates its owner. An already-initialised Seerr is never
/// re-pointed, since that would cost the household its existing sign-ins. The
/// minted password is returned to record whenever the account was created, even
/// if Seerr itself could not then be reached, because the account now holds it.
pub async fn wire_jellyfin_identity(
    jellyfin: &dyn MediaServer,
    seerr: &dyn Requests,
    random: &dyn Random,
    recorded_password: Option<&str>,
    server_url: &str,
) -> (Wiring, Option<String>) {
    let connection = "Jellyfin as Seerr's identity".to_owned();
    let (password, minted) = match jellyfin_admin(jellyfin, random, recorded_password).await {
        Ok(pair) => pair,
        Err(state) => return (Wiring::settled(connection, state), None),
    };
    let state = configure_seerr(seerr, &password, server_url).await;
    (Wiring::settled(connection, state), minted)
}

/// The Jellyfin admin credential, and the password to record if it was newly
/// minted: minted where the wizard has not run, read from what was recorded where
/// it has, and unknown — so the wiring cannot proceed — where the household ran
/// the wizard itself.
async fn jellyfin_admin(
    jellyfin: &dyn MediaServer,
    random: &dyn Random,
    recorded: Option<&str>,
) -> Result<(String, Option<String>), State> {
    let completed = match jellyfin.startup_completed().await {
        Ok(done) => done,
        Err(failure) => return Err(unreached(&failure)),
    };
    if completed {
        return match recorded {
            Some(password) => Ok((password.to_owned(), None)),
            None => Err(State::Skipped {
                reason: "Jellyfin was set up outside lemonfiber, so its admin password is unknown; a later run cannot complete this until it is set up through lemonfiber".to_owned(),
            }),
        };
    }
    let Some(password) = secret::generate(random) else {
        return Err(State::Failed {
            detail: "no randomness was available to generate a password".to_owned(),
        });
    };
    match jellyfin.create_admin(ADMIN, &password).await {
        Ok(()) => Ok((password.clone(), Some(password))),
        Err(failure) => Err(unreached(&failure)),
    }
}

/// Point Seerr at the media server, unless it is already initialised — which is
/// left untouched, whether lemonfiber initialised it on an earlier run or the
/// household set it up with accounts of its own. A fresh Seerr is signed in and
/// then read back: it must report itself initialised, or the write did not land.
async fn configure_seerr(seerr: &dyn Requests, password: &str, server_url: &str) -> State {
    let initialized = match seerr.initialized().await {
        Ok(done) => done,
        Err(failure) => return unreached(&failure),
    };
    if initialized {
        return State::AlreadyWired;
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
