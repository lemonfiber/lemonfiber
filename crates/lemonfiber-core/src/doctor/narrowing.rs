//! What a run was narrowed to: the whole suite, one family, or one check.
//!
//! A check is named on the way in by the very id its finding carries on the way out,
//! so the thing an operator reads in a report is the thing they can ask for again. A
//! second name kept beside the first would be a second place for the two to disagree,
//! and the one that drifted would be the one nobody could run.
//!
//! An id begins with the family the check belongs to — `storage.space` — which is what
//! resolves a named check to the checks worth running without a table of ids kept
//! anywhere.

use super::{Category, Finding};

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
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        if let Some(category) = Category::parse(name) {
            return Some(Self::Category(category));
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

    /// Whether a check of this family is one this run reaches for.
    ///
    /// A named check is resolved to its family, since a family is the smallest thing
    /// the suite holds as a value — one check in the list can report several findings,
    /// and which of those was wanted is settled once they exist.
    pub(super) fn runs(&self, family: Category) -> bool {
        match self {
            Self::Suite => true,
            Self::Category(category) => *category == family,
            Self::Check(id) => id.split('.').next().and_then(Category::parse) == Some(family),
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
    use super::Narrowing;
    use crate::doctor::{Category, Finding, Verdict};

    /// A finding reporting under `check`.
    fn finding(check: &str) -> Finding {
        Finding::in_category(
            Category::Storage,
            check,
            "Storage",
            Verdict::Pass { note: None },
        )
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
        assert!(narrowing.runs(Category::Storage));
        assert!(!narrowing.runs(Category::Vpn));
    }

    #[test]
    fn a_family_runs_its_own_checks_and_the_suite_runs_them_all() {
        assert!(Narrowing::Category(Category::Vpn).runs(Category::Vpn));
        assert!(!Narrowing::Category(Category::Vpn).runs(Category::Storage));
        assert!(Narrowing::Suite.runs(Category::Storage));
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
