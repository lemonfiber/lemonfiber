use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::household;
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
