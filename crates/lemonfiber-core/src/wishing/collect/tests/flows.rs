//! A member's search and ask for music, driven through the whole command against a house
//! whose curator of music speaks `music.curate`.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use crate::app::{Ctx, Outcome, Wishing};
use crate::collected::store::{self, Standing};
use crate::collected::{Medium, Unit};
use crate::error::codes::wish::{
    ALREADY_HERE, CANNOT_FETCH, NOTHING_LEFT, NOT_ASKABLE, NOT_A_MEMBER, NOT_A_PLACE, NOT_A_SEARCH,
    NOT_WORKING, NO_ROOM,
};
use crate::error::Code;
use crate::model::{AskedReport, Became, Stands, TitleKind};
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::service::{
    AskedWith, BookUnit, Candidate, CandidateState, Candidates, MusicUnit, Profile,
};
use crate::test_support::adapted::{answered, unavailable};
use crate::test_support::collecting::{at, at_of, speaking, speaking_of, BOOKS};
use crate::test_support::curators::MUSIC;
use crate::test_support::Reporting;
use crate::wishing::tests::{account, call, code, ctx_engine, house, Route};

pub(super) const ALBUM: Unit = Unit::Music(MusicUnit::Album);

const ARTIST: Unit = Unit::Music(MusicUnit::Artist);

const KIND_OF_BLUE: &str = "mbid-kind-of-blue";

pub(super) fn candidate(state: CandidateState) -> Candidate {
    Candidate {
        reference: KIND_OF_BLUE.to_owned(),
        name: "Kind of Blue".to_owned(),
        by: Some("Miles Davis".to_owned()),
        year: Some(1959),
        cover: None,
        format: None,
        state,
    }
}

pub(super) fn curator(operation: &str, answer: Answer) -> Route {
    (at(operation), None, vec![answer])
}

fn with() -> AskedWith {
    AskedWith {
        units: vec![MusicUnit::Album.into(), MusicUnit::Artist.into()],
        root_folders: vec!["/music".to_owned()],
        quality: vec![Profile {
            id: 1,
            name: "Lossless".to_owned(),
        }],
        metadata: vec![Profile {
            id: 2,
            name: "Standard".to_owned(),
        }],
    }
}

pub(super) fn the_house(first: Vec<Route>) -> Arc<Transport> {
    let mut routes = first;
    routes.extend([
        curator("profiles", answered(&with())),
        curator("request", answered(&candidate(CandidateState::Asked))),
    ]);
    house(routes)
}

fn engine() -> Reporting {
    Reporting::holding(&["sonarr", "radarr"], Lifecycle::Running, Health::Healthy)
        .alongside(crate::test_support::adapted::running(&[MUSIC]))
}

pub(super) fn ctx(transport: &Arc<Transport>, tag: &str) -> Ctx {
    let ctx = ctx_engine(Arc::clone(transport), &format!("collect-{tag}"), engine());
    speaking(&ctx);
    ctx
}

async fn found(
    ctx: &Ctx,
    unit: Unit,
    after: Option<&str>,
) -> Result<Outcome, Box<crate::error::Problem>> {
    crate::wishing::wished(
        ctx,
        Wishing::Find {
            member: "Ada".to_owned(),
            term: "kind of blue".to_owned(),
            after: after.map(str::to_owned),
            unit: Some(unit),
        },
    )
    .await
}

pub(super) async fn asked(
    ctx: &Ctx,
    member: &str,
    unit: Unit,
) -> Result<Outcome, Box<crate::error::Problem>> {
    crate::wishing::wished(
        ctx,
        Wishing::Collect {
            member: member.to_owned(),
            unit,
            reference: KIND_OF_BLUE.to_owned(),
        },
    )
    .await
}

