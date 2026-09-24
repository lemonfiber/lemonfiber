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
mod tests;
