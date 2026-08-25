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
//! it. An action that only reads and writes lemonfiber's own files is answered
//! with its outcome, because it has already finished by the time it could be.
//!
//! No payload is serialised here. An envelope renders itself, and the same
//! rendering answers the command line.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use axum::{Json, Router};
use lemonfiber_core::app::{dispatch, Command, Ctx, QualityAction};
use lemonfiber_core::audio::Format;
use lemonfiber_core::model::{kind, Envelope, Started};
use lemonfiber_core::ports::random::Random;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::guard::hex;
use crate::read::{carried_out, enveloped};
use crate::router::Serving;
use crate::serve::{carrying, SENTENCE};

/// Bytes of name. Wide enough that two runs never mint the same one.
const WIDTH: usize = 8;

/// The arguments an action was given, mirroring the flags its command takes.
///
/// Declared as one carrier rather than one shape per action, so a caller sends
/// the field an action names and nothing else. A field an action does not use is
/// refused rather than ignored: a caller who spelled `service` where `services`
/// was meant has been told, instead of watching a whole form stop.
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
}

impl Refused {
    /// The status a refusal answers with.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::Unknown { .. } => StatusCode::NOT_FOUND,
            Self::Missing { .. } | Self::Unrecognised { .. } => StatusCode::BAD_REQUEST,
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

/// A name for work that outlives the request that started it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job(String);

impl Job {
    /// Mints one, or nothing when the operating system will not say.
    ///
    /// Through the port rather than taken directly, so a test names a job it
    /// chose instead of depending on what the machine happens to produce.
    pub fn mint(random: &dyn Random) -> Option<Self> {
        Some(Self(hex(&random.bytes(WIDTH)?)))
    }

    /// The name as it is answered with, and as it appears on the stream.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where a piece of work got to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Still going.
    Running,
    /// Finished, and this is the envelope it came to.
    Done(String),
    /// Stopped, and this is the envelope saying why.
    Failed(String),
}

/// The work this run started, and where each piece of it got to.
///
/// Held for the life of the run rather than written down. A job names work in
/// flight, and work in flight does not survive the process that is doing it —
/// so a record that outlived the run would describe jobs nothing is running.
#[derive(Clone, Default)]
pub struct Jobs(Arc<Mutex<HashMap<String, Standing>>>);

impl Jobs {
    /// Start work under a name, and stop holding on to it.
    ///
    /// The work is handed to the runtime rather than awaited, which is the whole
    /// point: what happens to the request afterwards — answered, abandoned, a tab
    /// closed — cannot reach it.
    pub async fn start(&self, job: &Job, command: Command, ctx: Arc<Ctx>) {
        let (name, held) = (job.as_str().to_owned(), Arc::clone(&self.0));
        held.lock().await.insert(name.clone(), Standing::Running);
        tokio::spawn(async move {
            // Nothing is invented for a payload that could not be rendered: these
            // are plain owned values, and the empty arm is reached only by being
            // handed one, which nothing here can be.
            let standing = match dispatch(command, &ctx).await {
                Ok(outcome) => Standing::Done(outcome.envelope().to_json().unwrap_or_default()),
                Err(problem) => Standing::Failed(
                    Envelope::new(kind::ERROR, &*problem)
                        .to_json()
                        .unwrap_or_default(),
                ),
            };
            held.lock().await.insert(name, standing);
        });
    }

    /// Where a named piece of work got to, or nothing for a name this run never
    /// handed out.
    pub async fn standing(&self, job: &str) -> Option<Standing> {
        self.0.lock().await.get(job).cloned()
    }
}

/// A job accepted, named so the stream can be followed for it.
#[must_use]
pub fn accepted(job: &Job, action: &str) -> Response {
    let started = Started {
        job: job.as_str().to_owned(),
        action: action.to_owned(),
    };
    enveloped(
        StatusCode::ACCEPTED,
        Envelope::new(kind::JOB, started).to_json(),
    )
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
                .start(&job, command, Arc::clone(&serving.ctx))
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