fn report(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<AskedReport> {
    match outcome {
        Ok(Outcome::Asked(report)) => Some(report),
        _ => None,
    }
}

pub(super) fn is(outcome: &Result<Outcome, Box<crate::error::Problem>>, refused: Code) -> bool {
    code(outcome) == Some(refused.to_string())
}

pub(super) fn sent(transport: &Transport, operation: &str) -> usize {
    transport
        .requests()
        .iter()
        .filter(|one| one.url.contains(at(operation)))
        .count()
}

pub(super) fn kept(ctx: &Ctx) -> Vec<(String, Standing)> {
    crate::collected::run::every(ctx)
        .into_iter()
        .map(|ask| (ask.named().to_string(), ask.standing))
        .collect()
}

#[tokio::test]
async fn a_search_for_music_is_the_curators_page_in_the_households_words() {
    let page = Candidates {
        found: vec![
            candidate(CandidateState::MayAsk),
            candidate(CandidateState::Here),
        ],
        next: Some("20".to_owned()),
    };
    let transport = the_house(vec![curator("lookup", answered(&page))]);
    let ctx = ctx(&transport, "found");

    let shown = match found(&ctx, ALBUM, None).await {
        Ok(Outcome::Askable(report)) => Some(report),
        _ => None,
    };

    assert_eq!(
        shown.as_ref().map(|one| one.kinds.clone()),
        Some(vec![TitleKind::Music])
    );
    assert_eq!(
        shown.as_ref().map(|one| one
            .titles
            .iter()
            .map(|title| title.stands)
            .collect::<Vec<_>>()),
        Some(vec![Stands::Askable, Stands::Here])
    );
    assert_eq!(shown.and_then(|one| one.next).as_deref(), Some("20"));
}

#[tokio::test]
async fn a_search_from_nowhere_a_page_gives_or_of_a_curator_not_answering_is_refused() {
    let transport = the_house(vec![curator("lookup", unavailable())]);
    let ctx = ctx(&transport, "found-refused");

    let nowhere = found(&ctx, ALBUM, Some("")).await;
    let too_far = found(&ctx, ALBUM, Some(&"9".repeat(1025))).await;
    let unanswered = found(&ctx, ALBUM, None).await;

    assert!(is(&nowhere, NOT_A_PLACE));
    assert!(is(&too_far, NOT_A_PLACE));
    assert!(is(&unanswered, NOT_WORKING));
}

#[tokio::test]
async fn a_house_with_no_curator_of_a_medium_cannot_fetch_it() {
    let transport = the_house(Vec::new());
    let ctx = ctx_engine(Arc::clone(&transport), "collect-no-curator", engine());

    let refused = found(&ctx, ALBUM, None).await;
    let unasked = asked(&ctx, "Ada", ALBUM).await;

    assert!(is(&refused, CANNOT_FETCH));
    assert!(is(&unasked, CANNOT_FETCH));
}

#[tokio::test]
async fn an_ask_for_an_album_waits_where_the_members_asks_wait_and_is_kept() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let ctx = ctx(&transport, "asked-waits");

    let done = report(asked(&ctx, "Ada", ALBUM).await);

    assert_eq!(done.as_ref().map(|one| one.outcome), Some(Became::Waiting));
    assert_eq!(done.and_then(|one| one.request).as_deref(), Some("music-1"));
    assert_eq!(kept(&ctx), [("music-1".to_owned(), Standing::Waiting)]);
    assert_eq!(sent(&transport, "request"), 0);
}

#[tokio::test]
async fn an_album_asked_by_a_member_who_approves_their_own_is_sent_and_kept_as_sent() {
    let transport = the_house(vec![
        call("requesting", account(true)),
        curator(
            "candidate",
            answered(&Some(candidate(CandidateState::MayAsk))),
        ),
    ]);
    let ctx = ctx(&transport, "asked-sent");

    let done = report(asked(&ctx, "Ada", ALBUM).await);

    assert_eq!(done.map(|one| one.outcome), Some(Became::Asked));
    assert_eq!(sent(&transport, "request"), 1);
    assert_eq!(kept(&ctx), [("music-1".to_owned(), Standing::Sent)]);
}

#[tokio::test]
async fn an_artist_held_in_part_is_asked_for_the_rest() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::PartlyHere))),
    )]);
    let ctx = ctx(&transport, "asked-partly");

    let done = report(asked(&ctx, "Ada", ARTIST).await);

    assert_eq!(done.map(|one| one.outcome), Some(Became::Waiting));
    assert_eq!(kept(&ctx).len(), 1);
}

#[tokio::test]
async fn a_whole_artist_waits_even_for_a_member_who_approves_their_own() {
    let transport = the_house(vec![
        call("requesting", account(true)),
        curator(
            "candidate",
            answered(&Some(candidate(CandidateState::MayAsk))),
        ),
    ]);
    let ctx = ctx(&transport, "asked-artist");

    let done = report(asked(&ctx, "Ada", ARTIST).await);

    assert_eq!(done.map(|one| one.outcome), Some(Became::Waiting));
    assert_eq!(sent(&transport, "request"), 0);
}

