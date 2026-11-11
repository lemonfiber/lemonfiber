//! Each curator and media server asked to tell the core what happened, at the address it
//! reaches the core at, pinning the certificate the core presents there, and with a key
//! minted for it alone.
//!
//! A fresh key each time it is asked, its digest kept in place of the last, so a key that
//! leaked stops working at the next wiring. A service speaking a minor from before
//! `events` is left as it is, and said so.

use lemonfiber_contract::adapter::EVENTS_KEY_FILE;
use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::capabilities::media::serve;

use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::notify::events::Told;
use crate::ports::filesystem::Readable;
use crate::ports::service::{Failure, Recounting};
use crate::seed::{State, Wiring};
use crate::wiring::{Filler, Fillers};

/// What the report calls this connection for the service `name`.
fn connection(name: &str) -> String {
    format!("{name} telling the core what happened")
}

/// Ask every curator and media server here to tell the core what happened where `told`
/// says, or say why none is where it says nothing can be.
pub(super) async fn seed_events(
    ctx: &Ctx,
    fillers: &Fillers,
    told: Result<Told, String>,
) -> Vec<Wiring> {
    let speakers: Vec<(&Filler, &'static str, u32)> = fillers
        .services()
        .filter_map(|one| {
            [
                (curate::CAPABILITY, curate::MAJOR),
                (serve::CAPABILITY, serve::MAJOR),
            ]
            .into_iter()
            .find(|(capability, major)| one.contracted(capability, *major))
            .map(|(capability, major)| (one, capability, major))
        })
        .collect();
    let mut wirings = Vec::new();
    for (speaker, capability, major) in speakers {
        let part = fillers.fronted_by(speaker).unwrap_or(speaker);
        let state = match (&told, ctx.dry_run) {
            (Err(why), _) => State::Unmatched {
                reason: why.clone(),
            },
            (Ok(told), true) => State::WouldWire {
                yours: None,
                ours: Some(told.address.clone()),
            },
            (Ok(told), false) => asked(ctx, speaker, capability, major, told).await,
        };
        wirings.push(Wiring::settled(connection(&part.name), state));
    }
    wirings
}

/// What became of asking `speaker`, over `capability` at `major`, to tell the core where
/// `told` says.
async fn asked(ctx: &Ctx, speaker: &Filler, capability: &str, major: u32, told: &Told) -> State {
    let Spoken::Over(adapter) = spoken(ctx, speaker, capability, major).await else {
        return State::Skipped {
            reason: format!(
                "{} speaks {} and could not be asked over it",
                speaker.name,
                lemonfiber_contract::spoken(capability, major)
            ),
        };
    };
    let key = match crate::notify::events::minted(ctx, &speaker.id) {
        Ok(key) => key,
        Err(detail) => return State::Failed { detail },
    };
    if let Err(detail) = written(ctx, speaker, &key) {
        return State::Failed { detail };
    }
    let answer = if capability == curate::CAPABILITY {
        curate::Adapter(adapter)
            .events(Some(&told.address), Some(&told.pin), Some(&key))
            .await
    } else {
        serve::Adapter(adapter)
            .events(Some(&told.address), Some(&told.pin), Some(&key))
            .await
    };
    match answer {
        Ok(()) => State::Wired,
        Err(Failure::Unsupported { detail, .. }) => State::Unmatched {
            reason: format!(
                "{} cannot tell the core what happened yet: {detail}",
                speaker.name
            ),
        },
        Err(failure) => crate::seed::unreached(&failure),
    }
}

/// `key` written beside `speaker`'s configuration, readable by its user alone.
pub(super) fn written(ctx: &Ctx, speaker: &Filler, key: &str) -> Result<(), String> {
    let within = speaker.confined_to.as_deref().ok_or_else(|| {
        format!(
            "{} has no configuration on this machine to hold the key it tells the core with",
            speaker.name
        )
    })?;
    ctx.seams
        .confined
        .replaced(
            &within.join(EVENTS_KEY_FILE),
            within,
            format!("{key}\n").as_bytes(),
            Readable::Private,
        )
        .map_err(|fault| {
            format!(
                "the key {} tells the core with could not be written: {}",
                speaker.name, fault.message
            )
        })
}
