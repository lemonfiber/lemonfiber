//! The download clients a service should be told about.
//!
//! Which credential proves each one, held per client rather than per kind, and what
//! category its transfers are filed under. Where each is reached is the filler's own
//! declaration, read through [`crate::wiring::Fillers`].

use std::collections::BTreeMap;

use lemonfiber_manifest::ApiKind;

use crate::ports::service::Credential;
use crate::wiring::{Filler, Fillers};

use super::{read_temporary_password, Ctx};

/// The category an application files under, named as that application names its
/// category field, for the media type it manages.
///
/// The field is fixed per application — Sonarr names it `tvCategory`, Radarr
/// `movieCategory`, Lidarr `musicCategory` — so the mapping is by the media type
/// that identifies the application. A media type lemonfiber does not recognise has
/// no known field, so it names none rather than guessing.
pub(super) fn category_for(media: &str) -> Option<crate::ports::service::Category> {
    let field = match media {
        "tv" => "tvCategory",
        "movies" => "movieCategory",
        "music" => "musicCategory",
        _ => return None,
    };
    Some(crate::ports::service::Category {
        field: field.to_owned(),
        value: media.to_owned(),
    })
}

/// The credential each download client on this machine answers to, by the service it
/// is.
///
/// Per service, because two clients speaking one adapter are two accounts: a client
/// standing in for another is told about with its own key, never the one the client it
/// replaced held.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Held(pub(super) BTreeMap<String, Credential>);

impl Held {
    /// The credential this service answers to, where it is in hand.
    pub(super) fn of(&self, service: &str) -> Option<&Credential> {
        self.0.get(service)
    }

    /// Every credential in hand, beside the service it proves.
    pub(super) fn each(&self) -> impl Iterator<Item = (&str, &Credential)> {
        self.0
            .iter()
            .map(|(service, credential)| (service.as_str(), credential))
    }
}

/// Every download client's credential that is in hand: a Usenet client's key, read from
/// the file it wrote it to, and a torrent client's password, minted this run or recorded
/// on an earlier one.
///
/// A client missing from the answer has not written its key yet, or has never had a
/// password set, and nothing is told about it until a later run finds one.
pub(super) async fn held(ctx: &Ctx, fillers: &Fillers, minted: &BTreeMap<String, String>) -> Held {
    let mut held = BTreeMap::new();
    for filler in fillers.speaking(ApiKind::Sabnzbd) {
        if let Some(key) = usenet_key(ctx, filler).await {
            held.insert(filler.id.clone(), Credential::ApiKey(key));
        }
    }
    for filler in fillers.speaking(ApiKind::Qbittorrent) {
        let Some(setting) = fillers.setting(filler, crate::config::PASSWORD_SUFFIX) else {
            continue;
        };
        let password = minted
            .get(&filler.id)
            .cloned()
            .or_else(|| crate::app::targets::recorded_secret(ctx, &setting));
        if let Some(password) = password {
            held.insert(
                filler.id.clone(),
                Credential::UserPass {
                    username: crate::config::QBITTORRENT_USER.to_owned(),
                    password,
                },
            );
        }
    }
    Held(held)
}

/// A Usenet client's API key, read from the file it writes it to, or nothing where it
/// names no such file or has not written one yet.
async fn usenet_key(ctx: &Ctx, filler: &Filler) -> Option<String> {
    let text = crate::app::targets::credential_file(ctx, filler).await?;
    crate::sabnzbd::api_key(&text)
}

