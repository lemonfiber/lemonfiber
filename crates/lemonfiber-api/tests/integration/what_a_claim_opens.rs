//! Claiming an invitation at the door: the password the person chose set on the account
//! the invitation names, and their session opened, or one refusal for every invitation
//! that is not open.

use crate::door;
use door::*;
use lemonfiber_api::admission::remembered::Remembered;
use lemonfiber_fixtures::http::Answer as Reply;

/// The claim token an invitation's join link carries.
const CLAIM: &str = "the-claim-on-ana";

/// Its SHA-256, as the record of offers keeps it.
const CLAIM_HASHED: &str = "4937ceb5cd32fb8486f0e47e678405b43c5893c216c71e1aa81ac11be22a0ddf";

/// Ana's account, nobody having set its password yet.
fn unclaimed() -> Vec<Member> {
    vec![media::member("a7f3", "ana", false)]
}

/// A media server holding `household`, answering the first password set on an account
/// with `claim`.
fn holding(household: &[Member], claim: Reply) -> Arc<Fake> {
    Fake::by_path(vec![
        (media::identity("household"), media::answered(&household)),
        (media::identity("claim"), claim),
        (media::identity("whoever"), recognised()),
        (media::identity("standing"), standing()),
    ])
}

/// A stack named `named` over `transport`, seeded, with Ana's invitation on record and
/// claimable by [`CLAIM`] until long after the stopped clock.
fn offered(named: &str, transport: Arc<Fake>) -> Ctx {
    let ctx = a_stack(named, transport);
    seeded(&ctx);
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a stack built here has an env file")
    };
    let record = format!(
        r#"{{"a7f3":{{"offered":"2023-11-14T00:00:00Z","lapses":"2999-01-01T00:00:00Z","claim":"{CLAIM_HASHED}"}}}}"#
    );
    let Ok(()) = fs::write(env.with_file_name("invitations.json"), record) else {
        unreachable!("a scratch directory can be written")
    };
    ctx
}

/// A claim as the app sends it.
fn claiming(password: &str, claim: &str) -> String {
    format!(
        "{{\"name\":\"Ana\",\"password\":{},\"claim\":{}}}",
        serde_json::json!(password),
        serde_json::json!(claim)
    )
}

async fn claimed(router: axum::Router, password: &str, claim: &str) -> door::Answer {
    asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &claiming(password, claim),
    )
    .await
}

/// A password one character shorter than the least the core takes, built rather than
/// written.
fn too_short() -> String {
    ('a'..='z').take(credential::LEAST - 1).collect()
}

/// Whether the media server was asked to set a password.
fn set_a_password(transport: &Fake) -> bool {
    !media::asked(&transport.requests(), "claim").is_empty()
}

fn the_code(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|said| said.pointer("/data/code")?.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[tokio::test]
async fn a_claim_on_an_open_invitation_sets_the_password_and_opens_a_session() {
    let transport = holding(&unclaimed(), media::answered(&true));
    let ctx = offered("claim-opens", transport.clone());
    let (router, admitting) = door_over(&ctx);

    let answer = claimed(router, &hers(), CLAIM).await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(set_a_password(&transport));
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
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a stack built here has an env file")
    };
    let record = fs::read_to_string(env.with_file_name("invitations.json")).unwrap_or_default();
    assert!(
        !record.contains(CLAIM_HASHED),
        "the claim was not spent: {record}"
    );
    let _ = fs::remove_dir_all(a_directory("claim-opens"));
}

#[tokio::test]
async fn a_token_the_invitation_does_not_carry_is_refused_and_sets_nothing() {
    let transport = holding(&unclaimed(), media::answered(&true));
    let ctx = offered("claim-guessed", transport.clone());
    let (router, _) = door_over(&ctx);

    let answer = claimed(router, &hers(), "a-guess").await;

    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert_eq!(the_code(&answer.body), "ADMIT-13");
    assert!(session(&answer.body).is_empty());
    assert!(!set_a_password(&transport));
    let _ = fs::remove_dir_all(a_directory("claim-guessed"));
}

