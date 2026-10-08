//! The machine-readable description of what the surfaces exchange.
//!
//! Every SDK generates its types from this rather than transcribing them, so a
//! field added here reaches every client without anyone retyping it. The types
//! described are the ones that actually serialise the reply, which is what stops
//! the description drifting from the thing it describes.
//!
//! The shapes are generated rather than written, and regenerating must
//! produce no diff — a serialised type that changes without the artefact
//! changing with it fails the build instead of reaching an SDK. The artefact is a
//! directory: an index carrying the wire version, one file per kind, one per
//! definition, and one for each list that is not a kind.
//!
//! A kind is described by the report it carries rather than by the [`Outcome`]
//! union those reports belong to. `Outcome` serialises as the report itself, with
//! no variant name around it, so the union's own shape is never what reaches a
//! client — and a schema derived from it would describe a document nothing writes.
//!
//! # Names, and where a definition is kept
//!
//! A `$defs` key is what a generator keys a type by, so a key has to mean one type.
//! `schemars` names a definition after the bare Rust type and describes each kind on
//! its own, which met neither half of that. Two unrelated types called `Left` became
//! one name over two shapes, twenty-two names over in all; and two `Panel<T>` inside
//! one kind became `Panel` and `Panel2`, a number recording where the type was reached
//! rather than anything about it — so reordering a struct's fields renamed a published
//! type and nothing in the diff said so. Every type that collided now carries a
//! `#[schemars(rename = "...")]` of its own, and the sweeps below keep it that way.
//!
//! `sdk-ts` compensates for the old clashes by prefixing every divergent name with the
//! kind carrying it. That compensation is redundant rather than wrong, and may be
//! deleted on its side.
//!
//! Every definition is committed as a file of its own in `defs/`, and a reference is a
//! path to that file rather than a pointer into the kind carrying it, so a definition
//! nine kinds share is written once. What a reader has to do in return is resolve a
//! `$ref` against the file it appears in. A reader that cannot resolve one has to
//! refuse rather than describe the field as anything at all, because a reference left
//! unread produces a type that accepts everything and a build that reports nothing.
//!
//! [`Outcome`]: lemonfiber_core::app::Outcome

pub mod layout;
mod path;
mod split;
pub mod stability;

use std::collections::BTreeMap;

use schemars::{schema_for, Schema};
use serde::Serialize;

use lemonfiber_core::agreement;
use lemonfiber_core::alert::Alert;
use lemonfiber_core::app::plugins::REFUSALS;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::dashboard::Snapshot;
use lemonfiber_core::error::codes::declared;
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{
    kind::{self, Kind},
    Envelope, SetupReport, API_VERSION,
};
use lemonfiber_core::wiring;

use crate::actions::published::Action;
use crate::actions::{self, named, Arguments, KEY_CALLABLE};
use crate::admission::admitted::Admitted;
use crate::capabilities::Capabilities;
use crate::jobs::started::Started;

use crate::read::published::{self, Read};
use crate::refusal::Refusal;
use lemonfiber_core::app::rehearsal::Rehearsal;
use lemonfiber_core::logs::Line as LogLine;
use lemonfiber_core::news::Newest;
use lemonfiber_core::walkthrough::Line;

pub use path::{CONTRACT_DIR, INDEX};
pub use stability::{Surface, SURFACE_DIR};

