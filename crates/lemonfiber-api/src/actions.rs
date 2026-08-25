//! What a write request asks for, and what becomes of it.
//!
//! An action is named, and the name is turned into one of the core's own
//! commands. That translation is the whole of this surface's authority: a command
//! is what the command line produces too, so an action reaching a command cannot
//! be something only a browser can do. A name outside the table is refused rather
//! than invented, which is what keeps the two surfaces the same size.
//!
//! Only the actions that *change* something are here. Asking what the stack is
//! doing is a read and has an endpoint of its own; a write that also happens to
//! report is still a write.
//!
//! An action that reaches the container engine or the services runs for minutes,
//! and a request that waited for it would tie the work to a connection. So those
//! are answered with a name for the work instead, and the work runs somewhere the
//! connection cannot reach — a browser tab closed mid-repair takes nothing with
//! it. What that name is redeemed for lives in [`crate::jobs`]. An action that
//! only reads and writes lemonfiber's own files is answered with its outcome,
//! because it has already finished by the time it could be.
//!
//! No payload is serialised here. An envelope renders itself, and the same
//! rendering answers the command line.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use axum::{Json, Router};
use lemonfiber_core::app::{Command, QualityAction};
use lemonfiber_core::audio::Format;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;
use serde::Deserialize;

use crate::jobs::{accepted, Job};
use crate::read::carried_out;
use crate::router::Serving;
use crate::serve::{carrying, SENTENCE};

/// The arguments an action was given, mirroring the flags its command takes.
///
/// Declared as one carrier rather than one shape per action, so a caller sends
/// the field an action names and nothing else. A name the carrier does not hold
/// is refused rather than ignored: a caller who spelled `service` where `services`
/// was meant has been told, instead of watching a whole form stop.
///
/// A name the carrier holds and the action's command has nowhere to put is refused
/// too. Those are the fields that would have changed what the action did, so
/// dropping one answers a different request from the one that was asked — an
/// agreement that turns out to guard nothing, or a service named to a start that
/// then starts the whole form.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Arguments {
    /// The forms to act on.
    pub forms: Vec<String>,
    /// The services to act on, leaving the rest of the form alone.
    pub services: Vec<String>,
    /// The setting to change.
    pub key: Option<String>,
    /// What to change it to.
    pub value: Option<String>,
    /// The quality preset, or the music format, as it is written.
    pub preset: Option<String>,
    /// The media type a quality choice applies to.
    pub media_type: Option<String>,
    /// Whether a cost the action would incur was agreed to in advance.
    pub confirm: bool,
}

/// Why an action was not carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// No action goes by that name.
    Unknown {
        /// The name as it was asked for.
        name: String,
    },
    /// The action needs an argument that was not given.
    Missing {
        /// The action that needs it.
        action: String,
        /// The argument it needs.
        argument: String,
    },
    /// The argument was given and names nothing.
    Unrecognised {
        /// The argument that was given.
        argument: String,
        /// What it said, and what it could have said instead.
        offered: String,
    },
    /// The argument was given to an action whose command has nowhere to put it.
    Unwanted {
        /// The action it was given to.
        action: String,
        /// The argument it does not take.
        argument: String,
    },
}

impl Refused {
    /// The status a refusal answers with.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::Unknown { .. } => StatusCode::NOT_FOUND,
            Self::Missing { .. } | Self::Unrecognised { .. } | Self::Unwanted { .. } => {
                StatusCode::BAD_REQUEST
            }
        }
    }

    /// What the refusal says, in the one line a reader gets.
    #[must_use]
    pub fn said(&self) -> String {
        match self {
            Self::Unknown { name } => format!(
                "There is no action named `{name}`. \
                 This surface offers what the command line offers, and nothing else."
            ),
            Self::Missing { action, argument } => {
                format!("The action `{action}` needs `{argument}`, which was not given.")
            }
            Self::Unrecognised { argument, offered } => {
                format!("The `{argument}` given is not one this stack knows: {offered}")
            }
            Self::Unwanted { action, argument } => format!(
                "The action `{action}` takes no `{argument}`. It is refused rather \
                 than dropped, because dropping it would carry out a different \
                 request from the one asked for."
            ),
        }
    }
}

/// When an action's answer arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answering {
    /// With the outcome, because the work is already done.
    Now,
    /// With a name for the work, which goes on reporting after this reply.
    Later,
}

/// Which of the two an action is.
///
/// The rule is where the work happens rather than how long it has taken before:
/// anything reaching the container engine or a service is a wait an operator
/// should be able to watch, and anything confined to lemonfiber's own files has
/// finished by the time a reply could be written.
#[must_use]
pub const fn answering(command: &Command) -> Answering {
    match command {
        Command::ConfigSet { .. } | Command::Quality(_) | Command::Setup(_) => Answering::Now,
        _ => Answering::Later,
    }
}

