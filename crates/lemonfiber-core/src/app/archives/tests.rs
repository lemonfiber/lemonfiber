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
