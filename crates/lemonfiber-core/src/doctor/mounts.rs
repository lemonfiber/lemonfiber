//! What the stack's own files give each service beneath the data location.
//!
//! The storage check beside this one creates a file under the data location, links
//! it, and reads back how many names point at the one file. That is proof, and it is
//! proof about the host. Inside a container the data location is whatever the compose
//! files mounted there, and a bind mount *is* a filesystem boundary — so a stack that
//! mounts the downloads at one path and the library at another has put them on
//! opposite sides of one, where nothing can be linked. The host probe passes, every
//! import copies, and the two facts never meet. This is the only check in the suite
//! that establishes something no experiment on this machine could reach.
//!
//! It reports and never refuses, which is the whole reason it exists as a check
//! rather than as a line in the stack's validation. The stack lemonfiber ships is
//! held to the rule before anything runs — one that broke it would be a broken build,
//! and nobody using it could do a thing about that. A stack directory the operator
//! pointed lemonfiber at is the opposite: it is theirs, they laid it out, and a rule
//! of lemonfiber's is guidance there. Refusing to operate it would make this tool the
//! thing standing between an operator and their own system over a cost that is theirs
//! to carry.
//!
//! So what is owed is the consequence, in the terms they will feel it in — the same
//! sentence a location that cannot hardlink is given, because it is the same
//! outcome arriving by a different road — and a way to say they have weighed it. A
//! warning is answerable, so `lemonfiber doctor --accept storage.single-mount` settles
//! it once and it stops leading afterwards, the way running torrents with no tunnel
//! does.
//!
//! See `.docs/architecture/module-layout.md`.

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::stack::mounts::Crowded;
use crate::storage::COPY_CONSEQUENCE;

/// Raised where a service would see more than one mount beneath the data location,
/// so anything imported between them is copied rather than linked.
pub const SPLIT_MOUNTS: Code = Code::new("STORAGE-7");

/// The name this check's findings are given.
const CHECK: &str = "storage.single-mount";

/// What it is called on a report.
const TITLE: &str = "One mount beneath the data location";

/// Whether every service sees the data location as one place.
pub struct MountsCheck {
    /// The services that would see more than one mount beneath it, and which mounts
    /// those are.
    crowded: Vec<Crowded>,
}

impl MountsCheck {
    /// A check over what reading this stack's compose files found.
    ///
    /// The reading is done by the caller rather than here, the way the wiring and
    /// household checks take what was resolved for them: a check is a value in a
    /// list, and one that went and read a directory of its own would be the only
    /// member of that list holding a stack as well as an answer.
    #[must_use]
    pub fn new(crowded: Vec<Crowded>) -> Self {
        Self { crowded }
    }
}

#[async_trait]
impl Check for MountsCheck {
    fn category(&self) -> Category {
        Category::Storage
    }

    async fn run(&self) -> Vec<Finding> {
        if self.crowded.is_empty() {
            return vec![kept()];
        }
        self.crowded.iter().map(split).collect()
    }
}

/// What is said where one service would see more than one mount beneath the data
/// location.
///
/// One finding per service rather than one for the stack, because the cost lands per
/// service: a fork that splits the mounts for the television library and not for the
/// film one is a fork where half the imports are instant. They share a name, so
/// answering the choice answers it for the layout rather than service by service —
/// which is how the layout was decided in the first place.
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
    Finding::in_category(Category::Storage, CHECK, TITLE, Verdict::Warn(problem))
        .about(&crowded.service)
}