/// The command an action names, or why it names none.
///
/// # Errors
///
/// Returns the [`Refused`] a caller should be answered with.
pub fn named(action: &str, given: Arguments) -> Result<Command, Refused> {
    let Arguments {
        forms,
        services,
        key,
        value,
        preset,
        media_type,
        confirm,
    } = given;
    let needs = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    // Naming nothing means everything for the actions that can mean it, and is a
    // mistake for the three that cannot: switching to nothing, restarting nothing
    // and fetching nothing are each a request that has lost its subject, and the
    // command line refuses all three the same way.
    if NAMES_ITS_FORMS.contains(&action) && forms.is_empty() {
        return Err(needs("forms"));
    }
    // An argument that changes what an action does, given to an action whose command
    // has nowhere to put it, is refused rather than dropped: dropped, it carries out
    // a different request from the one that was asked and says nothing about having
    // done so. Only for a name this surface offers — a name it does not offer is
    // absent before it is anything else.
    let carried: [(&str, bool, &[&str]); 7] = [
        ("forms", !forms.is_empty(), TAKES_FORMS),
        ("services", !services.is_empty(), TAKES_SERVICES),
        ("key", key.is_some(), TAKES_SETTING),
        ("value", value.is_some(), TAKES_SETTING),
        ("preset", preset.is_some(), TAKES_PRESET),
        ("media_type", media_type.is_some(), TAKES_PRESET),
        ("confirm", confirm, TAKES_AGREEMENT),
    ];
    for (argument, given, takers) in carried {
        if given && OFFERED.contains(&action) && !takers.contains(&action) {
            return Err(Refused::Unwanted {
                action: action.to_owned(),
                argument: argument.to_owned(),
            });
        }
    }
    match action {
        "up" => Ok(Command::Up { forms }),
        // Stopping named services and tearing a form down are different requests
        // rather than one request with an argument, exactly as the command line
        // reads them, and Compose spells them differently too.
        "down" if services.is_empty() => Ok(Command::Down { forms }),
        "down" => Ok(Command::Halt { forms, services }),
        "switch" => Ok(Command::Switch { forms }),
        "restart" => Ok(Command::Restart { forms, services }),
        "pull" => Ok(Command::Pull { forms }),
        "config-set" => match (key, value) {
            (Some(key), Some(value)) => Ok(Command::ConfigSet { key, value }),
            (None, _) => Err(needs("key")),
            (_, None) => Err(needs("value")),
        },
        "quality-set" => match preset {
            Some(preset) => quality(&preset, media_type, confirm),
            None => Err(needs("preset")),
        },
        "quality-reapply" => Ok(Command::Quality(QualityAction::Reapply)),
        "quality-upgrade" => Ok(Command::QualityUpgrade { confirm }),
        "seed" => Ok(Command::Seed),
        "adopt" => Ok(Command::Adopt),
        "reset" => Ok(Command::Reset { confirm }),
        _ => Err(Refused::Unknown {
            name: action.to_owned(),
        }),
    }
}

/// The actions that must be told what to act on.
const NAMES_ITS_FORMS: [&str; 3] = ["switch", "restart", "pull"];

/// The actions whose command carries the forms it was given.
///
/// The lifecycle five, and nothing else. Seeding, adopting, resetting, changing a
/// setting and choosing a quality are whole-stack requests on every surface: no
/// command has a field to narrow them by and the command line declares no argument
/// for one, so a form named to any of them is a narrowing that was never available.
pub const TAKES_FORMS: &[&str] = &["up", "down", "switch", "restart", "pull"];

/// The actions whose command carries the operator's agreement.
///
/// The three the command line declares `--confirm` on, and no others. Each names
/// something that cannot be taken back once it is done: quality this host would
/// have to transcode in software, bandwidth spent re-fetching a library that is
/// already here, and hand-edits to the stack files discarded. Unconfirmed, each of
/// the three reports what it would cost and changes nothing, so the agreement is a
/// fork inside the command rather than a gate in front of it.
///
/// A teardown is not one of them. `down` removes what a form started and `up` puts
/// it back, so there is no cost to agree to in advance — and the question the
/// command line does ask before a teardown is not this one. It asks whether to let
/// a download in flight finish, which is answered by waiting rather than by
/// agreeing, and a machine-readable run is never asked it at all.
///
/// Choosing for music is inside `quality-set` and drops the agreement, because
/// picking an audio format is not a choice this host has to transcode for. The
/// command line drops it there too.
pub const TAKES_AGREEMENT: &[&str] = &["quality-set", "quality-upgrade", "reset"];

