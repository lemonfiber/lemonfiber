//! The same question as the hardlink probe, asked of the container's view.
//!
//! The probe beside this one creates a file under the data location, links it, and
//! reads back how many names point at the one file. That is proof, and it is proof
//! about the host. Inside a container the data location is whatever the compose files
//! mounted there, and a bind mount *is* a filesystem boundary — so a stack that mounts
//! the downloads at one path and the library at another has put them on opposite sides
//! of one, where nothing can be linked. The probe passes, every import copies, and the
//! two facts never meet unless something says so.
//!
//! Which is why it is reported here rather than left to a check of its own: an operator
//! reading that links work has been told half an answer, and the half they were not
//! told is the half that decides whether an import takes milliseconds or minutes. Both
//! halves arrive together, under the one heading they are both about.
//!
//! It reports and never refuses. The stack lemonfiber ships is held to the rule before
//! anything runs — one that broke it would be a broken build, and nobody using it could
//! do a thing about that — but a stack directory the operator pointed lemonfiber at is
//! theirs, they laid it out, and a rule of lemonfiber's is guidance there. Refusing to
//! operate it would make this tool the thing standing between an operator and their own
//! system over a cost that is theirs to carry.
//!
//! So what is owed is the consequence, in the terms they will feel it in — the same
//! sentence a location that cannot hardlink is given, because it is the same outcome
//! arriving by a different road — and a way to say they have weighed it. A warning is
//! answerable, so `lemonfiber doctor --accept storage.single-mount` settles it once and
//! it stops leading afterwards, the way running torrents with no tunnel does.

use super::{finding, Code, Finding, Problem, Remedy, Severity, State, Verdict};
use crate::stack::mounts::Crowded;
use crate::storage::COPY_CONSEQUENCE;

/// Raised where a service would see more than one mount beneath the data location,
/// so anything imported between them is copied rather than linked.
pub const SPLIT_MOUNTS: Code = Code::new("STORAGE-7");

/// The name these findings are given.
const CHECK: &str = "storage.single-mount";

/// What they are called on a report.
const TITLE: &str = "One mount beneath the data location";

/// What the stack's own compose files give each service beneath the data location.
///
/// One finding per crowded service rather than one for the stack, because the cost
/// lands per service: a fork that splits the mounts for the television library and not
/// for the film one is a fork where half the imports are instant. They share a name, so
/// answering the choice answers it for the layout rather than service by service —
/// which is how the layout was decided in the first place.
pub(super) fn findings(crowded: &[Crowded]) -> Vec<Finding> {
    if crowded.is_empty() {
        return vec![kept()];
    }
    crowded.iter().map(split).collect()
}

/// What is said where one service would see more than one mount beneath the data
/// location.
fn split(crowded: &Crowded) -> Finding {
    let problem = Problem::new(
        SPLIT_MOUNTS,
        Severity::Warning,
        format!(
            "Imports into {} will copy rather than link",
            crowded.service
        ),
        format!(
            "This stack gives {} {} separate mounts beneath the data location, and inside the \
             container each of those is its own filesystem. A file moved from one to another \
             cannot be linked between them. {COPY_CONSEQUENCE}",
            crowded.service,
            crowded.mounts.len()
        ),
        Remedy::new(
            "Mount the data location once and keep the downloads and the library as \
             directories beneath it",
        )
        .with_detail("one volume entry, `${DATA_ROOT}:/data`, in place of the ones below"),
    )
    .or_try(
        Remedy::new(
            "Or keep the layout as it is; lemonfiber goes on operating this stack either way",
        )
        .with_detail(format!(
            "where this is deliberate: lemonfiber doctor --accept {CHECK}"
        )),
    )
    .in_state(State::Guided)
    .with_detail(crowded.mounts.join("\n"));
    finding(CHECK, TITLE, Verdict::Warn(problem)).about(&crowded.service)
}

