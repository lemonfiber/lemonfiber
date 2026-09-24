//! The household's accounts: offered, withdrawn, reset and standing.

use crate::{about, hers, jellyfin, not_hers, reader, HOUSEHOLD, SIGNED_IN, SIGNED_IN_AS};
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::ports::service::Failure;
use lemonfiber_fixtures::http::{Answer, Fake};

#[tokio::test]
async fn an_account_without_a_password_reads_as_unclaimed() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, HOUSEHOLD),
    ]);

    let held = reader(&fake).household().await.unwrap_or_default();

    assert_eq!(
        held.iter()
            .map(|member| (member.name.as_str(), member.claimed))
            .collect::<Vec<_>>(),
        [("ana", false), ("bo", true)],
        "claimed and unclaimed were not told apart"
    );
}

/// Offering an account sends no password, which is what makes it an invitation.
#[tokio::test]
async fn offering_an_account_sends_no_password() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Id":"1","Name":"ana","HasPassword":false}"#),
    ]);

    let made = reader(&fake).invite("ana").await;

    assert!(made.is_ok_and(|member| member.name == "ana" && !member.claimed));
    let sent = fake
        .requests()
        .into_iter()
        .find(|asked| asked.url.ends_with("/Users/New"))
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        sent.contains(r#""Name":"ana""#) && !sent.to_lowercase().contains("password"),
        "a password was sent with an invitation: {sent}"
    );
}

/// Taking an account back asks the server to remove it.
#[tokio::test]
async fn withdrawing_an_invitation_removes_the_account() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);

    assert!(reader(&fake).withdraw("1").await.is_ok());
    assert!(
        fake.requests()
            .iter()
            .any(|asked| asked.method == Method::Delete && asked.url.ends_with("/Users/1")),
        "the account was not asked to be removed: {:?}",
        fake.requests()
    );
}

/// Resetting a password sends a flag and never a password.
///
/// **This is the structural half of the promise** that the operator cannot learn what
/// somebody's next password is: the request body has one field in it and that field is
/// a boolean. There is no key here a password could be put under by mistake, and the
/// server answers by putting the account back to having none — which is the state an
/// invitation leaves it in, so what comes back out of this is an invitation to send.
///
/// Verified against `jellyfin/jellyfin:10.10.3`: the same endpoint takes `CurrentPw`
/// and `NewPw` when somebody changes their own password, so naming neither is what
/// makes this a reset rather than a password chosen for them.
#[tokio::test]
async fn resetting_a_password_sends_a_flag_and_never_a_password() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);

    assert!(reader(&fake).unclaim("1").await.is_ok());

    let body = fake
        .requests()
        .into_iter()
        .find(|asked| asked.method == Method::Post && asked.url.ends_with("/Users/1/Password"))
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        body.contains(r#""ResetPassword":true"#),
        "no reset was asked for at the account's own password: {body:?}"
    );
    assert!(
        !body.to_lowercase().contains("pw") && body.matches(':').count() == 1,
        "the reset carried more than the flag, so a password could be among it: {body}"
    );
}

/// A server that refuses the reset is reported as a failure rather than a success.
///
/// Worth its own case because the answer carries no body either way: a `403` and a
/// `204` are told apart by the status alone, and reading only the body would make
/// every refusal look like it worked. What the operator would then send is an
/// invitation to an account whose old password still opens it.
#[tokio::test]
async fn a_refused_reset_is_not_reported_as_done() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(403, "")]);

    assert!(
        reader(&fake).unclaim("1").await.is_err(),
        "a refused reset was reported as having happened"
    );
}

/// When an account was offered comes from what the server recorded happening.
///
/// The account itself carries no date at all, so this reads the record instead. It keeps
/// **two** kinds of entry: an account being made, and a password moving on or off one.
/// Both are moments an account came to have no password on it, which is what an
/// invitation is — a reset offers an existing account again, months after it was made,
/// and dating that by its creation would have it expire before anybody was told.
///
/// Everything else the record holds is dropped: it also carries every sign-in and every
/// session, and an entry naming no account at all.
#[tokio::test]
async fn when_an_account_was_made_is_read_from_what_the_server_recorded() {
    let recorded = r#"{"Items":[
        {"Type":"UserCreated","Date":"2026-08-29T09:00:00Z","UserId":"1"},
        {"Type":"UserPasswordChanged","Date":"2026-08-30T09:00:00Z","UserId":"1"},
        {"Type":"AuthenticationSucceeded","Date":"2026-08-29T10:00:00Z","UserId":"2"},
        {"Type":"SessionStarted","Date":"2026-08-29T10:00:01Z","UserId":"2"},
        {"Type":"UserCreated","Date":"2026-08-29T11:00:00Z","UserId":""}
    ]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, recorded),
    ]);

    let made = reader(&fake)
        .when_invited("2026-08-27T09:00:00Z")
        .await
        .unwrap_or_default();

    assert_eq!(
        made.iter()
            .map(|entry| (entry.member.as_str(), entry.at.as_str()))
            .collect::<Vec<_>>(),
        [("1", "2026-08-29T09:00:00Z"), ("1", "2026-08-30T09:00:00Z")],
        "either an account being offered was dropped, or something that is not an \
         offering was carried through"
    );
    assert!(
        fake.requests().iter().any(
            |asked| asked.url.contains("minDate=2026-08-27T09%3A00%3A00Z")
                || asked.url.contains("minDate=2026-08-27T09:00:00Z")
        ),
        "the read was not bounded by the date asked for: {:?}",
        fake.requests()
    );
}

