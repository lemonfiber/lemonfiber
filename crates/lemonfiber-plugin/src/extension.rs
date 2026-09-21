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
pub const POINTS: &[Point] = &[DOCTOR_CHECK, DOCTOR_REMEDY];

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
mod tests {
    use std::collections::BTreeSet;

    use super::{categories, published, EXTENSION_POINTS_VERSION, POINTS};

    #[test]
    fn it_publishes_the_two_points_a_plugin_may_contribute_at() {
        let named: Vec<&str> = POINTS.iter().map(|point| point.name).collect();
        assert_eq!(named, vec!["doctor.check", "doctor.remedy"]);
    }

    /// The two a rule elsewhere is about are the two this publishes, by identity rather
    /// than by a name written down twice.
    #[test]
    fn the_two_points_a_rule_names_are_the_two_that_are_published() {
        assert!(POINTS.iter().any(|point| point.name == super::check()));
        assert!(POINTS.iter().any(|point| point.name == super::remedy()));
        assert_ne!(super::check(), super::remedy());
    }

    #[test]
    fn a_check_is_bounded_here_rather_than_by_the_plugin_s_opinion() {
        let bounds = POINTS
            .iter()
            .find(|point| point.name == "doctor.check")
            .and_then(|point| point.row.bounds.first())
            .map(|bound| {
                (
                    bound.field,
                    bound.limits.min,
                    bound.limits.max,
                    bound.limits.default,
                )
            });
        assert_eq!(bounds, Some(("timeout_s", 1, 30, 10)));
    }

    /// The bound the register is held to is the bound the artefact publishes.
    ///
    /// One value under two names would be two numbers to keep in step, and the one
    /// that fell behind would be the one actually bounding a run — leaving the
    /// published document describing a limit nothing applies.
    #[test]
    fn the_published_bound_and_the_one_a_runner_reads_are_the_same_value() {
        let published = POINTS
            .iter()
            .find(|point| point.name == super::check())
            .and_then(|point| point.row.bounds.first())
            .map(|bound| bound.limits);
        assert_eq!(published, Some(super::CHECK_TIMEOUT));
    }

    #[test]
    fn a_check_is_narrowed_to_one_of_the_nine_families_the_doctor_recognises() {
        let values = POINTS
            .iter()
            .find(|point| point.name == "doctor.check")
            .and_then(|point| point.row.enums.first())
            .map(|closed| (closed.field, closed.values.len()));
        assert_eq!(values, Some(("category", 9)));
        assert_eq!(categories().len(), 9);
    }

    /// A check asks the plugin's own service and nothing else.
    ///
    /// There is no field for a host, so a check cannot be pointed at another service,
    /// at the machine, or off it — and the way to keep that true is for the row to
    /// name no such field rather than for something downstream to refuse one.
    #[test]
    fn a_contributed_check_has_no_field_by_which_it_could_name_another_host() {
        let reaching: Vec<&str> = POINTS
            .iter()
            .flat_map(|point| point.row.required.iter().chain(point.row.optional))
            .copied()
            .filter(|field| matches!(*field, "host" | "url" | "to" | "address" | "machine"))
            .collect();
        assert!(reaching.is_empty(), "a row could name one: {reaching:?}");
    }

    #[test]
    fn the_identities_the_doctor_holds_are_published_against_the_check_point() {
        let artefact = published(&["storage.space", "environment.engine"]);
        let held: Vec<(&str, Vec<String>)> = artefact
            .points
            .into_iter()
            .map(|point| (point.name, point.occupied))
            .collect();
        assert_eq!(
            held,
            vec![
                (
                    "doctor.check",
                    vec!["environment.engine".to_owned(), "storage.space".to_owned()]
                ),
                ("doctor.remedy", Vec::new()),
            ]
        );
        assert_eq!(
            published(&[]).extension_points_version,
            EXTENSION_POINTS_VERSION
        );
    }

    /// Every point names the capability a contribution there asks for.
    ///
    /// A contribution declares behaviour, and a build that read one it could not run
    /// and dropped it would install a plugin whose declared behaviour is wider than its
    /// actual one — so the name is published rather than left to an author to remember.
    #[test]
    fn every_point_names_the_capability_a_contribution_there_asks_for() {
        let asked: Vec<(&str, &str)> = published(&[])
            .points
            .into_iter()
            .map(|point| (point.name, point.requires))
            .collect();
        assert_eq!(
            asked,
            vec![
                ("doctor.check", "doctor.contribute"),
                ("doctor.remedy", "doctor.contribute"),
            ]
        );
    }

    /// Each table is keyed by the field it is about, and no field is in one twice.
    ///
    /// The artefact keys both by field name, so a field written twice would produce a
    /// document whose second entry silently replaced the first — which is a rule the
    /// map form makes unnecessary for a reader and not for whoever writes the table.
    #[test]
    fn no_field_carries_two_bounds_or_two_closed_sets() {
        for point in POINTS {
            let bounded: BTreeSet<&str> = point.row.bounds.iter().map(|one| one.field).collect();
            let closed: BTreeSet<&str> = point.row.enums.iter().map(|one| one.field).collect();
            assert_eq!(bounded.len(), point.row.bounds.len(), "{}", point.name);
            assert_eq!(closed.len(), point.row.enums.len(), "{}", point.name);
        }
    }

    /// Both tables reach the artefact keyed by field, which is the shape it publishes.
    #[test]
    fn the_bounds_and_the_closed_sets_are_published_as_tables() {
        let text = published(&[])
            .points
            .iter()
            .filter_map(|point| serde_json::to_string(point).ok())
            .collect::<String>();
        assert!(text.contains(r#""bounds":{"timeout_s":{"#), "got: {text}");
        assert!(text.contains(r#""enums":{"category":["#), "got: {text}");
    }

    /// A row that declares no bound and no closed set says nothing about either.
    ///
    /// The remedy's row has neither, and an artefact carrying two empty tables would be
    /// publishing a shape nothing has — which a generator reading it would then have to
    /// decide was meaningful.
    #[test]
    fn a_row_with_no_bounds_and_no_closed_sets_publishes_neither() {
        let text = published(&[])
            .points
            .iter()
            .filter(|point| point.name == "doctor.remedy")
            .filter_map(|point| serde_json::to_string(point).ok())
            .collect::<String>();
        assert!(!text.contains("bounds"), "got: {text}");
        assert!(!text.contains("enums"), "got: {text}");
    }
}