/// No service in this stack would see the data location as more than one place.
///
/// A pass rather than a silence, because this is the half of the hardlink question a
/// probe cannot answer: an operator reading a storage section that says links work
/// would otherwise have no way to tell whether that had been established for the
/// containers as well as for the host.
///
/// Said as what the files hold rather than as a property of every service, because
/// that is what was read. A compose file this could not parse contributes nothing, so
/// a claim about services would be a claim about services it may never have seen.
fn kept() -> Finding {
    Finding::in_category(
        Category::Storage,
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
    use super::{MountsCheck, CHECK};
    use crate::doctor::{Category, Check, Finding, Verdict};
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

    /// What the check reports, given what reading the compose files found.
    async fn found(crowded: Vec<Crowded>) -> Vec<Finding> {
        MountsCheck::new(crowded).run().await
    }

    /// Every word these findings carry — the summary, what it means, each remedy and
    /// its detail, and the entries underneath.
    ///
    /// Gathered whole rather than picked out of the verdict, because a match would
    /// want an arm for each of the five and this check produces two. The other three
    /// would be lines no run here enters, which is what the coverage gate is for.
    fn said(findings: &[Finding]) -> String {
        format!("{findings:?}")
    }

    #[tokio::test]
    async fn a_split_data_location_is_reported_rather_than_refused() {
        // The whole point of the check: the stack still runs, and the operator is
        // told what it will cost them.
        let findings = found(vec![sonarr()]).await;
        assert!(
            findings
                .iter()
                .all(|finding| matches!(finding.verdict, Verdict::Warn(_))),
            "{findings:?}"
        );
    }

    #[tokio::test]
    async fn what_it_costs_is_named_rather_than_the_rule_it_broke() {
        // "More than one mount beneath the data location" means nothing to most
        // operators. Minutes instead of instants, twice the disk, and nothing left
        // to seed from is what they will actually meet.
        let words = said(&found(vec![sonarr()]).await);
        assert!(words.contains("copy rather than link"), "{words}");
        assert!(words.contains("twice the disk"), "{words}");
        assert!(words.contains("seed"), "{words}");
    }

    #[tokio::test]
    async fn the_mounts_it_found_are_shown_rather_than_counted() {
        // A count is not something an operator can act on. The entries are what they
        // will go and edit, so the finding carries them.
        let words = said(&found(vec![sonarr()]).await);
        assert!(
            words.contains("${DATA_ROOT}/downloads:/downloads"),
            "{words}"
        );
        assert!(words.contains("${DATA_ROOT}/media:/media"), "{words}");
    }

    #[tokio::test]
    async fn the_choice_can_be_answered_by_the_name_the_finding_carries() {
        // The id offered to `--accept` is the id the finding reports under; a second
        // name written into the sentence would be the one that drifts, and the one
        // nobody could answer with.
        let findings = found(vec![sonarr()]).await;
        let words = said(&findings);
        assert!(words.contains(&format!("--accept {CHECK}")), "{words}");
        assert_eq!(
            findings.first().map(|finding| finding.check.as_str()),
            Some(CHECK)
        );
    }

    #[tokio::test]
    async fn each_service_is_reported_as_its_own() {
        // A fork can split the mounts for one service and not for another, and the
        // finding is attributed so the report reads as being about that service.
        let findings = found(vec![
            sonarr(),
            crowding("radarr", &["${DATA_ROOT}/a:/a", "${DATA_ROOT}/b:/b"]),
        ])
        .await;
        let about: Vec<Option<String>> = findings
            .iter()
            .map(|finding| finding.service.clone())
            .collect();
        assert_eq!(
            about,
            vec![Some("sonarr".to_owned()), Some("radarr".to_owned())]
        );
    }

    #[tokio::test]
    async fn a_stack_that_mounts_it_once_is_told_so_rather_than_left_silent() {
        // The half a probe cannot answer. A storage section saying links work on the
        // host says nothing about what the containers can do, and an operator has no
        // way to tell the two apart unless this says it.
        let findings = found(Vec::new()).await;
        assert_eq!(findings.len(), 1, "{findings:?}");
        let words = said(&findings);
        assert!(words.contains("link rather than copy"), "{words}");
        assert!(
            findings
                .iter()
                .all(|finding| matches!(finding.verdict, Verdict::Pass { .. })),
            "{findings:?}"
        );
    }

    #[test]
    fn the_check_belongs_to_the_family_that_answers_for_hardlinks() {
        // Narrowing a run to `storage` is asking whether imports will be instant, and
        // this is half of that answer.
        assert_eq!(MountsCheck::new(Vec::new()).category(), Category::Storage);
    }
}
