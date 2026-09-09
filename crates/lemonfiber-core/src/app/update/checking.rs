//! Asking the release list what is newest, rarely and quietly.
//!
//! Four ways out before the network is reached, and each is an answer rather than a
//! failure: the operator switched the check off, the machine has given up asking, the
//! answer from last time still stands, or nothing has ever been read and it is not
//! time. What is left reaches the address once, writes down how it went, and returns
//! whatever version it now knows of.
//!
//! What was last read is the answer wherever a fresh one cannot be had. A machine with
//! no route out is not a machine with no answer — it is one whose answer is a day, or
//! a week, old — and telling an operator nothing is known when something is would be a
//! worse report than a stale one.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::config::REACH_UPDATES_KEY;
use crate::outbound::RELEASE_LIST;
use crate::update::{asking, newest, Noticed, Silence};

/// What the record of past checks is kept in, beside the settings it belongs with.
const RECORD: &str = "updates.json";

/// What the check came to.
pub(super) struct Read {
    /// The newest version known, from this check or from an earlier one.
    pub(super) offered: Option<String>,
    /// Why none is known, where none is.
    pub(super) untold: Option<Silence>,
}

impl Read {
    /// What is known, or the reason nothing is.
    ///
    /// The two are exclusive by construction: a report carrying both a version and a
    /// sentence saying none could be read would be two answers to one question.
    fn of(offered: Option<&str>, quiet: Silence) -> Self {
        match offered {
            Some(offered) => Self {
                offered: Some(offered.to_owned()),
                untold: None,
            },
            None => Self {
                offered: None,
                untold: Some(quiet),
            },
        }
    }
}

/// The newest version this machine knows of, asking for it where that is due.
pub(super) async fn read(ctx: &Ctx) -> Read {
    let record = record(ctx);
    let mut noticed = remembered(ctx, record.as_deref()).await;
    if !ctx.settings.reaching.allows(REACH_UPDATES_KEY) {
        return Read::of(noticed.remembered(), Silence::Refused);
    }
    if noticed.given_up() {
        return Read::of(noticed.remembered(), Silence::GivenUp);
    }
    let now = ctx.seconds();
    if !noticed.due(now) {
        return Read::of(noticed.remembered(), Silence::NotYet);
    }
    asked(ctx, &mut noticed, now).await;
    keep(ctx, record.as_deref(), &noticed).await;
    Read::of(noticed.remembered(), Silence::Unanswered)
}

/// Ask the release list, and write down how it went.
///
/// A status that is not a success is a silence: an address that answered `403` has
/// answered, and it has answered with nothing a version can be read out of, which is
/// the same thing to a caller as nothing answering at all.
async fn asked(ctx: &Ctx, noticed: &mut Noticed, now: u64) {
    match ctx.http.send(&asking(RELEASE_LIST)).await {
        Ok(answer) if answer.is_success() => noticed.answered(now, newest(&answer.body)),
        Ok(_) | Err(_) => noticed.silent(now),
    }
}

/// Where the record of past checks sits, given where the settings file is.
///
/// Derived from the settings file rather than resolved again, so the two sit together
/// and a machine that would not say where its own files go keeps no record at all —
/// which reads as one that has never asked, and asks.
fn record(ctx: &Ctx) -> Option<PathBuf> {
    ctx.settings
        .env_file
        .as_ref()
        .map(|file| file.with_file_name(RECORD))
}

/// What the last few checks came to, or a machine that has never asked.
async fn remembered(ctx: &Ctx, at: Option<&Path>) -> Noticed {
    let Some(at) = at else {
        return Noticed::default();
    };
    ctx.filesystem
        .read(at)
        .await
        .and_then(|held| serde_json::from_str(&held).ok())
        .unwrap_or_default()
}

/// Write down how this check went, where there is anywhere to write it.
///
/// Best effort, and deliberately so: a record that could not be written means the next
/// run asks again sooner than it needed to, which is a wasted request rather than a
/// fault worth reporting to somebody who asked a different question.
async fn keep(ctx: &Ctx, at: Option<&Path>, noticed: &Noticed) {
    let (Some(at), Ok(written)) = (at, serde_json::to_string(noticed)) else {
        return;
    };
    ctx.filesystem.write(at, &written).await;
}