/// No service in this stack would see the data location as more than one place.
///
/// A pass rather than a silence, because this is the half of the hardlink question no
/// probe can answer: an operator reading that links work on this machine would
/// otherwise have no way to tell whether that had been established for the containers
/// as well.
///
/// Said as what the files hold rather than as a property of every service, because that
/// is what was read. A compose file this could not parse contributes nothing, so a
/// claim about services would be a claim about services it may never have seen.
fn kept() -> Finding {
    finding(
        CHECK,
        TITLE,
        Verdict::Pass {
            note: Some(
                "nothing in this stack's compose files gives a service two mounts beneath the \
                 data location, so imports inside the containers link rather than copy"
                    .to_owned(),
            ),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{findings, CHECK};
    use crate::doctor::{Finding, Verdict};
    use crate::stack::mounts::Crowded;

    /// A service that would see the given mounts beneath the data location.
    fn crowding(service: &str, mounts: &[&str]) -> Crowded {
        Crowded {
            service: service.to_owned(),
            mounts: mounts.iter().map(|mount| (*mount).to_owned()).collect(),
        }
    }

    /// The service whose downloads and library are on opposite sides of a boundary.
    fn sonarr() -> Crowded {
        crowding(
            "sonarr",
            &[
                "${DATA_ROOT}/downloads:/downloads",
                "${DATA_ROOT}/media:/media",
            ],
        )
    }

    /// Every word these findings carry — the summary, what it means, each remedy and
    /// its detail, and the entries underneath.
    ///
    /// Gathered whole rather than picked out of the verdict, because a match would want
    /// an arm for each of the five verdicts and these are two of them. The other three
    /// would be lines no run here enters, which the coverage gate counts.
    fn said(findings: &[Finding]) -> String {
        format!("{findings:?}")
    }

    #[test]
    fn a_split_data_location_is_reported_rather_than_refused() {
        // The whole point: the stack still runs, and the operator is told what it will
        // cost them.
        let found = findings(&[sonarr()]);
        assert!(
            found
                .iter()
                .all(|finding| matches!(finding.verdict, Verdict::Warn(_))),
            "{found:?}"
        );
    }

    #[test]
    fn what_it_costs_is_named_rather_than_the_rule_it_broke() {
        // "More than one mount beneath the data location" means nothing to most
        // operators. Minutes instead of instants, twice the disk, and nothing left to
        // seed from is what they will actually meet.
        let words = said(&findings(&[sonarr()]));
        assert!(words.contains("copy rather than link"), "{words}");
        assert!(words.contains("twice the disk"), "{words}");
        assert!(words.contains("seed"), "{words}");
    }

    #[test]
    fn the_mounts_it_found_are_shown_rather_than_counted() {
        // A count is not something an operator can act on. The entries are what they
        // will go and edit, so the finding carries them.
        let words = said(&findings(&[sonarr()]));
        assert!(
            words.contains("${DATA_ROOT}/downloads:/downloads"),
            "{words}"
        );
        assert!(words.contains("${DATA_ROOT}/media:/media"), "{words}");
    }

    #[test]
    fn the_choice_can_be_answered_by_the_name_the_finding_carries() {
        // The id offered to `--accept` is the id the finding reports under; a second
        // name written into the sentence would be the one that drifts, and the one
        // nobody could answer with.
        let found = findings(&[sonarr()]);
        let words = said(&found);
        assert!(words.contains(&format!("--accept {CHECK}")), "{words}");
        assert_eq!(
            found.first().map(|finding| finding.check.as_str()),
            Some(CHECK)
        );
    }

    #[test]
    fn each_service_is_reported_as_its_own() {
        // A fork can split the mounts for one service and not for another, and each
        // finding is attributed so the report reads as being about that service.
        let found = findings(&[
            sonarr(),
            crowding("radarr", &["${DATA_ROOT}/a:/a", "${DATA_ROOT}/b:/b"]),
        ]);
        let about: Vec<Option<String>> = found
            .iter()
            .map(|finding| finding.service.clone())
            .collect();
        assert_eq!(
            about,
            vec![Some("sonarr".to_owned()), Some("radarr".to_owned())]
        );
    }

    #[test]
    fn a_stack_that_mounts_it_once_is_told_so_rather_than_left_silent() {
        // The half no probe can answer. That links work on this machine says nothing
        // about what the containers can do, and an operator has no way to tell the two
        // apart unless this says it.
        let found = findings(&[]);
        assert_eq!(found.len(), 1, "{found:?}");
        let words = said(&found);
        assert!(words.contains("link rather than copy"), "{words}");
        assert!(
            found
                .iter()
                .all(|finding| matches!(finding.verdict, Verdict::Pass { .. })),
            "{found:?}"
        );
    }
}
