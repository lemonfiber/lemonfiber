//! A member's search and ask, driven through the whole command against a house whose
//! media server and request service answer by what each call asks for.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{capitalised, wished, REFUSALS};
use crate::app::{Ctx, Outcome, Wishing};
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::http::Method;
use crate::ports::media::Kind;
use crate::ports::service::{Certificate, Unrated, Wish};
use crate::test_support::{a_context, json, media, Reporting, SeedFs};

/// A Servarr config carrying a readable key, so the curators resolve their targets.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// The household: Ada, whom nothing holds back; Bea, held to twelve with unrated titles
/// held back; and Cy, held to twelve with unrated titles let through.
pub(super) fn accounts() -> String {
    let held = |id: &str, name: &str, unrated: Unrated| {
        let mut member = media::member(id, name, true);
        member.access.age_limit = Some(12);
        member.access.unrated = unrated;
        member
    };
    json(&[
        media::member("a7f3", "Ada", true),
        held("b8e4", "Bea", Unrated::HeldBack),
        held("c9d5", "Cy", Unrated::LetThrough),
    ])
}

/// The media server's own table, as a British house's server answers it.
pub(super) fn ratings() -> String {
    let certificate = |name: &str, age: u32| Certificate {
        name: name.to_owned(),
        age,
    };
    json(&[
        certificate("U", 0),
        certificate("PG", 8),
        certificate("12A", 12),
        certificate("15", 15),
        certificate("18", 18),
    ])
}

/// The country the media server's table is for.
pub(super) fn country() -> String {
    json(&Some("GB"))
}

/// The member's account on the request service: asks that wait for approval.
pub(super) const WAITS: &str = r#"{"id":4,"permissions":32}"#;

/// The member's account on the request service: asks that arrive by themselves.
pub(super) const APPROVES_OWN: &str = r#"{"id":4,"permissions":160}"#;

/// What the member's period has counted: one film of two, nothing of five seasons.
pub(super) const COUNTED: &str =
    r#"{"movie":{"days":30,"limit":2,"used":1},"tv":{"days":30,"limit":5,"used":0}}"#;

/// The household's requests: none.
pub(super) const NO_REQUESTS: &str = r#"{"pageInfo":{"results":0},"results":[]}"#;

