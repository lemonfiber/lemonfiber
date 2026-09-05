//! What the request service is made to say to the household, driven through the HTTP
//! port against a fake transport.
//!
//! Driven from here rather than in-crate for the reason `seerr.rs` next door is: the
//! client speaks an async trait built on another, and a path exercised only from an
//! in-crate module is counted from the wrong copy.
//!
//! **Every fixture answers by route rather than in turn, except where the list changes
//! under the write.** Hanging a notice reads the whole list, takes this program's rows
//! down, files the new ones, and writes the list back whole — so the read has to answer
//! one thing before and another after, and the defect worth catching is a body that
//! forgets somebody else's row, which a queue could not see.

use std::sync::Arc;

use lemonfiber_core::ports::http::{Http, Method, Request};
use lemonfiber_core::ports::service::Noticing;
use lemonfiber_core::seerr::Seerr;
use lemonfiber_fixtures::http::{Answer, Fake};

fn seerr(fake: &Arc<Fake>) -> Seerr {
    let http: Arc<dyn Http> = fake.clone();
    Seerr::new(http, "http://127.0.0.1:5055", "seerr")
}

/// The rows of the home page as the service answers them: two it ships, one an operator
/// arranged for themselves and switched off, and two this program hung.
const ROWS: &str = r#"[
    {"id":1,"type":1,"order":0,"isBuiltIn":true,"enabled":true,"title":null,"data":null},
    {"id":9,"type":19,"order":1,"isBuiltIn":false,"enabled":false,"title":"Wessel's row","data":"213"},
    {"id":4,"type":17,"order":2,"isBuiltIn":false,"enabled":true,"title":"a film costs this much","data":"lemonfiber"},
    {"id":5,"type":17,"order":3,"isBuiltIn":false,"enabled":true,"title":"the disk is full","data":"lemonfiber"},
    {"id":2,"type":2,"order":4,"isBuiltIn":true,"enabled":false,"title":null,"data":null}
]"#;

/// The same house after this program's rows have been taken down and two filed again.
const AFTER: &str = r#"[
    {"id":1,"type":1,"order":0,"isBuiltIn":true,"enabled":true,"title":null,"data":null},
    {"id":9,"type":19,"order":1,"isBuiltIn":false,"enabled":false,"title":"Wessel's row","data":"213"},
    {"id":2,"type":2,"order":2,"isBuiltIn":true,"enabled":false,"title":null,"data":null},
    {"id":11,"type":17,"order":-1,"isBuiltIn":false,"enabled":false,"title":"something new","data":"lemonfiber"}
]"#;

/// A house whose home page this program has never touched.
const UNTOUCHED: &str = r#"[
    {"id":1,"type":1,"order":0,"isBuiltIn":true,"enabled":true,"title":null,"data":null}
]"#;

/// A fixture for the four calls hanging a notice makes, with the read changing under it.
///
/// The two `POST` rules are ordered so the narrower path is tried first: the service
/// files a new row at a path the whole-list write is a prefix of, and a table that put
/// the prefix in front would answer the wrong one.
fn hanging(before: &'static str, after: &'static str) -> Arc<Fake> {
    Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/discover",
            vec![Answer::reply(200, before), Answer::reply(200, after)],
        ),
        (
            Method::Delete,
            "/settings/discover/",
            vec![Answer::reply(204, "")],
        ),
        (
            Method::Post,
            "/settings/discover/add",
            vec![Answer::reply(200, "{}")],
        ),
        (
            Method::Post,
            "/settings/discover",
            vec![Answer::reply(200, "[]")],
        ),
    ])
}

/// Every request that went to a path holding `fragment`, by method.
fn sent_to(fake: &Arc<Fake>, method: Method, fragment: &str) -> Vec<Request> {
    fake.requests()
        .into_iter()
        .filter(|request: &Request| request.url.contains(fragment) && request.method == method)
        .collect()
}

/// Saying again what is already said writes nothing at all.
///
/// This rides along on a reading somebody may take several times a minute, so a write
/// every time would rearrange a household's home page under them on every glance.
#[tokio::test]
async fn saying_again_what_is_already_said_writes_nothing() {
    let fake = hanging(ROWS, ROWS);

    let hung = seerr(&fake)
        .set_notices(&[
            "a film costs this much".to_owned(),
            "the disk is full".to_owned(),
        ])
        .await;

    assert!(hung.is_ok());
    assert_eq!(
        fake.requests().len(),
        1,
        "a house already showing what it was told to show was written to anyway: {:?}",
        fake.requests()
            .iter()
            .map(|request| request.url.clone())
            .collect::<Vec<_>>()
    );
}

/// The same two notices in the other order are two different notices.
#[tokio::test]
async fn the_same_notices_in_another_order_are_written() {
    let fake = hanging(ROWS, AFTER);

    let hung = seerr(&fake)
        .set_notices(&[
            "the disk is full".to_owned(),
            "a film costs this much".to_owned(),
        ])
        .await;

    assert!(hung.is_ok());
    assert!(
        !sent_to(&fake, Method::Post, "/settings/discover/add").is_empty(),
        "a house told to read its notices the other way round was left as it was"
    );
}

