//! The places a plugin may extend lemonfiber itself.
//!
//! A point is not a hook and not a callback. It is a register that already exists,
//! already has bundled rows in it, and already has one evaluator — and the point names
//! the place a plugin may put another row. That is what keeps "nothing contributed is
//! executed" true without an argument: the row is data, and the engine reading it is
//! lemonfiber's, unchanged.
//!
//! Publishing them is what turns a declaration lemonfiber does not recognise into a
//! refusal that names it. A contribution skipped because nothing recognised it leaves
//! a plugin whose stated behaviour is narrower than its actual one, and a contributed
//! check that quietly disappears from a doctor run is worse than one that fails —
//! because the stack then looks healthy for the wrong reason.

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

/// The generation these points are at.
///
/// Monotonic. Advanced by removing a point or narrowing a row; adding a point does not
/// move it.
pub const EXTENSION_POINTS_VERSION: u32 = 1;

/// A place a plugin may put a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// What a contribution declares `at`. Unique.
    pub name: &'static str,
    /// What a row here is, in one line.
    pub summary: &'static str,
    /// The register a row joins, in the operator's words.
    pub register: &'static str,
    /// What already reads that register, and on what terms.
    pub engine: &'static str,
    /// Exactly what a row carries.
    pub row: Row,
    /// The capability a manifest must ask for in order to contribute here.
    pub requires: &'static str,
}

/// What a row at one point carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Row {
    /// The fields a row must have.
    pub required: &'static [&'static str],
    /// The fields it may have.
    pub optional: &'static [&'static str],
    /// The bounds on each bounded field, keyed by the field.
    #[serde(
        skip_serializing_if = "<[Bounded]>::is_empty",
        serialize_with = "bounded_by_field"
    )]
    pub bounds: &'static [Bounded],
    /// The closed set each field with one may draw from, keyed by the field.
    #[serde(
        skip_serializing_if = "<[Closed]>::is_empty",
        serialize_with = "closed_by_field"
    )]
    pub enums: &'static [Closed],
}

/// The bounds, as a table keyed by the field they are on.
///
/// Written out rather than derived because the artefact keys both of these by field
/// name and a list of Rust structs is the shape that actually reads well in source. A
/// sorted map would have done as well today, with one entry each — and would quietly
/// reorder the artefact the first time a point declared two.
fn bounded_by_field<S: Serializer>(
    bounds: &&'static [Bounded],
    into: S,
) -> Result<S::Ok, S::Error> {
    let mut table = into.serialize_map(Some(bounds.len()))?;
    for bound in *bounds {
        table.serialize_entry(bound.field, &bound.limits)?;
    }
    table.end()
}

/// The closed sets, the same way.
fn closed_by_field<S: Serializer>(closed: &&'static [Closed], into: S) -> Result<S::Ok, S::Error> {
    let mut table = into.serialize_map(Some(closed.len()))?;
    for one in *closed {
        table.serialize_entry(one.field, one.values)?;
    }
    table.end()
}

/// One bounded field, and what it is bounded to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounded {
    /// The field.
    pub field: &'static str,
    /// What it is bounded to.
    pub limits: Limits,
}

/// What a bounded field is bounded to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Limits {
    /// The smallest it may be.
    pub min: u32,
    /// The largest it may be.
    pub max: u32,
    /// What it is when a row does not say.
    pub default: u32,
}

/// One field drawn from a closed set, and the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Closed {
    /// The field.
    pub field: &'static str,
    /// Every value it may hold.
    pub values: &'static [&'static str],
}

/// The points as they are published, with the identities each register already holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Published {
    /// The generation.
    pub extension_points_version: u32,
    /// Every point this build publishes.
    pub points: Vec<Occupied>,
}

/// One point, and what is already standing in its register.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Occupied {
    /// What a contribution declares `at`.
    pub name: &'static str,
    /// What a row here is.
    pub summary: &'static str,
    /// The register a row joins.
    pub register: &'static str,
    /// What already reads it.
    pub engine: &'static str,
    /// Exactly what a row carries.
    pub row: Row,
    /// The identities the bundled rows already hold, read out of the register.
    ///
    /// Published because naming what a contribution collided with takes knowing it.
    /// Two rules do the work between them and are deliberately belt and braces: a
    /// contributed identity must be namespaced with the declaring plugin's id and a
    /// bundled one never carries a colon, so a collision cannot be expressed — and a
    /// contribution naming one of these anyway is refused, naming both. The second
    /// exists because the first is a property of two naming conventions, and a rule
    /// that holds only while two conventions stay disjoint has an undefended edge.
    pub occupied: Vec<String>,
    /// The capability a manifest must ask for in order to contribute here.
    ///
    /// A contribution declares behaviour, and there is one answer for behaviour this
    /// build does not have: refuse by naming the capability. A build that read a row it
    /// could not run and dropped it would install a plugin whose declared behaviour is
    /// wider than its actual one — and the check that would have noticed a fault simply
    /// never runs, which is worse than a check that fails.
    ///
    /// Published on the point rather than written into the manifest contract, so it
    /// stays a fact about the register and a point added later brings its own
    /// capability with it.
    pub requires: &'static str,
}