/// Every wire shape a surface may receive, keyed by its `kind`.
///
/// Each entry is the whole envelope with that kind's payload in place, rather
/// than the payload alone: a generator wants the shape it will actually parse.
#[derive(Debug, Serialize)]
pub struct Contract {
    /// The wire version these shapes belong to.
    pub api_version: u32,
    /// Every action the surface takes: the arguments each takes with their types, the
    /// consent each asks for before it writes, and whether it takes a rehearsal.
    ///
    /// Beside the kinds for the reason the reads are: an action is where a change is
    /// asked for rather than a document, and a client generating a method per action
    /// reads this before it asks anything.
    pub actions: Vec<Action>,
    /// The route each request body is sent to, to the schema of that body: every
    /// route's but an action's, whose arguments `actions` lists one by one.
    ///
    /// Beside the kinds for the reason the reads are: a body is what a request carries
    /// rather than a document any request answers with, and a client building a
    /// request reads it before it asks anything.
    pub bodies: BTreeMap<String, Schema>,
    /// Every action a key may call, with whether it disturbs the running system,
    /// whether it takes a rehearsal and whether calling it twice is calling it once.
    ///
    /// Beside the kinds rather than inside one, for the reason the refusals are: it is
    /// not a document any request answers with, and a client deciding which controls to
    /// offer a key reads it before it asks anything.
    pub key_callable: Vec<Callable>,
    /// `kind` to the schema of the envelope carrying it.
    pub kinds: BTreeMap<String, Schema>,
    /// Every read the surface serves: where it is asked, what it may be given, and
    /// what it answers with.
    ///
    /// Beside the kinds for the reason the actions a key may call are: a read is where
    /// a document is asked for rather than a document, and a client generating a
    /// method per read reads this before it asks anything.
    pub reads: Vec<Read>,
    /// Every code a refusal may carry, to what the registry says of it.
    ///
    /// Beside the kinds rather than inside one, because a refusal is the `error` kind
    /// the artefact already describes and a code is a string there. What a client
    /// branches on is which of these a refusal is, so they are listed where a
    /// generator can give each one a name rather than copy it.
    pub refusals: BTreeMap<String, Listed>,
}

/// One action a key may call, as the contract lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Callable {
    /// The action, as `POST /api/actions/<action>` names it.
    pub action: &'static str,
    /// Whether calling it disturbs the running system.
    pub disturbs: bool,
    /// Whether it takes `dry_run`, read off the core's own account of the command it
    /// reaches, so a client can rehearse it and offer the real call after.
    pub rehearsal: bool,
    /// Whether calling it again with the same arguments leaves the stack as calling it
    /// once did, so a client can tell a person whether repeating it is safe.
    pub idempotent: bool,
}

/// One refusal as the contract lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listed {
    /// The name the code is declared under, which a generator names its value after.
    pub name: &'static str,
    /// The one status the refusal is answered with.
    pub status: u16,
    /// The line the registry writes above it.
    pub description: &'static str,
}

impl Contract {
    /// Builds the contract from the types that serialise the reply.
    #[must_use]
    pub fn describe() -> Self {
        let mut kinds = BTreeMap::new();
        answered(&mut kinds);
        beside(&mut kinds);

        Self {
            api_version: API_VERSION,
            actions: actions::published::every(rehearsable),
            bodies: bodies(),
            key_callable: key_callable(),
            kinds,
            reads: published::every(),
            refusals: refusals(),
        }
    }
}

/// The shapes a command's own answer takes, one per [`Outcome`] variant, as the
/// list that declares [`Outcome`] names them.
fn answered(kinds: &mut BTreeMap<String, Schema>) {
    Outcome::schemas(|kind, shape| describing(kinds, kind, shape));
}

/// The shapes that belong to no command's answer.
///
/// A session, a failure, a name for work that outlives its request, and the lines a
/// long run says while it is still running — none of which any [`Outcome`] carries,
/// and each of which a caller still has to parse.
///
/// The walkthrough and the supervision report sat here and are carried by an outcome,
/// which is what a count could not tell anybody: the doc said one thing, the code did
/// another, and the guard compared two integers that agreed.
fn beside(kinds: &mut BTreeMap<String, Schema>) {
    describing(kinds, kind::ADMISSION, schema_for!(Envelope<Admitted>));
    describing(kinds, kind::ALERT, schema_for!(Envelope<Alert>));
    describing(
        kinds,
        kind::CAPABILITIES,
        schema_for!(Envelope<Capabilities>),
    );
    describing(kinds, kind::DASHBOARD, schema_for!(Envelope<Snapshot>));
    describing(kinds, kind::ERROR, schema_for!(Envelope<Problem>));
    describing(kinds, kind::JOB, schema_for!(Envelope<Started>));
    describing(kinds, kind::LOG, schema_for!(Envelope<LogLine>));
    describing(kinds, kind::NEWS, schema_for!(Envelope<Newest>));
    describing(kinds, kind::PULL, schema_for!(Envelope<String>));
    describing(kinds, kind::SETUP, schema_for!(Envelope<SetupReport>));
    describing(kinds, kind::START, schema_for!(Envelope<String>));
    describing(kinds, kind::STEP, schema_for!(Envelope<Line>));
}