#[tokio::test]
async fn a_rehearsed_ask_says_what_it_would_come_to_and_keeps_nothing() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let mut ctx = ctx(&transport, "asked-rehearsed");
    ctx.dry_run = true;

    let done = report(asked(&ctx, "Ada", ALBUM).await);

    assert_eq!(done.as_ref().map(|one| one.outcome), Some(Became::Waiting));
    assert_eq!(done.and_then(|one| one.request), None);
    assert!(kept(&ctx).is_empty());
}

#[tokio::test]
async fn what_is_here_is_refused_and_what_is_on_its_way_is_said_to_be() {
    let here = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::Here))),
    )]);
    let wanted = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::Asked))),
    )]);

    let refused = asked(&ctx(&here, "asked-here"), "Ada", ALBUM).await;
    let on_its_way = report(asked(&ctx(&wanted, "asked-wanted"), "Ada", ALBUM).await);

    assert!(is(&refused, ALREADY_HERE));
    assert_eq!(
        on_its_way.as_ref().map(|one| one.outcome),
        Some(Became::OnItsWay)
    );
    assert_eq!(on_its_way.and_then(|one| one.request), None);
}

#[tokio::test]
async fn an_ask_for_what_already_waits_here_is_on_its_way_and_kept_once() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let ctx = ctx(&transport, "asked-twice");

    let first = report(asked(&ctx, "Ada", ALBUM).await);
    let again = report(asked(&ctx, "Ada", ALBUM).await);

    assert_eq!(first.map(|one| one.outcome), Some(Became::Waiting));
    assert_eq!(again.map(|one| one.outcome), Some(Became::OnItsWay));
    assert_eq!(kept(&ctx).len(), 1);
}

#[tokio::test]
async fn a_reference_the_curator_does_not_hold_or_cannot_be_asked_about_is_refused() {
    let unheld = the_house(vec![curator("candidate", answered(&None::<Candidate>))]);
    let unanswered = the_house(vec![curator("candidate", unavailable())]);

    let none = asked(&ctx(&unheld, "asked-unheld"), "Ada", ALBUM).await;
    let down = asked(&ctx(&unanswered, "asked-down"), "Ada", ALBUM).await;

    assert!(is(&none, NOT_ASKABLE));
    assert!(is(&down, NOT_WORKING));
}

#[tokio::test]
async fn a_curator_that_will_not_take_an_ask_keeps_nothing() {
    let transport = house(vec![
        call("requesting", account(true)),
        curator(
            "candidate",
            answered(&Some(candidate(CandidateState::MayAsk))),
        ),
        curator("profiles", answered(&with())),
        curator("request", unavailable()),
    ]);
    let ctx = ctx(&transport, "asked-not-taken");

    let refused = asked(&ctx, "Ada", ALBUM).await;

    assert!(is(&refused, NOT_WORKING));
    assert!(kept(&ctx).is_empty());
}

#[tokio::test]
async fn a_member_who_asked_for_all_they_may_is_refused() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let ctx = ctx(&transport, "asked-spent");
    let at = crate::collected::run::kept_at(&ctx).ok();
    for reference in ["mbid-one", "mbid-two"] {
        let asking = store::Asking {
            unit: ALBUM,
            reference: reference.to_owned(),
            name: reference.to_owned(),
            by: None,
            year: None,
            member: "a7f3".to_owned(),
            at: crate::instant::written(ctx.seams.clock.now()).unwrap_or_default(),
            standing: Standing::Waiting,
        };
        assert!(at
            .as_deref()
            .is_some_and(|at| store::filed(at, &asking).is_ok()));
    }

    let refused = asked(&ctx, "Ada", ALBUM).await;

    assert!(is(&refused, NOTHING_LEFT));
}

#[tokio::test]
async fn a_house_with_no_room_keeps_no_ask() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let mut ctx = ctx(&transport, "asked-no-room");
    let full = crate::ports::filesystem::StorageFacts {
        point: std::path::PathBuf::new(),
        kind: crate::ports::filesystem::FsKind::Linking("test".to_owned()),
        removable: false,
        available: 0,
        total: 4 * 1024 * 1024 * 1024 * 1024,
    };
    ctx = ctx.with_filesystem(Arc::new(
        crate::test_support::SeedFs::keyed(None).with_facts(full),
    ));
    ctx.settings.data_root = ctx
        .settings
        .env_file
        .clone()
        .and_then(|env| env.parent().map(std::path::Path::to_path_buf));

    let refused = asked(&ctx, "Ada", ALBUM).await;

    assert!(is(&refused, NO_ROOM));
    assert!(kept(&ctx).is_empty());
}

