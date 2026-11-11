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
use crate::ports::service::{Failure, Heard, Hearing, Recounting};
use crate::seed::{State, Wiring};
use crate::wiring::{Filler, Fillers};

/// What the report calls this connection for the service `name`, with how it hears what
/// happened where it said.
fn connection(name: &str, heard: Option<Heard>) -> String {
    let how = match heard {
        Some(Heard {
            hearing: Hearing::Push,
            ..
        }) => ", as it is told".to_owned(),
        Some(Heard {
            hearing: Hearing::Poll,
            every,
        }) => every.map_or_else(
            || ", by looking for it".to_owned(),
            |seconds| format!(", by looking for it every {seconds} s"),
        ),
        None => String::new(),
    };
    format!("{name} telling the core what happened{how}")
}

/// How many seconds apart a filler that polls is asked to look.
fn every(ctx: &Ctx) -> u32 {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|path| crate::config::store::read(path).ok())
        .map_or(crate::ports::service::EVERY_DEFAULT, |file| {
            crate::config::events_poll_from_env(&file)
        })
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
        let (state, heard) = match (&told, ctx.dry_run) {
            (Err(why), _) => (
                State::Unmatched {
                    reason: why.clone(),
                },
                None,
            ),
            (Ok(told), true) => (
                State::WouldWire {
                    yours: None,
                    ours: Some(told.address.clone()),
                },
                None,
            ),
            (Ok(told), false) => asked(ctx, speaker, capability, major, told).await,
        };
        wirings.push(Wiring::settled(connection(&part.name, heard), state));
    }
    wirings
}

/// What became of asking `speaker`, over `capability` at `major`, to tell the core where
/// `told` says, with how it hears where it said.
async fn asked(
    ctx: &Ctx,
    speaker: &Filler,
    capability: &str,
    major: u32,
    told: &Told,
) -> (State, Option<Heard>) {
    match hearing(ctx, speaker, capability, major, told).await {
        Ok(heard) => (State::Wired, heard),
        Err(state) => (state, None),
    }
}

/// How `speaker`, asked over `capability` at `major` to tell the core where `told` says,
/// hears what happened, or the state it was left in instead.
async fn hearing(
    ctx: &Ctx,
    speaker: &Filler,
    capability: &str,
    major: u32,
    told: &Told,
) -> Result<Option<Heard>, State> {
    let Spoken::Over(adapter) = spoken(ctx, speaker, capability, major).await else {
        return Err(State::Skipped {
            reason: format!(
                "{} speaks {} and could not be asked over it",
                speaker.name,
                lemonfiber_contract::spoken(capability, major)
            ),
        });
    };
    let key = crate::notify::events::minted(ctx, &speaker.id)
        .map_err(|detail| State::Failed { detail })?;
    written(ctx, speaker, &key).map_err(|detail| State::Failed { detail })?;
    let every = Some(every(ctx));
    let answer = if capability == curate::CAPABILITY {
        curate::Adapter(adapter)
            .events(Some(&told.address), Some(&told.pin), Some(&key), every)
            .await
    } else {
        serve::Adapter(adapter)
            .events(Some(&told.address), Some(&told.pin), Some(&key), every)
            .await
    };
    answer.map_err(|failure| match failure {
        Failure::Unsupported { detail, .. } => State::Unmatched {
            reason: format!(
                "{} cannot tell the core what happened yet: {detail}",
                speaker.name
            ),
        },
        failure => crate::seed::unreached(&failure),
    })
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