/// Set every torrent client's web UI password, where it is still the one it started
/// with, and answer what each came to beside every password minted this run, by the
/// service it was minted for.
///
/// One that publishes no port is passed over: this machine sets the password through the
/// client's own web UI, and a client it cannot reach is one it cannot set anything on.
/// One whose password would be kept under a setting something else already holds is
/// refused before anything is read or set, and said.
pub(super) async fn seed_passwords(
    ctx: &Ctx,
    fillers: &Fillers,
) -> (Vec<crate::seed::Wiring>, BTreeMap<String, String>) {
    let mut wirings = Vec::new();
    let mut minted = BTreeMap::new();
    for filler in fillers.speaking(ApiKind::Qbittorrent) {
        let Some(port) = filler.published else {
            continue;
        };
        let Some(setting) = fillers.setting(filler, crate::config::PASSWORD_SUFFIX) else {
            wirings.push(crate::seed::Wiring::settled(
                password_connection(filler),
                crate::seed::State::Refused {
                    reason: format!(
                        "the setting {}'s password would be kept under is one lemonfiber or the \
                         stack already keeps something else in, so it is neither read nor set",
                        filler.name
                    ),
                },
            ));
            continue;
        };
        let base = crate::app::targets::loopback(port);
        let (wiring, generated) = seed_qbittorrent_password(ctx, filler, &base, &setting).await;
        wirings.push(wiring);
        if let Some(password) = generated {
            minted.insert(filler.id.clone(), password);
        }
    }
    (wirings, minted)
}

/// Set one torrent client's web UI password, where it is still the one it started with.
///
/// A password already recorded and still accepted is the one in force, and the
/// connection reports that rather than setting another. Otherwise the temporary
/// password is read from the container's own log; without it there is nothing to
/// authenticate with, so the connection is skipped for a re-run once the container
/// has announced one. A generated password that lands is recorded in the
/// environment under `setting`, the client's own.
pub(super) async fn seed_qbittorrent_password(
    ctx: &Ctx,
    filler: &Filler,
    base: &str,
    setting: &str,
) -> (crate::seed::Wiring, Option<String>) {
    let connection = password_connection(filler);
    let client = crate::qbittorrent::Qbittorrent::new(ctx.seams.http.clone(), base);

    // A password lemonfiber has already set is the one in force, and asking again
    // is how a healthy stack gets reported as refused. The temporary one is still
    // in the container's log — a log is not consumed by being read — so a run that
    // reached for it a second time would authenticate with a credential that was
    // spent the first time. Checked against the client rather than assumed from
    // the recording, because a container rebuilt from nothing holds neither.
    if let Some(recorded) = crate::app::targets::recorded_secret(ctx, setting) {
        // Signing in is how this question is answered, and signing in is a POST — state
        // left on the client by a run that promised to leave none. So a rehearsal says
        // what it could not tell rather than guessing: the recorded password is usually
        // still the one in force, and on a container rebuilt from nothing it is not,
        // and the difference is exactly what the sign-in exists to find out.
        if ctx.dry_run {
            return (
                crate::seed::Wiring::settled(
                    connection,
                    crate::seed::State::Skipped {
                        reason: UNTOLD_WITHOUT_SIGNING_IN.to_owned(),
                    },
                ),
                None,
            );
        }
        if client.accepts(&recorded).await.is_ok() {
            return (
                crate::seed::Wiring::settled(connection, crate::seed::State::AlreadyWired),
                None,
            );
        }
    }

    let Some(temporary) = read_temporary_password(ctx, &filler.id).await else {
        let wiring = crate::seed::Wiring::settled(
            connection,
            crate::seed::State::Skipped {
                reason: format!(
                    "{} has not announced a temporary password yet; a later run completes it",
                    filler.name
                ),
            },
        );
        return (wiring, None);
    };

    let (wiring, recorded) = crate::seed::wire_qbittorrent_password(
        &client,
        ctx.seams.random.as_ref(),
        &temporary,
        ctx.dry_run,
    )
    .await;

    // A rehearsal generates nothing, so there is nothing here to record and the
    // condition is already false. Said as a pair with the flag above rather than left
    // to that, because a value arriving from anywhere else would be written down by a
    // run that promised to write nothing.
    if let Some(password) = recorded.as_ref().filter(|_| !ctx.dry_run) {
        crate::app::targets::record_secret(ctx, setting, password);
    }
    (wiring, recorded)
}

/// What setting one torrent client's web UI password is called where it is reported.
fn password_connection(filler: &Filler) -> String {
    format!("{} web UI password", filler.name)
}

/// What a rehearsal says about a torrent client whose recorded password it will not
/// sign in to test.
const UNTOLD_WITHOUT_SIGNING_IN: &str = "whether the password lemonfiber recorded is \
     still the one in force is answered by signing in, which is a write and which a run \
     that only says what it would do does not make; a real run mints and sets a new one \
     where it is not";
