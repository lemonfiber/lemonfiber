use std::path::Path;
use std::sync::Arc;

use lemonfiber_sidecar::TokenHash;

use super::{invitations, with_token};
use crate::invitation::{Offer, Offers};
use crate::ports::service::Member;
use crate::test_support::guards::{Decline, Serving, DECLINE};

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
fn the_invitations_hold_each_unclaimed_offer_that_carries_a_token() {
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

    let held = invitations(&offers, &household);

    assert_eq!(held.invitations.len(), 1);
    let ana = held
        .invitations
        .iter()
        .find(|one| one.token == TokenHash::of("ana-token").as_str());
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

    assert!(invitations(&offers, &[member("9", "ana", false)])
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
fn an_account_is_declined_where_its_offers_token_was_refused() {
    use crate::ports::service::Declined;

    let offers: Offers = [
        ("9".to_owned(), offer(Some("ana-token"))),
        ("10".to_owned(), offer(Some("bo-token"))),
        ("11".to_owned(), offer(None)),
    ]
    .into_iter()
    .collect();
    let refusals = [
        Declined {
            token: TokenHash::of("ana-token").as_str().to_owned(),
            account: "9".to_owned(),
            at: 1,
        },
        Declined {
            token: TokenHash::of("an-older-token").as_str().to_owned(),
            account: "10".to_owned(),
            at: 1,
        },
    ];

    let declined = super::refused(&offers, &refusals);

    assert_eq!(
        declined.into_iter().collect::<Vec<_>>(),
        vec!["9".to_owned()]
    );
}

#[test]
fn an_account_is_removed_at_its_lapse_only_for_the_offer_standing_now() {
    use crate::ports::service::{Lapsed, LeftBecause, TakenBack};

    let offers: Offers = [
        ("9".to_owned(), offer(Some("ana-token"))),
        ("10".to_owned(), offer(Some("bo-token"))),
        ("11".to_owned(), offer(Some("cy-token"))),
        ("12".to_owned(), offer(None)),
    ]
    .into_iter()
    .collect();
    let lapse = |token: &str, account: &str, name: &str, outcome| Lapsed {
        token: TokenHash::of(token).as_str().to_owned(),
        account: account.to_owned(),
        name: name.to_owned(),
        issued: 1,
        at: 2,
        outcome,
    };
    let lapses = [
        lapse("ana-token", "9", "ana", TakenBack::Removed),
        lapse("an-older-token", "10", "bo", TakenBack::Removed),
        lapse(
            "cy-token",
            "11",
            "cy",
            TakenBack::Left {
                why: LeftBecause::Claimed,
            },
        ),
        lapse("bo-token", "somebody-else", "bo", TakenBack::Removed),
    ];

    let removed = super::removed(&offers, &lapses);

    assert_eq!(
        removed.into_iter().collect::<Vec<_>>(),
        vec![("9".to_owned(), "ana".to_owned())]
    );
}

/// A context over a stack written out on the real disk whose decline service, moved into
/// its plugin, answers as `decline` does.
fn reaching(decline: Decline) -> crate::app::Ctx {
    let mut ctx = crate::test_support::a_context().build();
    ctx.settings.env_file =
        crate::app::targets::project_directory(&ctx.stack, None).map(|at| at.join(".env"));
    let decline = Arc::new(decline);
    DECLINE.reaching(ctx).with_http(Arc::new(
        Serving::beside(lemonfiber_fixtures::http::Fake::silent()).decline(&decline),
    ))
}

/// What the decline service answers it declined is read, and an answer it will not give
/// declines nobody and removed nobody: what it says cannot be told.
#[tokio::test]
async fn what_the_service_will_not_answer_declines_and_removed_nobody() {
    let offers: Offers = [("9".to_owned(), offer(Some("ana-token")))]
        .into_iter()
        .collect();
    let declining = || {
        Decline::default().declining(vec![crate::ports::service::Declined {
            token: TokenHash::of("ana-token").as_str().to_owned(),
            account: "9".to_owned(),
            at: 1,
        }])
    };
    assert_eq!(
        super::declined(&reaching(declining()), &offers)
            .await
            .into_iter()
            .collect::<Vec<_>>(),
        vec!["9".to_owned()],
        "what the service answers is what is read"
    );

    let silent = reaching(declining().refusing(&["refusals", "lapses"]));

    assert!(super::declined(&silent, &offers).await.is_empty());
    assert!(super::removed_at_lapse(&silent, &offers).await.is_empty());
}

/// With no record on disk, or no stack written out to hold one, there is nothing to read,
/// and nobody is declined or removed.
#[tokio::test]
async fn without_a_stack_on_disk_nobody_is_declined() {
    let ctx = crate::test_support::a_context().build();
    let offers: Offers = [("9".to_owned(), offer(Some("ana-token")))]
        .into_iter()
        .collect();

    assert!(super::declined(&ctx, &offers).await.is_empty());
    assert!(super::removed_at_lapse(&ctx, &offers).await.is_empty());

    let unwritten = crate::test_support::a_context()
        .over(crate::stack::Source::Embedded(
            &crate::test_support::STACKLET,
        ))
        .build();
    assert!(super::declined(&unwritten, &offers).await.is_empty());
    assert!(super::removed_at_lapse(&unwritten, &offers)
        .await
        .is_empty());
}

/// The plugin that fills the decline service in the tests that ask it.
const THEIRS: &str = "their-decline";

/// The decline service filled by [`THEIRS`] alone, speaking its contract where `speaks`,
/// written at `project` and trusted as first-party.
fn filled_by_theirs(project: &Path, speaks: bool) -> crate::wiring::Fillers {
    use crate::test_support::{a_placed, an_installed, contracted, CONTRACTED_PORT};
    let installed = if speaks {
        contracted(THEIRS, THEIRS, super::decline::CAPABILITY)
    } else {
        an_installed(
            THEIRS,
            vec![a_placed(
                THEIRS,
                &[super::decline::CAPABILITY],
                Some(CONTRACTED_PORT),
            )],
        )
    };
    crate::test_support::stack_fillers(
        Vec::new(),
        &[installed],
        Some(project),
        &crate::test_support::first_party(THEIRS),
    )
}

/// A context reaching [`THEIRS`] holding its key where `keyed`, over a stack written to
/// `project`, answering every call over `http`.
fn reaching_theirs(
    project: &Path,
    keyed: bool,
    http: std::sync::Arc<lemonfiber_fixtures::http::Fake>,
) -> crate::app::Ctx {
    crate::test_support::contracted_context(project, THEIRS, keyed)
        .settings(crate::config::Settings {
            env_file: Some(project.join(".env")),
            stack_dir: Some(project.to_path_buf()),
            ..crate::config::Settings::default()
        })
        .build()
        .with_http(http)
}

/// A decline service that speaks its contract is asked over it.
#[tokio::test]
async fn a_decline_service_speaking_its_contract_is_asked_over_it() {
    use lemonfiber_fixtures::http::{Answer, Fake};
    let project = lemonfiber_fixtures::scratch::Scratch::new("decline-asked-over");
    let http = Fake::always(Answer::reply(200, "null"));
    let ctx = reaching_theirs(&project, true, http.clone());

    let asked = super::asked(&ctx, &filled_by_theirs(&project, true)).await;
    let held = match asked {
        Some(decline) => decline.held().await.ok(),
        None => None,
    };

    assert_eq!(held, Some(None));
    assert_eq!(
        http.request().map(|request| request.url),
        Some(format!(
            "http://127.0.0.1:{}/lemonfiber/invite.decline/v1/held",
            crate::test_support::CONTRACTED_PORT
        ))
    );
}

/// A decline service that speaks its contract and cannot be reached over it is not
/// asked at all.
#[tokio::test]
async fn a_decline_service_that_cannot_be_reached_over_its_contract_is_not_asked() {
    use lemonfiber_fixtures::http::{Answer, Fake};
    let project = lemonfiber_fixtures::scratch::Scratch::new("decline-asked-unanswered");
    let http = Fake::always(Answer::reply(200, "null"));
    let ctx = reaching_theirs(&project, false, http.clone());

    assert!(super::asked(&ctx, &filled_by_theirs(&project, true))
        .await
        .is_none());
    assert!(http.request().is_none());
}

/// A plugin filling the decline service that speaks no contract is asked nothing, a
/// first-party one standing in for the stack's own included.
#[tokio::test]
async fn a_plugin_speaking_no_contract_is_not_asked_even_standing_in() {
    use lemonfiber_fixtures::http::{Answer, Fake};
    let project = lemonfiber_fixtures::scratch::Scratch::new("decline-asked-unspoken");
    let ctx = reaching_theirs(&project, true, Fake::always(Answer::reply(200, "null")));
    let unspoken = filled_by_theirs(&project, false);
    let mut standing = crate::test_support::a_placed(
        THEIRS,
        &[super::decline::CAPABILITY],
        Some(crate::test_support::CONTRACTED_PORT),
    );
    standing.replaces = Some(THEIRS.to_owned());
    let mut theirs = crate::test_support::an_installed(THEIRS, vec![standing]);
    theirs.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    theirs.tree = crate::test_support::A_TREE.to_owned();
    let stood_in = crate::test_support::stack_fillers(
        Vec::new(),
        &[theirs],
        Some(&project),
        &crate::test_support::first_party(THEIRS),
    );

    assert!(stood_in
        .service(THEIRS)
        .is_some_and(crate::wiring::Filler::stands_in));
    for fillers in [unspoken, stood_in] {
        assert!(super::asked(&ctx, &fillers).await.is_none());
    }
}
