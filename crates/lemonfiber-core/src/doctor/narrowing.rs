//! What a run was narrowed to: the whole suite, one family, or one check.
//!
//! A check is named on the way in by the very id its finding carries on the way out,
//! so the thing an operator reads in a report is the thing they can ask for again. A
//! second name kept beside the first would be a second place for the two to disagree,
//! and the one that drifted would be the one nobody could run.
//!
//! A bundled id begins with the family the check belongs to — `storage.space` — which
//! is what resolves a named check to the checks worth running without a table of ids
//! kept anywhere.
//!
//! A contributed one does not, and cannot. It is namespaced with the plugin that
//! declared it — `comics:catalogue` — because what a plugin's row belongs to is the
//! plugin rather than a family, and that is the whole of what keeps it from taking a
//! bundled identity. So a name with no family in it is resolved by asking the checks
//! which of them reports against it, rather than by reading the name.

use super::{Category, Check, Finding};

/// What a run was narrowed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Narrowing {
    /// Every check there is.
    Suite,
    /// The checks of one family.
    Category(Category),
    /// One check, named by the id its findings carry.
    Check(String),
}

impl Narrowing {
    /// What an operator named, where it is something that can be run.
    ///
    /// A name that is neither a family nor a check inside one is `None` rather than a
    /// silent empty run, so a surface can tell the operator they mistyped rather than
    /// reporting that nothing was wrong.
    ///
    /// A contributed id is taken on its shape rather than on whether anything holds
    /// it, because which plugins are installed is not a fact this can see. A name
    /// nothing reports against then runs nothing and reports nothing — which is the
    /// honest answer about a plugin that is not installed, and a different answer from
    /// the refusal a name that could never be a check gets.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        if let Some(category) = Category::parse(name) {
            return Some(Self::Category(category));
        }
        if let Some((plugin, rest)) = name.split_once(':') {
            return (!plugin.is_empty() && !rest.is_empty()).then(|| Self::Check(name.to_owned()));
        }
        let (family, rest) = name.split_once('.')?;
        (Category::parse(family).is_some() && !rest.is_empty())
            .then(|| Self::Check(name.to_owned()))
    }

    /// The check this was narrowed to, where it was narrowed to one.
    #[must_use]
    pub fn check(&self) -> Option<&str> {
        match self {
            Self::Suite | Self::Category(_) => None,
            Self::Check(id) => Some(id),
        }
    }

    /// Whether this check is one this run reaches for.
    ///
    /// A named bundled check is resolved to its family, since a family is the smallest
    /// thing the suite holds as a value — one check in the list can report several
    /// findings, and which of those was wanted is settled once they exist.
    ///
    /// A name with no family in it is a contributed one, and the only thing that can
    /// say whether a check answers to it is the check. Asking every check is what the
    /// alternative — running the whole suite and keeping one finding — would have cost
    /// an operator who narrowed a run precisely so that it would not do that.
    ///
    /// The question asked of it is the one [`Self::keeps`] asks of a finding, so a
    /// check runs exactly when what it would report is a thing this run keeps. Two
    /// predicates would let a run drop the finding of a check it had just decided to
    /// run, which reads as the check having found nothing.
    pub(super) fn runs(&self, check: &dyn Check) -> bool {
        match self {
            Self::Suite => true,
            Self::Category(category) => *category == check.category(),
            Self::Check(id) => match id.split('.').next().and_then(Category::parse) {
                Some(family) => family == check.category(),
                None => check
                    .reports()
                    .is_some_and(|one| beneath(&one.check, id) || beneath(id, &one.check)),
            },
        }
    }

    /// Whether a finding is one that was asked for.
    ///
    /// Kept either way round the two ids sit. A check that could not run reports once
    /// under the family it belongs to, and dropping that would answer "no such check"
    /// about a check that had just been unable to answer; a check that reports one
    /// finding per service reports beneath its own name, and dropping those would
    /// answer the same about a check that had answered several times over.
    pub(super) fn keeps(&self, finding: &Finding) -> bool {
        match self {
            Self::Suite | Self::Category(_) => true,
            Self::Check(id) => beneath(id, &finding.check) || beneath(&finding.check, id),
        }
    }
}

