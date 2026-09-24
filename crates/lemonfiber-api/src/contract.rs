//! The machine-readable description of what the surfaces exchange.
//!
//! Every SDK generates its types from this rather than transcribing them, so a
//! field added here reaches every client without anyone retyping it. The types
//! described are the ones that actually serialise the reply, which is what stops
//! the description drifting from the thing it describes.
//!
//! The shapes are generated rather than written, and regenerating must
//! produce no diff — a serialised type that changes without the artefact
//! changing with it fails the build instead of reaching an SDK.
//!
//! A kind is described by the report it carries rather than by the [`Outcome`]
//! union those reports belong to. `Outcome` serialises as the report itself, with
//! no variant name around it, so the union's own shape is never what reaches a
//! client — and a schema derived from it would describe a document nothing writes.
//!
//! # Names, and the direction a change to them travels
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
//! Two changes settle the shape of this artefact, and **they travel in opposite
//! directions**. Each one taken the wrong way round fails silently rather than loudly,
//! which is why it is written beside the code rather than left in a pull request.
//!
//! **Renaming travels producer first, and has happened here.** `sdk-ts` compensates for
//! the old clashes by prefixing every divergent name with the kind carrying it. That
//! compensation is now redundant rather than wrong, and may be deleted — but only after
//! it has taken this artefact. The other order keys four different `Left`s to one name
//! and keeps whichever kind was written last, with nothing anywhere reporting it.
//!
//! **Hoisting `$defs` to the document root travels consumers first, and has not
//! happened.** Both SDKs resolve a reference against the kind carrying it, so a root
//! `$defs` leaves every reference unresolvable — and `sdk-php` answers an unresolvable
//! reference with `mixed` and exits nought. Every consumer has to resolve against the
//! root, and be released, before anything moves here.
//!
//! [`Outcome`]: lemonfiber_core::app::Outcome

mod path;
pub mod stability;

use std::collections::BTreeMap;

use schemars::{schema_for, Schema};
use serde::Serialize;

use lemonfiber_core::app::Outcome;
use lemonfiber_core::dashboard::Snapshot;
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{
    kind::{self, Kind},
    Envelope, SetupReport, API_VERSION,
};

use crate::admission::admitted::Admitted;
use crate::jobs::started::Started;
use lemonfiber_core::ports::docker::LogLine;
use lemonfiber_core::walkthrough::Line;

pub use path::CONTRACT_PATH;
pub use stability::{Surface, SURFACE_PATH};

/// Every wire shape a surface may receive, keyed by its `kind`.
///
/// Each entry is the whole envelope with that kind's payload in place, rather
/// than the payload alone: a generator wants the shape it will actually parse.
#[derive(Debug, Serialize)]
pub struct Contract {
    /// The wire version these shapes belong to.
    pub api_version: u32,
    /// `kind` to the schema of the envelope carrying it.
    pub kinds: BTreeMap<String, Schema>,
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
            kinds,
        }
    }

    /// As it is committed: sorted keys, two-space indent, one trailing newline.
    ///
    /// `None` only if it cannot serialise, which a tree of schemas cannot.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        let mut text = serde_json::to_string_pretty(self).ok()?;
        text.push('\n');
        Some(text)
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
    describing(kinds, kind::DASHBOARD, schema_for!(Envelope<Snapshot>));
    describing(kinds, kind::ERROR, schema_for!(Envelope<Problem>));
    describing(kinds, kind::JOB, schema_for!(Envelope<Started>));
    describing(kinds, kind::LOG, schema_for!(Envelope<LogLine>));
    describing(kinds, kind::PULL, schema_for!(Envelope<String>));
    describing(kinds, kind::SETUP, schema_for!(Envelope<SetupReport>));
    describing(kinds, kind::START, schema_for!(Envelope<String>));
    describing(kinds, kind::STEP, schema_for!(Envelope<Line>));
}

/// One kind, and the shape of the envelope carrying it.
///
/// Named rather than written out at each of two dozen call sites: the pair is the
/// whole of what a reader is here for, and the ceremony around it was three lines
/// of noise per kind.
fn describing(kinds: &mut BTreeMap<String, Schema>, kind: Kind, shape: Schema) {
    kinds.insert(kind.as_str().to_owned(), shape);
}