/// Every refusal this surface answers with, keyed by its code.
///
/// Its own, and the core's refusals of an answer that named an offer or a listing that
/// has since moved: those end work rather than a request, and they are the one refusal
/// a client answers by reading again rather than by reporting a failure, so a client
/// has to be able to name them as it names this surface's own.
///
/// And the refusals of the plugins and wiring reads where what they are read from could
/// not be read, which a client has to tell apart from an empty answer by name, and of a
/// choice of what fills a capability and of an install, an update or a removal, each at
/// the status its code is declared with.
///
/// A code the registry does not declare cannot be built, so every refusal is found;
/// one missing here would be a code no client can name, and a test counts them.
fn refusals() -> BTreeMap<String, Listed> {
    Refusal::EVERY
        .iter()
        .map(|refusal| refusal.code())
        .chain(agreement::MOVED)
        .chain(
            wiring::UNREAD
                .iter()
                .flat_map(|codes| codes.iter().copied()),
        )
        .chain(wiring::REFUSED)
        .chain(REFUSALS)
        .filter_map(|code| {
            let declared = declared(code)?;
            Some((
                declared.code().as_str().to_owned(),
                Listed {
                    name: declared.name(),
                    status: declared.status(),
                    description: declared.description(),
                },
            ))
        })
        .collect()
}

/// Every body a route takes, keyed by the route, from the type the route reads it into.
fn bodies() -> BTreeMap<String, Schema> {
    crate::setup::bodies()
        .into_iter()
        .chain([crate::keys::body(), crate::admission::body()])
        .map(|(route, schema)| (route.to_owned(), schema))
        .collect()
}

/// Every action a key may call, each with what calling it is like.
fn key_callable() -> Vec<Callable> {
    KEY_CALLABLE
        .iter()
        .map(|by| Callable {
            action: by.action,
            disturbs: by.disturbs,
            rehearsal: rehearsable(by.action),
            idempotent: by.idempotent,
        })
        .collect()
}

/// Whether an action takes `dry_run`, as the core decides for the command it reaches.
///
/// The command is named from the plainest arguments that name one — nothing, a form,
/// or the checks that disturb the system — because what an action reaches is decided by
/// its name and those, never by which form or which service it was given.
#[must_use]
pub fn rehearsable(action: &str) -> bool {
    let given = |forms: bool, disruptive: bool| Arguments {
        forms: if forms {
            vec!["any".to_owned()]
        } else {
            Vec::new()
        },
        disruptive: disruptive.into(),
        ..Arguments::default()
    };
    [given(false, false), given(true, false), given(false, true)]
        .into_iter()
        .find_map(|arguments| named(action, arguments).ok())
        .is_some_and(|command| {
            matches!(
                lemonfiber_core::app::rehearsal::asked(&command).rehearsal,
                Rehearsal::Reads | Rehearsal::Reports
            )
        })
}

/// One kind, and the shape of the envelope carrying it.
///
/// Named rather than written out at each of two dozen call sites: the pair is the
/// whole of what a reader is here for, and the ceremony around it was three lines
/// of noise per kind.
fn describing(kinds: &mut BTreeMap<String, Schema>, kind: Kind, shape: Schema) {
    kinds.insert(kind.as_str().to_owned(), shape);
}