/// The family a contributed check is narrowed to.
///
/// The nine the doctor recognises. Written here because the artefact publishes the
/// closed sets a row draws from, and held to the doctor's own enumeration by a test
/// in the crate that owns both — this one cannot see the register, and a second copy
/// nobody compares is a second copy that drifts.
const CATEGORIES: &[&str] = &[
    "environment",
    "storage",
    "network",
    "vpn",
    "credentials",
    "services",
    "providers",
    "queue",
    "config",
];

/// What a contributed check's timeout is bounded to.
///
/// Published as the value rather than as something to go and find, so the register
/// that runs a row is held to these bounds without a lookup — and a lookup that came
/// back with nothing cannot become a reason to fall back to some other number. The
/// point below is built from this, so there is one of it.
pub const CHECK_TIMEOUT: Limits = Limits {
    min: 1,
    max: 30,
    default: 10,
};

/// The capability a manifest asks for in order to contribute at either point.
///
/// One name for both, because what it stands for is the doctor reading rows it did not
/// ship — and a build that has that has it for checks and remedies together.
const CONTRIBUTE: &str = "doctor.contribute";

/// A check the doctor runs, alongside the bundled ones.
const DOCTOR_CHECK: Point = Point {
    name: "doctor.check",
    summary: "A check the doctor runs, alongside the bundled ones.",
    register: "the diagnostics register",
    engine: "the check engine — independent, bounded, four verdicts, a remedy on anything \
             that does not pass",
    row: Row {
        required: &[
            "id", "title", "category", "request", "expect", "why", "fixture",
        ],
        optional: &["timeout_s", "service"],
        bounds: &[Bounded {
            field: "timeout_s",
            limits: CHECK_TIMEOUT,
        }],
        enums: &[Closed {
            field: "category",
            values: CATEGORIES,
        }],
    },
    requires: CONTRIBUTE,
};

/// What to do about a contributed check that did not pass.
const DOCTOR_REMEDY: Point = Point {
    name: "doctor.remedy",
    summary: "What to do about a contributed check that did not pass.",
    register: "the remedies a finding carries",
    engine: "the error renderer — rendered, never executed",
    row: Row {
        required: &["id", "for", "action", "why"],
        optional: &["detail"],
        bounds: &[],
        enums: &[],
    },
    requires: CONTRIBUTE,
};

/// Every point this build publishes, in the order the artefact lists them.
pub(crate) const POINTS: &[Point] = &[DOCTOR_CHECK, DOCTOR_REMEDY];

/// The point a finding is contributed at.
///
/// Exposed because one rule is about these two points in particular — every check
/// carries a remedy, and a remedy names a check — and a rule holding its own copy of
/// either name is a rule a rename switches off without failing.
#[must_use]
pub const fn check() -> &'static str {
    DOCTOR_CHECK.name
}

/// The point the remedy for a finding is contributed at.
#[must_use]
pub const fn remedy() -> &'static str {
    DOCTOR_REMEDY.name
}

/// The categories a contributed check may be narrowed to.
///
/// Exposed so the register that owns them can be held to this list rather than this
/// list being taken on trust.
#[must_use]
pub fn categories() -> &'static [&'static str] {
    CATEGORIES
}

/// The points as they are published, against the identities the doctor already holds.
///
/// `occupied` is passed in rather than written here, for the reason `declared_by` is
/// read off the stack rather than restated: a bundled check that is renamed moves the
/// artefact rather than leaving a stale name a contribution could take.
///
/// A remedy's list is empty, and stays empty until a bundled remedy has an identity of
/// its own. Today a remedy is a field on the finding its check produced rather than a
/// row somebody can name, so there is nothing for a contribution to collide with —
/// which is a different claim from nobody having looked.
#[must_use]
pub fn published(occupied: &[&str]) -> Published {
    let mut held: Vec<String> = occupied.iter().map(|&check| check.to_owned()).collect();
    held.sort_unstable();

    Published {
        extension_points_version: EXTENSION_POINTS_VERSION,
        points: POINTS
            .iter()
            .map(|point| Occupied {
                name: point.name,
                summary: point.summary,
                register: point.register,
                engine: point.engine,
                row: point.row,
                occupied: if point.name == DOCTOR_CHECK.name {
                    held.clone()
                } else {
                    Vec::new()
                },
                requires: point.requires,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests;
