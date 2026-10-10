use std::path::Path;

use lemonfiber_sidecar::decline::TokenHash;

use super::{path, table, with_token};
use crate::invitation::{Offer, Offers};
use crate::ports::service::Member;

fn member(id: &str, name: &str, claimed: bool) -> Member {
    Member {
        id: id.to_owned(),
        name: name.to_owned(),
        claimed,
        ..Member::default()
    }
}

fn offer(decline: Option<&str>) -> Offer {
    Offer {
        offered: "2026-10-03T10:00:00Z".to_owned(),
        lapses: "2026-10-05T10:00:00Z".to_owned(),
        decline: decline.map(TokenHash::of),
        claim: None,
    }
}

#[test]
fn the_table_holds_each_unclaimed_offer_that_carries_a_token() {
    let offers: Offers = [
        ("9".to_owned(), offer(Some("ana-token"))),
        ("10".to_owned(), offer(None)),
        ("11".to_owned(), offer(Some("bo-token"))),
        ("12".to_owned(), offer(Some("gone-token"))),
    ]
    .into_iter()
    .collect();
    let household = [
        member("9", "ana", false),
        member("10", "cy", false),
        member("11", "bo", true),
    ];

    let table = table(&offers, &household);

    assert_eq!(table.invitations.len(), 1);
    let ana = table.find(&TokenHash::of("ana-token"));
    assert_eq!(
        ana.map(|one| (one.account.as_str(), one.name.as_str())),
        Some(("9", "ana"))
    );
    assert_eq!(
        ana.map(|one| (one.issued, one.lapses)),
        Some((1_791_021_600, 1_791_194_400))
    );
}

#[test]
fn an_offer_whose_dates_cannot_be_read_is_left_out() {
    let mut unreadable = offer(Some("ana-token"));
    unreadable.offered = "yesterday".to_owned();
    let offers: Offers = [("9".to_owned(), unreadable)].into_iter().collect();

    assert!(table(&offers, &[member("9", "ana", false)])
        .invitations
        .is_empty());
}

#[test]
fn a_token_is_recorded_as_its_hash_on_an_offer_already_recorded() {
    let offers: Offers = [("9".to_owned(), offer(None))].into_iter().collect();

    let tokened = with_token(offers.clone(), "9", "ana-token");

    assert_eq!(
        tokened.and_then(|offers| offers.get("9").and_then(|one| one.decline.clone())),
        Some(TokenHash::of("ana-token"))
    );
    assert!(with_token(offers, "10", "ana-token").is_none());
}

#[test]
fn the_table_goes_into_the_decline_services_configuration_directory() {
    assert_eq!(
        path(
            Path::new("/stack"),
            lemonfiber_sidecar::decline::File::Table
        ),
        Path::new("/stack/config/decline/invitations.json")
    );
}

#[test]
fn an_account_is_declined_where_its_offers_token_was_refused() {
    use lemonfiber_sidecar::decline::{Refusal, Refusals};

    let offers: Offers = [
        ("9".to_owned(), offer(Some("ana-token"))),
        ("10".to_owned(), offer(Some("bo-token"))),
        ("11".to_owned(), offer(None)),
    ]
    .into_iter()
    .collect();
    let refusals = Refusals::default()
        .with(Refusal {
            token: TokenHash::of("ana-token"),
            account: "9".to_owned(),
            at: 1,
        })
        .with(Refusal {
            token: TokenHash::of("an-older-token"),
            account: "10".to_owned(),
            at: 1,
        });

    let declined = super::refused(&offers, &refusals);

    assert_eq!(
        declined.into_iter().collect::<Vec<_>>(),
        vec!["9".to_owned()]
    );
}

#[test]
fn refusals_are_recorded_beside_the_table() {
    assert_eq!(
        path(
            Path::new("/stack"),
            lemonfiber_sidecar::decline::File::Refusals
        ),
        Path::new("/stack/config/decline/refusals.json")
    );
}

#[test]
fn an_account_is_removed_at_its_lapse_only_for_the_offer_standing_now() {
    use lemonfiber_sidecar::decline::{Lapse, Lapses, Left, Outcome};

    let offers: Offers = [
        ("9".to_owned(), offer(Some("ana-token"))),
        ("10".to_owned(), offer(Some("bo-token"))),
        ("11".to_owned(), offer(Some("cy-token"))),
        ("12".to_owned(), offer(None)),
    ]
    .into_iter()
    .collect();
    let lapse = |token: &str, account: &str, name: &str, outcome| Lapse {
        token: TokenHash::of(token),
        account: account.to_owned(),
        name: name.to_owned(),
        issued: 1,
        at: 2,
        outcome,
    };
    let lapses = Lapses::default()
        .with(lapse("ana-token", "9", "ana", Outcome::Removed))
        .with(lapse("an-older-token", "10", "bo", Outcome::Removed))
        .with(lapse("cy-token", "11", "cy", Outcome::Left(Left::Claimed)))
        .with(lapse("bo-token", "somebody-else", "bo", Outcome::Removed));

    let removed = super::removed(&offers, &lapses);

    assert_eq!(
        removed.into_iter().collect::<Vec<_>>(),
        vec![("9".to_owned(), "ana".to_owned())]
    );
}

#[test]
fn lapses_are_recorded_beside_the_refusals() {
    assert_eq!(
        path(
            Path::new("/stack"),
            lemonfiber_sidecar::decline::File::Lapses
        ),
        Path::new("/stack/config/decline/lapses.json")
    );
}

/// A context over a stack written to `project`, on the real disk.
fn over(project: &Path) -> crate::app::Ctx {
    crate::test_support::a_context()
        .settings(crate::config::Settings {
            stack_dir: Some(project.to_path_buf()),
            ..crate::config::Settings::default()
        })
        .build()
}

/// A pipe the decline service put where its record goes is refused at once rather than
/// waited on, and a link there is not followed: either declines nobody.
#[cfg(unix)]
#[tokio::test]
async fn a_record_that_is_a_pipe_or_a_link_declines_nobody_and_never_waits() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("refusals-planted");
    let record = path(&project, super::File::Refusals);
    let _ = std::fs::create_dir_all(record.parent().unwrap_or(&project));
    let offers: Offers = [("9".to_owned(), offer(Some("ana-token")))]
        .into_iter()
        .collect();
    let made = tokio::process::Command::new("mkfifo")
        .arg(&record)
        .status()
        .await;
    assert!(made.is_ok_and(|status| status.success()), "a pipe was made");

    let piped = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        super::declined(&over(&project), &offers),
    )
    .await;

    assert_eq!(piped.ok().map(|declined| declined.len()), Some(0));

    let elsewhere = project.join("elsewhere.json");
    let _ = std::fs::write(&elsewhere, "{}");
    let _ = std::fs::remove_file(&record);
    let _ = std::os::unix::fs::symlink(&elsewhere, &record);
    assert!(super::declined(&over(&project), &offers).await.is_empty());
}

/// A stack compiled in, which is written out nowhere until a directory is chosen for it.
static STACKLET: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

/// With no record on disk, or no stack written out to hold one, there is nothing to read,
/// and nobody is declined.
#[tokio::test]
async fn without_a_stack_on_disk_nobody_is_declined() {
    let ctx = crate::test_support::a_context().build();
    let offers: Offers = [("9".to_owned(), offer(Some("ana-token")))]
        .into_iter()
        .collect();

    assert!(super::declined(&ctx, &offers).await.is_empty());

    let unwritten = crate::test_support::a_context()
        .over(crate::stack::Source::Embedded(&STACKLET))
        .build();
    assert!(super::declined(&unwritten, &offers).await.is_empty());
}