/// The actions whose command carries the services it was given.
///
/// Stopping named services is `Command::Halt`, a different request from a teardown
/// rather than a teardown with an argument, and a restart carries the services it
/// restarts. No other command has a field to put them in.
///
/// `up` is the one whose absence costs something. The command line starts named
/// services through a streamed path of its own that never reaches a `Command`, so
/// there is nothing here to hand them to — and dropping them starts every service
/// the form holds, which is the answer to a request nobody made. Whether starting
/// named services is its own request, the way `Halt` is its own request, is a
/// question for the core rather than for this table.
pub const TAKES_SERVICES: &[&str] = &["down", "restart"];

/// The action whose command carries the setting it was given.
///
/// One setting is read or written by name, and no other action is about a setting
/// at all.
pub const TAKES_SETTING: &[&str] = &["config-set"];

/// The action whose command carries the quality it was given.
///
/// Choosing is the only one of the three quality actions that takes a preset or a
/// media type. Re-asserting the recorded choice re-asserts what is recorded, and
/// upgrading re-fetches at what is recorded — neither takes a new choice, and the
/// command line declares neither argument on them.
///
/// `quality-upgrade` is where dropping one costs the most. A caller that named a
/// preset and a media type asked to upgrade one kind of media to one quality; the
/// command re-fetches the whole library at the quality already recorded, which is
/// a far larger download than the one the agreement beside it was given for.
pub const TAKES_PRESET: &[&str] = &["quality-set"];

/// Every action this surface offers, in the order they are worth reading.
///
/// Held as a list so that what the surface offers can be counted and checked
/// against what it translates, rather than being knowable only by reading a
/// match arm at a time.
pub const OFFERED: &[&str] = &[
    "up",
    "down",
    "switch",
    "restart",
    "pull",
    "config-set",
    "quality-set",
    "quality-reapply",
    "quality-upgrade",
    "seed",
    "adopt",
    "reset",
];

/// A quality choice, which is two commands depending on what it is about.
///
/// Music has no resolution: choosing for it picks an audio format rather than a
/// resolution preset, and reaches the service rather than only recording, so it
/// routes to its own command — the same fork the command line takes.
fn quality(preset: &str, media_type: Option<String>, confirm: bool) -> Result<Command, Refused> {
    if media_type.as_deref() == Some("music") {
        let Some(format) = Format::from_label(preset) else {
            return Err(Refused::Unrecognised {
                argument: "preset".to_owned(),
                offered: "try compact, lossless, or hi-res".to_owned(),
            });
        };
        return Ok(Command::QualityMusic { format });
    }
    let Some(preset) = Preset::from_label(preset) else {
        return Err(Refused::Unrecognised {
            argument: "preset".to_owned(),
            offered: "try space-saving, balanced, high-quality, or maximum".to_owned(),
        });
    };
    // Asked as one value and answered with one expression rather than through an
    // early return: a block that only ever ends by leaving leaves its own closing
    // brace with nothing to run it.
    let known = media_type
        .as_deref()
        .is_none_or(|named| Kind::ALL.iter().any(|kind| kind.media_type() == named));
    if known {
        Ok(Command::Quality(QualityAction::Set {
            preset,
            media_type,
            confirm,
        }))
    } else {
        Err(Refused::Unrecognised {
            argument: "media_type".to_owned(),
            offered: "try tv, movies, or music".to_owned(),
        })
    }
}

/// An action refused, said plainly rather than as a bare status.
///
/// Prose rather than an envelope, and labelled as prose, the way every other
/// request this surface could not read is answered.
#[must_use]
pub fn declined(refused: &Refused) -> Response {
    carrying(refused.status(), SENTENCE, Body::from(refused.said()))
}

/// The route every action is asked for through.
///
/// Admission is not applied here. Whether a request may be answered at all is one
/// question for the whole surface, asked once above the whole tree, which is what
/// keeps an endpoint added later from arriving unguarded.
pub fn routes() -> Router<Serving> {
    Router::new().route("/api/actions/{action}", post(taken))
}

/// One action, carried out or refused.
async fn taken(
    State(serving): State<Serving>,
    Path(action): Path<String>,
    Json(given): Json<Arguments>,
) -> Response {
    let command = match named(&action, given) {
        Ok(command) => command,
        Err(refused) => return declined(&refused),
    };
    match answering(&command) {
        Answering::Now => carried_out(&serving.ctx, command).await,
        Answering::Later => {
            let Some(job) = Job::mint(serving.ctx.random.as_ref()) else {
                return unnameable();
            };
            serving
                .jobs
                .start(&job, &action, command, Arc::clone(&serving.ctx))
                .await;
            accepted(&job, &action)
        }
    }
}

/// Work that could not be named, and therefore was not begun.
///
/// A job with no name is work nothing could ever be told about, so there is
/// nothing here to fall back to.
fn unnameable() -> Response {
    carrying(
        StatusCode::INTERNAL_SERVER_ERROR,
        SENTENCE,
        Body::from("This machine would not supply the randomness a job needs to be named."),
    )
}