/// A media server that will not answer is unavailable, not an empty household.
#[tokio::test]
async fn a_media_server_that_will_not_answer_is_unavailable_for_the_household() {
    let fake = Fake::silent();
    assert!(matches!(
        reader(&fake).household().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_pair_the_server_knows_answers_with_the_account_it_proved() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN_AS)]);

    assert_eq!(
        jellyfin(&fake).whoever("ana", &hers()).await.ok().flatten(),
        Some("a7f3".to_owned())
    );
    // The member's own credentials, and the admin's nowhere near it: this is the one
    // call where somebody else's password travels, and it travels only to the route
    // where it is being proved.
    let asked: Vec<&str> = fake
        .requests()
        .iter()
        .map(|request| request.url.as_str())
        .filter(|url| url.ends_with("/Users/AuthenticateByName"))
        .map(|_| "authenticate")
        .collect();
    assert_eq!(asked, ["authenticate"], "the member was proved elsewhere");
}

#[tokio::test]
async fn a_pair_the_server_refuses_is_nobody_rather_than_a_fault() {
    // Both statuses, because a server tells an unknown account and a wrong password
    // apart and this deliberately does not.
    for refused in [401, 403] {
        let fake = Fake::in_turn(vec![Answer::reply(refused, r#"{"error":"no"}"#)]);

        let said = jellyfin(&fake).whoever("ana", &not_hers()).await;
        assert!(
            matches!(said, Ok(None)),
            "a refusal at {refused} was read as something other than nobody"
        );
    }
}

#[tokio::test]
async fn a_server_that_could_not_answer_is_not_a_pair_that_was_wrong() {
    // The distinction the nested shape exists for. Collapsed, this would tell
    // somebody their password was wrong for as long as the server was down.
    let fake = Fake::in_turn(vec![Answer::reply(500, "upstream is unwell")]);

    assert!(
        jellyfin(&fake).whoever("ana", &hers()).await.is_err(),
        "a server that could not answer was read as a pair that was wrong"
    );
}

#[tokio::test]
async fn an_account_with_no_id_is_nobody() {
    // An empty id would match every other account that answered the same way, so it
    // is read as nobody rather than as somebody with no name.
    let fake = Fake::in_turn(vec![Answer::reply(200, r#"{"AccessToken":"t","User":{}}"#)]);

    assert!(matches!(
        jellyfin(&fake).whoever("ana", &hers()).await,
        Ok(None)
    ));
}

/// An account the server holds, with the flag that says it is not switched off.
const STILL_HELD: &str =
    r#"{"Id":"a7f3","Name":"ana","HasPassword":true,"Policy":{"IsDisabled":false}}"#;

/// The same account, switched off at the server rather than removed from it.

const SWITCHED_OFF: &str =
    r#"{"Id":"a7f3","Name":"ana","HasPassword":true,"Policy":{"IsDisabled":true}}"#;

/// One account read, rather than the household listed to look for it.

#[tokio::test]
async fn an_account_the_server_still_holds_is_standing() {
    let fake = about(Answer::reply(200, STILL_HELD));

    assert!(matches!(reader(&fake).standing("a7f3").await, Ok(true)));
}

#[tokio::test]
async fn an_account_the_server_no_longer_holds_is_not_standing() {
    // The answer this question exists for, and the reason it is `Ok` rather than an
    // error: a removed account is a fact the server stated, not a failure to answer.
    let fake = about(Answer::reply(404, r#"{"error":"no such user"}"#));

    assert!(matches!(reader(&fake).standing("a7f3").await, Ok(false)));
}

#[tokio::test]
async fn an_account_switched_off_at_the_server_is_not_standing() {
    let fake = about(Answer::reply(200, SWITCHED_OFF));

    assert!(matches!(reader(&fake).standing("a7f3").await, Ok(false)));
}

#[tokio::test]
async fn a_server_that_would_not_say_is_not_an_account_that_is_gone() {
    // The distinction the nested shape exists for, one floor up from the sign-in.
    // Collapsed, a media server restarting would sign out every member and tell each
    // of them their account had been removed.
    let fake = about(Answer::reply(500, "upstream is unwell"));

    assert!(
        reader(&fake).standing("a7f3").await.is_err(),
        "a server that could not answer was read as an account that is gone"
    );
}

#[tokio::test]
async fn an_account_answered_without_a_policy_is_one_the_server_still_holds() {
    // Existence is what this was asked, and the server answered it. Reading a missing
    // flag as *switched off* would lock out everybody the day the field stopped
    // arriving — permanently, since the next call reads the same answer.
    let fake = about(Answer::reply(200, r#"{"Id":"a7f3","Name":"ana"}"#));

    assert!(matches!(reader(&fake).standing("a7f3").await, Ok(true)));
}
