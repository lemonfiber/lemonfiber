//! The door as the web surface assembles it, over the household the stack holds.
//!
//! `door` proves the door against a household a test hands it. These prove the other
//! half: that the household the door asks is the stack's own media server, reached at
//! the address the stack gives it, and opened from what the stack holds when somebody
//! knocks.

use std::fs;

use crate::door;
use door::*;
use lemonfiber_fixtures::http::Answer as Reply;

/// A media server that recognises the member, and says their account stands.
fn recognising() -> Arc<Fake> {
    Fake::by_path(vec![
        ("/Users/AuthenticateByName", Reply::reply(200, SIGNED_IN)),
        ("/Users/Me", Reply::reply(200, STANDING)),
    ])
}

/// A media server that recognises nobody.
fn refusing() -> Arc<Fake> {
    Fake::by_path(vec![("/Users/AuthenticateByName", Reply::reply(401, ""))])
}

/// A member's name and password, sent to the door.
async fn knocked(router: axum::Router, password: &str) -> door::Answer {
    asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &offering_as("ana", password),
    )
    .await
}

#[tokio::test]
async fn a_member_the_media_server_recognises_is_admitted_as_that_member() {
    let transport = recognising();
    let ctx = a_stack("stack-admits", transport.clone());
    seeded(&ctx);
    let (router, admitting) = door_over(&ctx);

    let answer = knocked(router, &hers()).await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        transport
            .requests()
            .iter()
            .any(|request| request.url.starts_with("http://127.0.0.1:")
                && request.url.ends_with("/Users/AuthenticateByName")
                && request
                    .body
                    .as_deref()
                    .is_some_and(|sent| sent.contains("\"ana\""))),
        "the name was not offered to the stack's media server"
    );
    let Some(token) = Token::mint(&Chance::exactly(Some(vec![b'q'; 32]))) else {
        unreachable!("bytes of the minting width mint a token")
    };
    assert_eq!(
        admitting
            .carried(
                &carrying(Some(&session(&answer.body))),
                &token,
                None,
                moment()
            )
            .await,
        Knocking::Known(Caller::Member("a7f3".to_owned()))
    );
    let _ = fs::remove_dir_all(a_directory("stack-admits"));
}

#[tokio::test]
async fn a_pair_the_media_server_does_not_recognise_is_refused() {
    let ctx = a_stack("stack-refuses", refusing());
    seeded(&ctx);
    let (router, _) = door_over(&ctx);

    let answer = knocked(router, &nobodys()).await;

    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert!(session(&answer.body).is_empty());
    let _ = fs::remove_dir_all(a_directory("stack-refuses"));
}

/// The household is opened from the stack when somebody knocks, so a stack seeded
/// while the surface was already serving admits its members from then on.
#[tokio::test]
async fn a_stack_seeded_while_serving_admits_its_members_from_then_on() {
    let ctx = a_stack("stack-seeded-later", recognising());
    let (router, _) = door_over(&ctx);

    let before = knocked(router.clone(), &hers()).await;
    seeded(&ctx);
    let after = knocked(router, &hers()).await;

    assert_eq!(before.status, StatusCode::UNAUTHORIZED, "{}", before.body);
    assert_eq!(after.status, StatusCode::OK, "{}", after.body);
    let _ = fs::remove_dir_all(a_directory("stack-seeded-later"));
}

/// Write down that the member's account was offered as an invitation running out at
/// `lapses`, the way inviting them does.
fn offered_until(ctx: &Ctx, lapses: &str) {
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a stack built here has an env file")
    };
    let record = format!(r#"{{"a7f3":{{"offered":"2019-12-30T00:00:00Z","lapses":"{lapses}"}}}}"#);
    let Ok(()) = fs::write(env.with_file_name("invitations.json"), record) else {
        unreachable!("a scratch directory can be written")
    };
}

/// An invitation claimed after it ran out is refused at the door, with nothing having
/// swept the household or taken it back at the media server: the media server signs
/// whoever claimed it in, and the door still refuses them, the way it refuses a wrong
/// password.
#[tokio::test]
async fn an_invitation_claimed_after_it_ran_out_is_refused_with_no_sweep() {
    let transport = recognising();
    let ctx = a_stack("stack-lapsed", transport.clone());
    seeded(&ctx);
    offered_until(&ctx, "2020-01-01T00:00:00Z");
    let (router, _) = door_over(&ctx);

    let answer = knocked(router, &hers()).await;

    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert!(session(&answer.body).is_empty());
    assert!(
        transport.requests().iter().all(|request| request.method
            != lemonfiber_core::ports::http::Method::Delete
            && !request.url.ends_with("/Policy")),
        "the refusal rested on something being taken back"
    );
    let _ = fs::remove_dir_all(a_directory("stack-lapsed"));
}

/// A member who signs in while their invitation stands has taken it up, and the offer is
/// closed so they are never judged against it again.
#[tokio::test]
async fn signing_in_while_an_invitation_stands_admits_and_closes_it() {
    let ctx = a_stack("stack-in-time", recognising());
    seeded(&ctx);
    offered_until(&ctx, "2999-01-01T00:00:00Z");
    let (router, _) = door_over(&ctx);

    let answer = knocked(router, &hers()).await;
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a stack built here has an env file")
    };
    let record = fs::read_to_string(env.with_file_name("invitations.json")).unwrap_or_default();

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        !record.contains("a7f3"),
        "the offer was not closed: {record}"
    );
    let _ = fs::remove_dir_all(a_directory("stack-in-time"));
}