#[tokio::test]
async fn a_search_for_books_is_asked_of_the_curator_of_books() {
    let page = Candidates {
        found: vec![Candidate {
            format: Some("ebook".to_owned()),
            ..candidate(CandidateState::MayAsk)
        }],
        next: None,
    };
    let transport = house(vec![(
        at_of(Medium::Books, "lookup"),
        None,
        vec![answered(&page)],
    )]);
    let ctx = ctx_engine(
        Arc::clone(&transport),
        "collect-books",
        Reporting::holding(&["sonarr", "radarr"], Lifecycle::Running, Health::Healthy)
            .alongside(crate::test_support::adapted::running(&[BOOKS])),
    );
    speaking_of(&ctx, Medium::Books);

    let shown = match found(&ctx, Unit::Books(BookUnit::Series), None).await {
        Ok(Outcome::Askable(report)) => Some(report),
        _ => None,
    };

    assert_eq!(shown.map(|one| one.kinds), Some(vec![TitleKind::Books]));
}

#[tokio::test]
async fn a_search_of_no_words_or_by_nobody_in_the_household_is_refused() {
    let transport = the_house(Vec::new());
    let ctx = ctx(&transport, "found-nobody");

    let wordless = crate::wishing::wished(
        &ctx,
        Wishing::Find {
            member: "Ada".to_owned(),
            term: "  ".to_owned(),
            after: None,
            unit: Some(ALBUM),
        },
    )
    .await;
    let stranger = crate::wishing::wished(
        &ctx,
        Wishing::Find {
            member: "Zed".to_owned(),
            term: "kind of blue".to_owned(),
            after: None,
            unit: Some(ALBUM),
        },
    )
    .await;
    let asked_by_a_stranger = asked(&ctx, "Zed", ALBUM).await;

    assert!(is(&wordless, NOT_A_SEARCH), "{wordless:?}");
    assert!(is(&stranger, NOT_A_MEMBER), "{stranger:?}");
    assert!(is(&asked_by_a_stranger, NOT_A_MEMBER));
}

#[tokio::test]
async fn an_allowance_that_cannot_be_read_is_not_working() {
    let transport = house(vec![
        call("left", unavailable()),
        curator("lookup", answered(&Candidates::default())),
        curator(
            "candidate",
            answered(&Some(candidate(CandidateState::MayAsk))),
        ),
    ]);
    let ctx = ctx(&transport, "allowance-unread");

    let searched = found(&ctx, ALBUM, None).await;
    let asking = asked(&ctx, "Ada", ALBUM).await;

    assert!(is(&searched, NOT_WORKING), "{searched:?}");
    assert!(is(&asking, NOT_WORKING), "{asking:?}");
}

#[tokio::test]
async fn an_artist_held_in_part_whose_ask_waits_here_is_on_its_way() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::PartlyHere))),
    )]);
    let ctx = ctx(&transport, "partly-twice");

    let first = report(asked(&ctx, "Ada", ARTIST).await);
    let again = report(asked(&ctx, "Ada", ARTIST).await);

    assert_eq!(first.map(|one| one.outcome), Some(Became::Waiting));
    assert_eq!(again.map(|one| one.outcome), Some(Became::OnItsWay));
}

#[tokio::test]
async fn an_ask_that_cannot_be_kept_is_not_working() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let ctx = ctx(&transport, "unkeepable");
    let at = crate::collected::run::kept_at(&ctx).ok();
    assert!(at
        .as_deref()
        .is_some_and(|at| std::fs::create_dir_all(at.join("in-the-way")).is_ok()));

    let refused = asked(&ctx, "Ada", ALBUM).await;

    assert!(is(&refused, NOT_WORKING), "{refused:?}");
}

#[tokio::test]
async fn an_ask_with_nowhere_to_be_kept_asks_the_curator_nothing() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let mut ctx = ctx(&transport, "nowhere-kept");
    ctx.settings.env_file = None;

    let refused = asked(&ctx, "Ada", ALBUM).await;

    assert!(is(&refused, NOT_WORKING), "{refused:?}");
    assert_eq!(sent(&transport, "candidate"), 0);
    assert_eq!(sent(&transport, "request"), 0);
}
