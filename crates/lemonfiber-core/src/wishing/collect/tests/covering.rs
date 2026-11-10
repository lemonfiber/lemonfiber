//! The cover of music a member found or asked for, fetched by the core from wherever its
//! curator says it is and handed to that member alone.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};
use lemonfiber_fixtures::ports::Resolving;

use super::flows::{asked, candidate, ctx, ALBUM};
use crate::app::Ctx;
use crate::collected::Unit;
use crate::config::{Reaching, REACH_POSTERS_KEY};
use crate::error::codes::wish::{CANNOT_FETCH, NOT_ASKABLE, NOT_WORKING, NO_PICTURE};
use crate::error::Code;
use crate::ports::service::{BookUnit, Candidate, CandidateState};
use crate::screening::Pictured;
use crate::test_support::adapted::answered;
use crate::test_support::collecting::at;
use crate::wishing::{cover, kept_cover};

const KIND_OF_BLUE: &str = "mbid-kind-of-blue";

const GIF: &str = "GIF89a, a picture's bytes";

fn covered(at_location: &str) -> Candidate {
    Candidate {
        cover: Some(at_location.to_owned()),
        ..candidate(CandidateState::MayAsk)
    }
}

fn house_covering(found: Candidate) -> Arc<Transport> {
    super::flows::the_house(vec![
        (
            "covers.example",
            None,
            vec![Answer::served(200, "image/gif", GIF)],
        ),
        (at("candidate"), None, vec![answered(&Some(found))]),
    ])
}

fn outward(transport: &Arc<Transport>, tag: &str) -> Ctx {
    ctx(transport, tag).with_resolver(Resolving::anywhere())
}

fn code(outcome: &Result<Pictured, Box<crate::error::Problem>>) -> Option<Code> {
    outcome.as_ref().err().map(|problem| problem.code)
}

fn reached_covers(transport: &Transport) -> usize {
    transport
        .requests()
        .iter()
        .filter(|one| one.url.contains("covers.example"))
        .count()
}

#[tokio::test]
async fn a_candidate_s_cover_is_fetched_from_its_host_which_is_noted() {
    let transport = house_covering(covered("https://covers.example/blue.jpg"));
    let ctx = outward(&transport, "cover-handed");

    let handed = cover(&ctx, "Ada", ALBUM, KIND_OF_BLUE).await;

    assert_eq!(
        handed.ok().map(|pictured| pictured.bytes),
        Some(GIF.as_bytes().to_vec())
    );
    assert_eq!(crate::outbound::covers::recorded(&ctx), ["covers.example"]);
}

#[tokio::test]
async fn a_cover_not_over_https_with_a_login_or_somewhere_inward_is_never_reached() {
    for (location, tag, resolving) in [
        (
            "http://covers.example/blue.jpg",
            "cover-plain",
            Resolving::anywhere(),
        ),
        (
            "https://me:secret@covers.example/blue.jpg",
            "cover-login",
            Resolving::anywhere(),
        ),
        (
            "https://covers.example/blue.jpg",
            "cover-inward",
            Resolving::standing_for(&[std::net::IpAddr::from([10, 0, 0, 7])]),
        ),
    ] {
        let transport = house_covering(covered(location));
        let ctx = ctx(&transport, tag).with_resolver(resolving);

        let refused = cover(&ctx, "Ada", ALBUM, KIND_OF_BLUE).await;

        assert_eq!(code(&refused), Some(NOT_WORKING), "{tag}");
        assert_eq!(reached_covers(&transport), 0, "{tag}");
        assert!(crate::outbound::covers::recorded(&ctx).is_empty(), "{tag}");
    }
}

#[tokio::test]
async fn no_cover_or_covers_switched_off_is_no_picture() {
    let bare = house_covering(candidate(CandidateState::MayAsk));
    let bare_ctx = outward(&bare, "cover-none");
    let switched = house_covering(covered("https://covers.example/blue.jpg"));
    let mut switched_ctx = outward(&switched, "cover-off");
    switched_ctx.settings.reaching = Reaching::without(REACH_POSTERS_KEY);

    assert_eq!(
        code(&cover(&bare_ctx, "Ada", ALBUM, KIND_OF_BLUE).await),
        Some(NO_PICTURE)
    );
    assert_eq!(
        code(&cover(&switched_ctx, "Ada", ALBUM, KIND_OF_BLUE).await),
        Some(NO_PICTURE)
    );
    assert_eq!(reached_covers(&switched), 0);
}

#[tokio::test]
async fn a_cover_of_nothing_the_curator_holds_or_of_a_medium_nobody_fetches_is_refused() {
    let transport = super::flows::the_house(vec![(
        at("candidate"),
        None,
        vec![answered(&None::<Candidate>)],
    )]);
    let ctx = outward(&transport, "cover-unheld");

    let unheld = cover(&ctx, "Ada", ALBUM, KIND_OF_BLUE).await;
    let unfetched = cover(&ctx, "Ada", Unit::Books(BookUnit::Book), KIND_OF_BLUE).await;

    assert_eq!(code(&unheld), Some(NOT_ASKABLE));
    assert_eq!(code(&unfetched), Some(CANNOT_FETCH));
}

#[tokio::test]
async fn a_curator_that_fails_to_answer_for_a_cover_is_not_working() {
    let transport = super::flows::the_house(vec![(
        at("candidate"),
        None,
        vec![Answer::reply(500, "down")],
    )]);
    let ctx = outward(&transport, "cover-failing");

    let failed = cover(&ctx, "Ada", ALBUM, KIND_OF_BLUE).await;

    assert_eq!(code(&failed), Some(NOT_WORKING));
    assert_eq!(reached_covers(&transport), 0);
}

#[tokio::test]
async fn a_kept_ask_s_cover_is_the_asker_s_and_the_operator_s_alone() {
    let transport = house_covering(covered("https://covers.example/blue.jpg"));
    let ctx = outward(&transport, "cover-kept");
    let _ = asked(&ctx, "Ada", ALBUM).await;

    let own = kept_cover(&ctx, Some("Ada"), "music-1").await;
    let operator = kept_cover(&ctx, None, "music-1").await;
    let someone_else = kept_cover(&ctx, Some("Bea"), "music-1").await;
    let unkept = kept_cover(&ctx, None, "music-9").await;
    let filed = kept_cover(&ctx, None, "42").await;
    let nonsense = kept_cover(&ctx, None, "not a reference").await;

    assert!(own.is_ok(), "{:?}", code(&own));
    assert!(operator.is_ok(), "{:?}", code(&operator));
    for refused in [someone_else, unkept, filed, nonsense] {
        assert_eq!(code(&refused), Some(NOT_ASKABLE));
    }
}