/// A changed notice takes this program's rows down, files the new ones, and writes the
/// whole list back — with everybody else's row carried through exactly as found.
#[tokio::test]
async fn hanging_a_notice_leaves_every_other_row_as_it_was() {
    let fake = hanging(ROWS, AFTER);

    let hung = seerr(&fake)
        .set_notices(&["something new".to_owned()])
        .await;

    assert!(hung.is_ok());

    let taken_down = sent_to(&fake, Method::Delete, "/settings/discover/");
    assert_eq!(
        taken_down.len(),
        2,
        "this program's two rows were not both taken down before the new one was filed"
    );
    assert!(
        taken_down.iter().any(|request| request.url.ends_with("/4"))
            && taken_down.iter().any(|request| request.url.ends_with("/5")),
        "something other than this program's own rows was taken down: {:?}",
        taken_down
            .iter()
            .map(|request| request.url.clone())
            .collect::<Vec<_>>()
    );

    let filed = sent_to(&fake, Method::Post, "/settings/discover/add");
    assert_eq!(
        filed.len(),
        1,
        "one notice was filed as {} rows",
        filed.len()
    );
    let Some(body) = filed.first().and_then(|request| request.body.clone()) else {
        unreachable!("a row is filed with a body naming it");
    };
    assert!(
        body.contains("something new") && body.contains("lemonfiber"),
        "the row filed carried neither the sentence nor the mark that says whose it \
         is: {body}"
    );

    let Some(written) = sent_to(&fake, Method::Post, "/settings/discover")
        .into_iter()
        .filter(|request| !request.url.contains("/add"))
        .next_back()
        .and_then(|request| request.body)
    else {
        unreachable!("the whole list is written back once the rows are filed");
    };
    let Ok(rows) = serde_json::from_str::<serde_json::Value>(&written) else {
        unreachable!("the write sends JSON");
    };
    let Some(rows) = rows.as_array() else {
        unreachable!("the write sends an array");
    };
    assert_eq!(
        rows.len(),
        4,
        "the write back named {} of the four rows the service holds, and a row left \
         out of it is a row switched off: {written}",
        rows.len()
    );
    let Some(first) = rows.first() else {
        unreachable!("an array of four has a first");
    };
    assert_eq!(
        first["id"], 11,
        "this program's notice was not put at the front, where somebody about to ask \
         for something reads it: {written}"
    );
    assert_eq!(
        first["enabled"], true,
        "the notice was filed and left switched off, which is a notice nobody sees"
    );
    let Some(theirs) = rows.iter().find(|row| row["id"] == 9) else {
        unreachable!("the operator's own row is among the four");
    };
    assert_eq!(
        theirs["enabled"], false,
        "a row the operator had switched off came back switched on: {written}"
    );
    assert_eq!(
        theirs["title"], "Wessel's row",
        "the operator's own row lost its heading on the way through: {written}"
    );
}

/// Taking every notice down is a house with nothing left to say.
#[tokio::test]
async fn a_house_with_nothing_to_say_takes_its_notices_down() {
    let fake = hanging(ROWS, UNTOUCHED);

    let hung = seerr(&fake).set_notices(&[]).await;

    assert!(hung.is_ok());
    assert_eq!(
        sent_to(&fake, Method::Delete, "/settings/discover/").len(),
        2,
        "a house told to say nothing kept a notice standing after it stopped being true"
    );
    assert!(
        sent_to(&fake, Method::Post, "/settings/discover/add").is_empty(),
        "a house told to say nothing filed a row anyway"
    );
}

/// A service that will not answer is a failure rather than a house showing nothing.
#[tokio::test]
async fn a_service_that_will_not_answer_is_a_failure() {
    let fake = Fake::silent();

    assert!(
        seerr(&fake)
            .set_notices(&["anything".to_owned()])
            .await
            .is_err(),
        "a request service that answered nothing was reported as showing what it was told to"
    );
}

/// An answer that arrives but will not parse is a failure, not an empty page.
#[tokio::test]
async fn an_unreadable_answer_is_a_failure() {
    let fake = Fake::by_route(vec![(
        Method::Get,
        "/settings/discover",
        Answer::reply(200, "{\"not\":\"a list\"}"),
    )]);

    assert!(
        seerr(&fake)
            .set_notices(&["anything".to_owned()])
            .await
            .is_err(),
        "a page this could not read was treated as a page with nothing on it, and \
         written over"
    );
}

/// A refusal at any of the three writes is a refusal.
#[tokio::test]
async fn a_refusal_at_any_write_is_a_refusal() {
    for refused in ["delete", "add", "write"] {
        let fake = Fake::by_route_in_turn(vec![
            (
                Method::Get,
                "/settings/discover",
                vec![Answer::reply(200, ROWS), Answer::reply(200, AFTER)],
            ),
            (
                Method::Delete,
                "/settings/discover/",
                vec![Answer::reply(
                    if refused == "delete" { 403 } else { 204 },
                    "",
                )],
            ),
            (
                Method::Post,
                "/settings/discover/add",
                vec![Answer::reply(
                    if refused == "add" { 403 } else { 200 },
                    "{}",
                )],
            ),
            (
                Method::Post,
                "/settings/discover",
                vec![Answer::reply(
                    if refused == "write" { 403 } else { 200 },
                    "[]",
                )],
            ),
        ]);

        assert!(
            seerr(&fake)
                .set_notices(&["something new".to_owned()])
                .await
                .is_err(),
            "a service that refused the {refused} reported the notice as hung"
        );
    }
}

/// A page that will not be read back after the rows are filed is a failure.
#[tokio::test]
async fn a_page_that_will_not_be_read_back_is_a_failure() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/discover",
            vec![Answer::reply(200, ROWS), Answer::Silent],
        ),
        (
            Method::Delete,
            "/settings/discover/",
            vec![Answer::reply(204, "")],
        ),
        (
            Method::Post,
            "/settings/discover/add",
            vec![Answer::reply(200, "{}")],
        ),
        (
            Method::Post,
            "/settings/discover",
            vec![Answer::reply(200, "[]")],
        ),
    ]);

    assert!(
        seerr(&fake)
            .set_notices(&["something new".to_owned()])
            .await
            .is_err(),
        "a page that stopped answering halfway through was reported as arranged"
    );
}
