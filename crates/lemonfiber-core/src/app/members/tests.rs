use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{claim_spent, household, offers_claim, vouched_for};
use crate::app::Ctx;
use crate::ports::service::Signed;
use crate::test_support::{a_context, a_password, json, media, nowhere};

/// A context over the repository's stack, with an env file of the test's own, its media
/// server moved into its plugin where `moved`, and the server answering every sign-in as
/// the member it knows.
fn ctx_with(tag: &str, moved: bool) -> (Ctx, Arc<Transport>) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("members-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let signed = Some(Signed {
        id: "a7f3".to_owned(),
        token: "a-session".to_owned(),
    });
    let transport = Transport::by_path(vec![(
        media::identity("whoever"),
        Answer::reply(200, json(&signed)),
    )]);
    let mut context = a_context()
        .engine(Arc::new(media::running()))
        .build()
        .with_http(transport.clone());
    context.settings.env_file = Some(dir.join(".env"));
    if moved {
        media::moved_in(&context);
    }
    (context, transport)
}

/// The household that comes back is the moved server's, asked over its contracts at the
/// adapter the host reaches on its loopback.
#[tokio::test]
async fn a_stack_whose_media_server_moved_in_has_a_household_to_ask() {
    let (context, transport) = ctx_with("held", true);

    let Some(held) = household(&context).await else {
        unreachable!("a stack whose media server moved in has a household")
    };
    let signed = held.whoever("ana", &a_password(), "a-device").await;

    assert_eq!(
        signed.ok().flatten().map(|signed| signed.id).as_deref(),
        Some("a7f3")
    );
    assert!(transport.request().is_some_and(|asked| asked
        .url
        .starts_with(&format!("http://127.0.0.1:{}", media::PORT))
        && asked.url.ends_with(media::identity("whoever"))));
}

/// A stack whose media server speaks no contract has no household to ask.
#[tokio::test]
async fn a_stack_whose_media_server_has_not_moved_has_no_household() {
    let (context, _) = ctx_with("unmoved", false);

    assert!(household(&context).await.is_none());
}

#[tokio::test]
async fn a_stack_whose_manifest_cannot_be_read_has_no_household() {
    let (context, _) = ctx_with("unread", true);
    let context = Ctx {
        stack: nowhere(),
        ..context
    };

    assert!(household(&context).await.is_none());
}

/// A context whose record holds one offer for account `a7f3`, running out `lapses_in`
/// hours from now — in the past where negative.
fn offered(tag: &str, lapses_in: i64) -> Ctx {
    let (context, _) = ctx_with(tag, false);
    let offers: crate::invitation::Offers = [(
        "a7f3".to_owned(),
        crate::invitation::Offer {
            offered: context.hours_ago(48 - lapses_in),
            lapses: context.hours_ago(-lapses_in),
            decline: None,
            claim: None,
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

/// A context whose record holds an offer on `a7f3` lapsing `lapses_in` hours from now,
/// claimed by `a-claim` and declined by `a-refusal`, with the decline service's refusals
/// written under a stack of its own where `refused`.
fn claimable(tag: &str, lapses_in: i64, refused: bool) -> Ctx {
    use lemonfiber_sidecar::decline::{File, Refusal, Refusals, TokenHash};

    let context = offered(tag, lapses_in);
    let mut offers = on_record(&context);
    if let Some(offer) = offers.get_mut("a7f3") {
        offer.claim = Some(TokenHash::of("a-claim"));
        offer.decline = Some(TokenHash::of("a-refusal"));
    }
    crate::app::record::keep_beside(&context, crate::invitation::RECORD, &offers);
    let project =
        lemonfiber_fixtures::scratch::Scratch::named(&format!("members-{tag}-stack")).kept();
    lemonfiber_fixtures::stack::manifest_into(&project);
    if refused {
        let record = crate::app::invite::declining::path(&project, File::Refusals);
        let refusals = Refusals::default().with(Refusal {
            token: TokenHash::of("a-refusal"),
            account: "a7f3".to_owned(),
            at: 1,
        });
        let _ = std::fs::create_dir_all(record.parent().unwrap_or(project.as_path()));
        let _ = std::fs::write(&record, refusals.written());
    }
    let project: &'static std::path::Path = Box::leak(project.into_boxed_path());
    Ctx {
        stack: crate::stack::Source::External(project),
        ..context
    }
}

/// The token its offer carries claims an account's standing invitation, and nothing
/// else claims it.
#[tokio::test]
async fn only_the_token_an_offer_carries_claims_it() {
    let context = claimable("claim-open", 1, false);

    assert!(offers_claim(&context, "a7f3", "a-claim").await);
    assert!(!offers_claim(&context, "a7f3", "a-guess").await);
    assert!(!offers_claim(&context, "b9c1", "a-claim").await);
}

#[tokio::test]
async fn a_lapsed_invitation_claims_nothing() {
    let context = claimable("claim-lapsed", -1, false);

    assert!(!offers_claim(&context, "a7f3", "a-claim").await);
}

#[tokio::test]
async fn a_declined_invitation_claims_nothing() {
    let context = claimable("claim-declined", 1, true);

    assert!(!offers_claim(&context, "a7f3", "a-claim").await);
}

/// A claim spent is taken off the offer, which stays, so its token claims nothing again.
#[tokio::test]
async fn a_spent_claim_claims_nothing_again() {
    let context = claimable("claim-spent", 1, false);

    claim_spent(&context, "a7f3");
    claim_spent(&context, "b9c1");

    assert!(!offers_claim(&context, "a7f3", "a-claim").await);
    assert!(on_record(&context)
        .get("a7f3")
        .is_some_and(|offer| offer.claim.is_none() && offer.decline.is_some()));
}