/// A password shorter than the least the core takes is refused before anything is
/// asked of the media server, naming that least.
#[tokio::test]
async fn a_short_password_is_refused_before_the_media_server_is_asked() {
    let transport = holding(&unclaimed(), media::answered(&true));
    let ctx = offered("claim-short", transport.clone());
    let (router, _) = door_over(&ctx);

    let answer = claimed(router, &too_short(), CLAIM).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", answer.body);
    assert_eq!(the_code(&answer.body), "ADMIT-14");
    assert!(
        answer
            .body
            .contains(&format!("at least {} characters", credential::LEAST)),
        "{}",
        answer.body
    );
    assert!(transport.requests().is_empty());
    let _ = fs::remove_dir_all(a_directory("claim-short"));
}

#[tokio::test]
async fn an_account_somebody_already_claimed_is_not_claimed_again() {
    let transport = holding(&household(), media::answered(&true));
    let ctx = offered("claim-taken", transport.clone());
    let (router, _) = door_over(&ctx);

    let answer = claimed(router, &hers(), CLAIM).await;

    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert_eq!(the_code(&answer.body), "ADMIT-13");
    assert!(!set_a_password(&transport));
    let _ = fs::remove_dir_all(a_directory("claim-taken"));
}

/// The media server setting no password is an account that is claimed or switched off by
/// the time the claim reaches it: refused, and the claim kept.
#[tokio::test]
async fn an_account_the_media_server_will_not_sign_in_empty_is_refused() {
    let transport = holding(&unclaimed(), media::answered(&false));
    let ctx = offered("claim-closed", transport.clone());
    let (router, _) = door_over(&ctx);

    let answer = claimed(router, &hers(), CLAIM).await;

    assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{}", answer.body);
    assert!(session(&answer.body).is_empty());
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a stack built here has an env file")
    };
    let record = fs::read_to_string(env.with_file_name("invitations.json")).unwrap_or_default();
    assert!(record.contains(CLAIM_HASHED), "{record}");
    let _ = fs::remove_dir_all(a_directory("claim-closed"));
}

/// A media server that cannot be asked is said as that, not as an invitation that is
/// closed, so nobody is sent to ask for a new one.
#[tokio::test]
async fn a_household_that_cannot_be_read_claims_nothing() {
    let transport = Fake::by_path(vec![(media::identity("household"), media::refusing())]);
    let ctx = offered("claim-unread", transport.clone());
    let (router, _) = door_over(&ctx);

    let answer = claimed(router, &hers(), CLAIM).await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(the_code(&answer.body), "ADMIT-7");
    assert!(!set_a_password(&transport));
    let _ = fs::remove_dir_all(a_directory("claim-unread"));
}

/// A password the media server will not take is said as the media server not
/// answering, and the claim is kept for another try.
#[tokio::test]
async fn a_password_the_media_server_will_not_take_keeps_the_claim() {
    let transport = holding(&unclaimed(), media::refusing());
    let ctx = offered("claim-refused", transport.clone());
    let (router, _) = door_over(&ctx);

    let answer = claimed(router, &hers(), CLAIM).await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(the_code(&answer.body), "ADMIT-7");
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a stack built here has an env file")
    };
    let record = fs::read_to_string(env.with_file_name("invitations.json")).unwrap_or_default();
    assert!(record.contains(CLAIM_HASHED), "{record}");
    let _ = fs::remove_dir_all(a_directory("claim-refused"));
}

/// A household handed to the door keeps no record of what it offered, so nothing it
/// holds is claimable and nothing is spent.
#[tokio::test]
async fn a_household_keeping_no_record_offers_no_claim() {
    let at: Arc<dyn Household> = AHousehold::knowing("a7f3");

    assert!(!at.offers_claim("a7f3", CLAIM).await);
    at.claim_spent("a7f3");
}

/// A household kept for a while asks the stack's own record about a claim each time.
#[tokio::test]
async fn a_household_kept_for_a_while_asks_the_record_about_a_claim() {
    let ctx = offered(
        "claim-remembered",
        holding(&unclaimed(), media::answered(&true)),
    );
    let kept = Remembered::over(Arc::new(ctx.clone()));

    assert!(kept.offers_claim("a7f3", CLAIM).await);
    kept.claim_spent("a7f3");
    assert!(!kept.offers_claim("a7f3", CLAIM).await);
    let _ = fs::remove_dir_all(a_directory("claim-remembered"));
}
