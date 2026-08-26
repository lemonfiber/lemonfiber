//! What this machine has kept, so an archive can be picked rather than remembered.
//!
//! The half of a restore that comes before naming one. A surface with a filesystem
//! in front of it lists the backups directory for itself; a surface without one has
//! no way to find out what it may name, and a name that cannot be discovered is a
//! request that cannot be made. So the listing is a command like any other and both
//! surfaces read the same one, which is what stops a browser carrying its own idea
//! of what is on this disk.
//!
//! Names and nothing else, in the order they were taken. What an archive *holds* is
//! its own manifest's account of itself, and naming one to a restore answers with
//! that account having touched nothing — so a listing that opened every archive to
//! describe it would be doing the next request's work as well as its own, on every
//! archive rather than on the one that was chosen.

use serde::Serialize;

use crate::archive::Fault;
use crate::error::{Code, Problem, Remedy, Severity, State};

use super::Ctx;

/// Raised when this run has nowhere it knows to look for archives.
pub const NOWHERE_KEPT: Code = Code::new("BACKUP-6");

/// Raised when the directory the archives are kept in could not be read.
pub const NOT_LISTED: Code = Code::new("BACKUP-7");

/// The archives this machine has kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Listing {
    /// Each one by the name it was written under, newest first.
    ///
    /// The name is the whole of what another surface needs: it is what a restore
    /// asks for, and it carries the moment the archive was taken and what it
    /// covers, because that is how a capture names one.
    pub archives: Vec<String>,
}

/// List the archives this machine has kept, newest first.
///
/// # Errors
///
/// Returns a [`Problem`] where this run has nowhere it keeps archives, or where
/// the directory it keeps them in could not be read.
pub async fn run(ctx: &Ctx) -> Result<Listing, Box<Problem>> {
    let archiving = ctx.archives.as_ref().ok_or_else(|| Box::new(nowhere()))?;
    let mut existing = archiving
        .vault
        .existing(&archiving.paths.backups())
        .await
        .map_err(|fault| Box::new(not_listed(&fault)))?;

    // Newest first, which is the opposite of the order retention wants and the
    // order a person wants: the archive most often restored is the one taken last.
    // Ties fall back to the name so that two archives written within one second of
    // each other are listed the same way twice running.
    existing.sort_by(|one, other| {
        other
            .created_at
            .cmp(&one.created_at)
            .then_with(|| one.name.cmp(&other.name))
    });
    Ok(Listing {
        archives: existing.into_iter().map(|found| found.name).collect(),
    })
}

/// The refusal for a run that cannot say where its own files go.
fn nowhere() -> Problem {
    Problem::new(
        NOWHERE_KEPT,
        Severity::Error,
        "This run has nowhere it knows to look for backups",
        "Backups are kept in lemonfiber's own directory, and this machine would not say where \
         that is — so there is nowhere to read a list of them from.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}

/// The refusal for a backups directory that would not be read.
fn not_listed(fault: &Fault) -> Problem {
    Problem::new(
        NOT_LISTED,
        Severity::Error,
        "The backups kept here could not be listed",
        "The directory lemonfiber keeps backups in would not be read, so what is in it is not \
         known. Nothing was touched.",
        Remedy::new("Check the backups directory is readable and ask again"),
    )
    .in_state(State::Guided)
    .with_detail(fault.message.clone())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{run, Listing, NOT_LISTED, NOWHERE_KEPT};
    use crate::app::fixtures::FakeArchive;
    use crate::test_support::a_context;

    /// A run that keeps the archives a fake holds.
    fn keeping(vault: &Arc<FakeArchive>) -> crate::app::Ctx {
        crate::app::fixtures::keeping(a_context().build(), vault)
    }

    #[tokio::test]
    async fn a_run_with_nowhere_to_look_says_so_rather_than_listing_none() {
        // Absent archives and an empty listing are different answers: one is "this
        // machine keeps none" and the other is "this run cannot tell".
        let listed = run(&a_context().build()).await;
        assert_eq!(
            listed.err().map(|problem| problem.code),
            Some(NOWHERE_KEPT),
            "a run that cannot say where its files go refuses rather than answering"
        );
    }

    #[tokio::test]
    async fn a_directory_that_will_not_be_read_is_a_refusal_and_not_an_empty_list() {
        let vault = Arc::new(FakeArchive::unlistable());
        let listed = run(&keeping(&vault)).await;
        let problem = listed.err();
        assert_eq!(
            problem.as_ref().map(|problem| problem.code),
            Some(NOT_LISTED)
        );
        assert_eq!(
            problem.and_then(|problem| problem.detail),
            Some("permission denied".to_owned()),
            "the platform's own words are carried through"
        );
    }

    #[tokio::test]
    async fn the_archives_are_listed_newest_first() {
        let vault = Arc::new(FakeArchive::keeping_backups(&[
            ("lemonfiber-full-2.tar.gz", "00000000000000000002"),
            ("lemonfiber-full-1.tar.gz", "00000000000000000001"),
            ("lemonfiber-full-3.tar.gz", "00000000000000000003"),
        ]));
        let listed = run(&keeping(&vault)).await.ok();
        assert_eq!(
            listed,
            Some(Listing {
                archives: vec![
                    "lemonfiber-full-3.tar.gz".to_owned(),
                    "lemonfiber-full-2.tar.gz".to_owned(),
                    "lemonfiber-full-1.tar.gz".to_owned(),
                ]
            })
        );
    }

    #[tokio::test]
    async fn two_archives_taken_in_the_same_second_are_listed_the_same_way_twice() {
        let vault = Arc::new(FakeArchive::keeping_backups(&[
            ("lemonfiber-sonarr-1.tar.gz", "00000000000000000001"),
            ("lemonfiber-full-1.tar.gz", "00000000000000000001"),
        ]));
        let listed = run(&keeping(&vault)).await.ok();
        assert_eq!(
            listed.map(|listing| listing.archives),
            Some(vec![
                "lemonfiber-full-1.tar.gz".to_owned(),
                "lemonfiber-sonarr-1.tar.gz".to_owned(),
            ])
        );
    }

    #[tokio::test]
    async fn a_machine_that_has_kept_nothing_says_it_has_kept_nothing() {
        let vault = Arc::new(FakeArchive::roomy());
        let listed = run(&keeping(&vault)).await.ok();
        assert_eq!(
            listed,
            Some(Listing {
                archives: Vec::new()
            }),
            "an empty list is an answer, not a refusal"
        );
    }
}