/// One route of the house: the method it answers, what its address contains, and its
/// answers in turn, the last repeating.
pub(super) type Route = (Method, &'static str, Vec<Answer>);

/// A route answering one thing, always.
pub(super) fn route(method: Method, fragment: &'static str, status: u16, body: &str) -> Route {
    (
        method,
        fragment,
        vec![Answer::reply(status, body.to_owned())],
    )
}

/// The house, answering `first` ahead of everything it answers by default.
pub(super) fn house(first: Vec<Route>) -> Arc<Transport> {
    let mut routes = first;
    routes.extend([
        route(Method::Post, media::identity("country"), 200, &country()),
        route(Method::Post, media::identity("ratings"), 200, &ratings()),
        route(Method::Post, media::identity("household"), 200, &accounts()),
        route(Method::Get, "/api/v1/user/jellyfin/", 200, WAITS),
        route(Method::Get, "/quota", 200, COUNTED),
        route(Method::Get, "/api/v1/request?", 200, NO_REQUESTS),
        route(
            Method::Post,
            "/api/v1/request",
            201,
            r#"{"id":7,"status":1}"#,
        ),
        route(Method::Get, "", 404, "{}"),
        route(Method::Post, "", 404, "{}"),
    ]);
    Transport::by_route_in_turn(routes)
}

/// A house whose curators for both kinds are running, over `transport`.
pub(super) fn ctx_over(transport: Arc<Transport>, tag: &str) -> Ctx {
    ctx_running(transport, tag, &["sonarr", "radarr"])
}

/// A house over `transport` whose running containers are `running`.
pub(super) fn ctx_running(transport: Arc<Transport>, tag: &str, running: &[&str]) -> Ctx {
    ctx_engine(
        transport,
        tag,
        Reporting::holding(running, Lifecycle::Running, Health::Healthy),
    )
}

/// A house over `transport` whose engine is `engine`, its media server moved into its
/// plugin.
pub(super) fn ctx_engine(transport: Arc<Transport>, tag: &str, engine: Reporting) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("wishing-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let disk = SeedFs::keyed(Some(KEYED), None)
        .with_requests(lemonfiber_fixtures::support::SEERR_SETTINGS);
    let mut context = a_context()
        .engine(Arc::new(engine.alongside(media::running())))
        .build()
        .with_filesystem(Arc::new(disk))
        .with_http(transport);
    context.settings.env_file = Some(dir.join(".env"));
    media::moved_in(&context);
    context
}

/// What `member` finds for `term` from `after`.
pub(super) async fn found(
    ctx: &Ctx,
    member: &str,
    term: &str,
    after: Option<&str>,
) -> Result<Outcome, Box<crate::error::Problem>> {
    wished(
        ctx,
        Wishing::Find {
            member: member.to_owned(),
            term: term.to_owned(),
            after: after.map(str::to_owned),
        },
    )
    .await
}

/// What `member` asking for `id` of `kind`, and `seasons` of it, comes to.
pub(super) async fn asked(
    ctx: &Ctx,
    member: &str,
    kind: Kind,
    id: &str,
    seasons: &[u32],
) -> Result<Outcome, Box<crate::error::Problem>> {
    wished(
        ctx,
        Wishing::Ask {
            member: member.to_owned(),
            wish: Wish {
                id: id.to_owned(),
                kind,
                seasons: seasons.to_vec(),
            },
        },
    )
    .await
}

/// The code an outcome was refused with, or nothing where it was answered.
pub(super) fn code(outcome: &Result<Outcome, Box<crate::error::Problem>>) -> Option<String> {
    outcome
        .as_ref()
        .err()
        .map(|problem| problem.code.to_string())
}

/// Whether the house was asked anything whose address contains `fragment`.
pub(super) fn asked_for(transport: &Transport, fragment: &str) -> bool {
    transport.asked_for(fragment)
}

#[tokio::test]
async fn nobody_named_is_refused_before_anything_is_asked() {
    let transport = house(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "nobody");

    let refused = found(&ctx, "  ", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-11"));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn somebody_the_household_does_not_hold_is_refused_as_not_in_it() {
    let transport = house(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "stranger");

    let refused = found(&ctx, "Zed", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-12"));
    assert!(!asked_for(&transport, "/search"));
}

#[tokio::test]
async fn a_house_with_no_request_service_is_not_set_up_for_asking() {
    let transport = house(Vec::new());
    let dir = lemonfiber_fixtures::scratch::Scratch::named("wishing-no-requests").kept();
    let _ = std::fs::remove_dir_all(&dir);
    lemonfiber_fixtures::stack::manifest_without(&dir, "seerr");
    let ctx = a_context()
        .over(crate::stack::Source::External(Box::leak(
            dir.into_boxed_path(),
        )))
        .build()
        .with_http(transport.clone());

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-9"));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn a_house_whose_stack_cannot_be_read_is_asking_not_working() {
    let transport = house(Vec::new());
    let dir = lemonfiber_fixtures::scratch::Scratch::named("wishing-no-stack").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let ctx = a_context()
        .over(crate::stack::Source::External(Box::leak(
            dir.into_boxed_path(),
        )))
        .build()
        .with_http(transport.clone());

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn a_media_server_that_will_not_say_who_is_in_the_house_is_asking_not_working() {
    let transport = house(vec![route(
        Method::Post,
        media::identity("household"),
        500,
        "",
    )]);
    let ctx = ctx_over(Arc::clone(&transport), "accounts-unread");

    let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert!(!asked_for(&transport, "/api/v1/movie/"));
}

#[tokio::test]
async fn a_held_member_whose_rating_table_or_country_is_unread_is_offered_nothing() {
    for (failing, tag) in [
        (media::identity("country"), "country-unread"),
        (media::identity("ratings"), "table-unread"),
    ] {
        let transport = house(vec![route(Method::Post, failing, 500, "")]);
        let ctx = ctx_over(Arc::clone(&transport), tag);

        let search = found(&ctx, "Bea", "harbour", None).await;
        let ask = asked(&ctx, "Bea", Kind::Movies, "603", &[]).await;

        assert_eq!(code(&search).as_deref(), Some("WISH-8"), "{failing}");
        assert_eq!(code(&ask).as_deref(), Some("WISH-8"), "{failing}");
        assert!(!asked_for(&transport, "/api/v1/search"), "{failing}");
        assert!(!asked_for(&transport, "/api/v1/movie/"), "{failing}");
    }
}

/// An engine that cannot say what runs cannot say where the media server is published
/// either, so asking is not working, and nothing is searched on a guess.
#[tokio::test]
async fn an_engine_that_cannot_say_what_runs_leaves_asking_not_working() {
    let transport = house(Vec::new());
    let dir = lemonfiber_fixtures::scratch::Scratch::named("wishing-engine-absent").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context()
        .engine(Arc::new(Reporting::absent()))
        .build()
        .with_http(transport.clone());
    ctx.settings.env_file = Some(dir.join(".env"));
    media::moved_in(&ctx);

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert!(!asked_for(&transport, "/api/v1/search"));
}

/// An engine gone once the media server is reached cannot say which curators run, so no
/// kind is offered to search and nothing is searched.
#[tokio::test]
async fn an_engine_gone_once_the_server_is_reached_offers_no_kind_to_search() {
    let transport = house(Vec::new());
    let ctx = ctx_engine(
        Arc::clone(&transport),
        "engine-gone",
        Reporting::holding(&["sonarr", "radarr"], Lifecycle::Running, Health::Healthy)
            .leaving_after(1),
    );

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-4"));
    assert!(!asked_for(&transport, "/api/v1/search"));
}

#[test]
fn every_refusal_is_a_code_the_registry_declares_with_a_household_remedy() {
    for code in REFUSALS {
        let declared = code.declaration();
        assert!(declared.is_some(), "{code}");
        let remedy = declared
            .map(crate::error::codes::Declared::remedy)
            .unwrap_or_default();
        for operator in ["lemonfiber", "service", "Seerr", "Jellyfin", "`"] {
            assert!(!remedy.contains(operator), "{code}: {remedy}");
        }
    }
}

#[test]
fn a_sentence_begins_with_a_capital_letter() {
    assert_eq!(capitalised("nothing arrives"), "Nothing arrives");
    assert_eq!(capitalised(""), "");
}