/// Whether `id` is `root` itself or a name beneath it.
///
/// Bounded on the separator, so `vpn.tunnel-restored` is a check of its own rather than
/// something beneath `vpn.tunnel`.
fn beneath(id: &str, root: &str) -> bool {
    id.strip_prefix(root)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::Narrowing;
    use crate::doctor::{Category, Check, Finding, Reported, Verdict};

    /// A finding reporting under `check`.
    fn finding(check: &str) -> Finding {
        Finding::in_category(
            Category::Storage,
            check,
            "Storage",
            Verdict::Pass { note: None },
        )
    }

    /// A check of one family, which may or may not say what it reports against.
    struct Standing {
        category: Category,
        reports: Option<Reported>,
    }

    #[async_trait]
    impl Check for Standing {
        fn category(&self) -> Category {
            self.category
        }
        fn reports(&self) -> Option<Reported> {
            self.reports.clone()
        }
        async fn run(&self) -> Vec<Finding> {
            // What it says it reports against, so that the two predicates below can
            // be asked about the same check and the same finding. A stand-in that
            // answered nothing would make the agreement between them unassertable.
            self.reports.iter().map(|one| finding(&one.check)).collect()
        }
    }

    /// A check of a family that does not say what it reports against, as most do not.
    fn unnamed(category: Category) -> Standing {
        Standing {
            category,
            reports: None,
        }
    }

    /// A check that reports against exactly one identity, as a contributed row does.
    fn named(category: Category, check: &str) -> Standing {
        Standing {
            category,
            reports: Some(Reported {
                check: check.to_owned(),
                service: None,
                origin: crate::origin::Origin::Bundled,
            }),
        }
    }

    #[test]
    fn a_family_is_read_as_the_family_it_names() {
        assert_eq!(
            Narrowing::parse("storage"),
            Some(Narrowing::Category(Category::Storage))
        );
    }

    /// The id a finding carries is the id that can be asked for again.
    #[test]
    fn a_check_is_read_as_the_id_its_finding_carries() {
        let narrowing = Narrowing::parse("storage.space");
        assert_eq!(
            narrowing,
            Some(Narrowing::Check("storage.space".to_owned()))
        );
        assert_eq!(
            narrowing.as_ref().and_then(Narrowing::check),
            Some("storage.space")
        );
    }

    /// A name nothing could answer to is refused rather than run as an empty suite,
    /// which would report that nothing was wrong.
    #[test]
    fn a_name_belonging_to_no_family_is_not_a_narrowing() {
        assert_eq!(Narrowing::parse("nonsense"), None);
        assert_eq!(Narrowing::parse("nonsense.space"), None);
        assert_eq!(Narrowing::parse("storage."), None);
    }

    #[test]
    fn the_whole_suite_and_a_family_name_no_single_check() {
        assert_eq!(Narrowing::Suite.check(), None);
        assert_eq!(Narrowing::Category(Category::Storage).check(), None);
    }

    #[test]
    fn a_named_check_runs_the_family_it_belongs_to_and_no_other() {
        let narrowing = Narrowing::Check("storage.space".to_owned());
        assert!(narrowing.runs(&unnamed(Category::Storage)));
        assert!(!narrowing.runs(&unnamed(Category::Vpn)));
    }

    #[test]
    fn a_family_runs_its_own_checks_and_the_suite_runs_them_all() {
        assert!(Narrowing::Category(Category::Vpn).runs(&unnamed(Category::Vpn)));
        assert!(!Narrowing::Category(Category::Vpn).runs(&unnamed(Category::Storage)));
        assert!(Narrowing::Suite.runs(&unnamed(Category::Storage)));
    }

    /// A plugin's row is named by the plugin, and a run can be narrowed to one.
    ///
    /// The published points say a contributed check is what `doctor --only` takes, and
    /// a namespaced name reaching no narrowing at all made that untrue: the operator
    /// was told there is no such check, about one that had just been installed.
    #[test]
    fn a_contributed_id_is_read_as_the_check_it_names() {
        let narrowing = Narrowing::parse("comics:catalogue");
        assert_eq!(
            narrowing,
            Some(Narrowing::Check("comics:catalogue".to_owned()))
        );
        assert_eq!(
            narrowing.as_ref().and_then(Narrowing::check),
            Some("comics:catalogue")
        );
    }

    /// And a shape that is not one is still refused, so the acceptance is bounded.
    #[test]
    fn a_colon_with_nothing_on_one_side_of_it_is_not_a_narrowing() {
        assert_eq!(Narrowing::parse(":catalogue"), None);
        assert_eq!(Narrowing::parse("comics:"), None);
        assert_eq!(Narrowing::parse(":"), None);
    }

    /// The name carries no family, so the checks are asked which of them answers to it.
    ///
    /// Both directions. Running every check of every family would answer correctly and
    /// do the work the operator narrowed the run to avoid; running none would report
    /// that nothing was wrong about a check nobody ran.
    #[test]
    fn a_contributed_name_runs_the_check_that_reports_against_it_and_no_other() {
        let narrowing = Narrowing::Check("comics:catalogue".to_owned());
        assert!(narrowing.runs(&named(Category::Services, "comics:catalogue")));
        assert!(!narrowing.runs(&named(Category::Services, "comics:identity")));
        assert!(!narrowing.runs(&unnamed(Category::Services)));
        assert!(!narrowing.runs(&unnamed(Category::Storage)));
    }

    /// A contributed check is not reached by narrowing to a bundled family's name.
    ///
    /// The direction that would be a collision rather than a miss: a plugin's row
    /// answering to `storage.space` would be two checks under one name.
    #[test]
    fn a_bundled_name_does_not_reach_a_contributed_check() {
        let narrowing = Narrowing::Check("storage.space".to_owned());
        assert!(!narrowing.runs(&named(Category::Vpn, "comics:catalogue")));
        // And a contributed check of that very family is run, because narrowing by a
        // bundled name is narrowing by family and the family is the smallest unit.
        assert!(narrowing.runs(&named(Category::Storage, "comics:catalogue")));
    }

    /// What a check this run reaches would report is a thing this run keeps.
    ///
    /// The claim the two predicates make about each other, asserted rather than left
    /// in prose. Two that disagreed would let a run drop the finding of a check it had
    /// just decided to run, and on the page that reads as the check having found
    /// nothing rather than as a narrowing that contradicted itself.
    #[tokio::test]
    async fn a_check_this_run_reaches_reports_a_finding_this_run_keeps() {
        let narrowing = Narrowing::Check("comics:catalogue".to_owned());
        let check = named(Category::Services, "comics:catalogue");
        assert!(narrowing.runs(&check));
        let found = check.run().await;
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found.iter().all(|one| narrowing.keeps(one)), "{found:?}");
    }

    /// A contributed finding is kept by the name it was asked for.
    #[test]
    fn a_contributed_name_keeps_its_own_finding_and_no_neighbour() {
        let narrowing = Narrowing::Check("comics:catalogue".to_owned());
        assert!(narrowing.keeps(&finding("comics:catalogue")));
        assert!(!narrowing.keeps(&finding("comics:identity")));
    }

    #[test]
    fn a_family_and_the_suite_keep_everything_that_ran() {
        assert!(Narrowing::Suite.keeps(&finding("storage.space")));
        assert!(Narrowing::Category(Category::Storage).keeps(&finding("storage.space")));
    }

    #[test]
    fn a_named_check_keeps_its_own_finding_and_no_neighbour() {
        let narrowing = Narrowing::Check("storage.space".to_owned());
        assert!(narrowing.keeps(&finding("storage.space")));
        assert!(!narrowing.keeps(&finding("storage.hardlinks")));
    }

    /// A check that could not run reports once, under its family, and that answer is
    /// the answer for anything it would have said.
    #[test]
    fn a_named_check_keeps_the_answer_given_for_the_whole_family() {
        let narrowing = Narrowing::Check("storage.space".to_owned());
        assert!(narrowing.keeps(&finding("storage")));
        assert!(!narrowing.keeps(&finding("storage.space-headroom")));
    }

    /// A check reporting once per service reports beneath its own name.
    #[test]
    fn a_named_check_keeps_what_it_reported_for_each_service() {
        let narrowing = Narrowing::Check("services.releases".to_owned());
        assert!(narrowing.keeps(&finding("services.releases.sonarr")));
        assert!(!narrowing.keeps(&finding("services.quality-guides")));
    }
}
