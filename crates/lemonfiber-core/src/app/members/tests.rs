use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{household, vouched_for};
use crate::app::Ctx;
use crate::test_support::{a_context, a_password, nowhere};

/// What the media server answers a member's name and password with.
const SIGNED_IN: &str = r#"{"AccessToken":"a-session","User":{"Id":"a7f3"}}"#;

/// A context over the repository's stack, with an env file of the test's own and the
/// media server answering every sign-in as the member it knows.
fn ctx_with(tag: &str, admin_password: Option<&str>) -> (Ctx, Arc<Transport>) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("members-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let transport = Transport::by_path(vec![(
        "/Users/AuthenticateByName",
        Answer::reply(200, SIGNED_IN),
    )]);
    let mut context = a_context().build().with_http(transport.clone());
    context.settings.env_file = Some(dir.join(".env"));
    if let Some(password) = admin_password {
        crate::app::targets::record_secret(
            &context,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            password,
        );
    }
    (context, transport)
}

/// The household that comes back is the stack's own media server, at the address the
/// manifest gives it.
#[tokio::test]
async fn a_stack_with_a_media_server_and_its_password_recorded_has_a_household_to_ask() {
    let (context, transport) = ctx_with("held", Some(&a_password()));

    let Some(held) = household(&context) else {
        unreachable!("a stack with a media server and its password recorded has a household")
    };
    let signed = held.whoever("ana", &a_password(), "a-device").await;

    assert_eq!(
        signed.ok().flatten().map(|signed| signed.id).as_deref(),
        Some("a7f3")
    );
    assert!(transport
        .request()
        .is_some_and(|asked| asked.url.starts_with("http://127.0.0.1:")
            && asked.url.ends_with("/Users/AuthenticateByName")));
}

/// Before the stack is seeded there is no admin password, and so no household.
#[test]
fn a_stack_with_no_admin_password_recorded_has_no_household() {
    let (context, _) = ctx_with("unseeded", None);

    assert!(household(&context).is_none());
}

#[test]
fn a_stack_whose_manifest_cannot_be_read_has_no_household() {
    let (context, _) = ctx_with("unread", Some(&a_password()));
    let context = Ctx {
        stack: nowhere(),
        ..context
    };

    assert!(household(&context).is_none());
}

/// A context whose record holds one offer for account `a7f3`, running out `lapses_in`
/// hours from now — in the past where negative.
fn offered(tag: &str, lapses_in: i64) -> Ctx {
    let (context, _) = ctx_with(tag, None);
    let offers: crate::invitation::Offers = [(
        "a7f3".to_owned(),
        crate::invitation::Offer {
            offered: context.hours_ago(48 - lapses_in),
            lapses: context.hours_ago(-lapses_in),
            decline: None,
        },
    )]
    .into_iter()
    .collect();
    crate::app::record::keep_beside(&context, crate::invitation::RECORD, &offers);
    context
}

/// What the record holds now.
fn on_record(context: &Ctx) -> crate::invitation::Offers {
    crate::app::record::beside(context, crate::invitation::RECORD)
}

/// An invitation that ran out before anybody was seen to take it up is refused, with
/// nothing having swept the household since.
#[test]
fn an_invitation_that_ran_out_unseen_is_not_vouched_for() {
    let context = offered("lapsed", -1);

    assert!(!vouched_for(&context, "a7f3"));
    assert!(
        on_record(&context).contains_key("a7f3"),
        "the refusal let go of what it rests on"
    );
}

/// A member signing in while their offer still stands has taken it up, and is never
/// judged against it again.
#[test]
fn signing_in_while_the_offer_stands_closes_it() {
    let context = offered("in-time", 1);

    assert!(vouched_for(&context, "a7f3"));
    assert!(on_record(&context).is_empty(), "the offer was not closed");
}

#[test]
fn an_account_never_offered_is_vouched_for() {
    let context = offered("never-offered", 1);

    assert!(vouched_for(&context, "b9c1"));
    assert!(on_record(&context).contains_key("a7f3"));
}
