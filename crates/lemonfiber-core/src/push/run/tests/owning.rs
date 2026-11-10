//! A member's own phone: registered, listed and removed by them alone, and woken only to
//! say something they asked for is ready.

use super::{alert, machine, owed, registering};
use crate::alert::Moment;
use crate::app::Outcome;
use crate::error::codes::push::PUSH_NOT_REGISTERED;
use crate::push::run::{asked, Asked, Waking};
use crate::push::telling::{owns_one, ready};

fn theirs(name: &str, owner: &str) -> Asked {
    match registering(name, "https://push.example/a") {
        Asked::Register {
            name,
            endpoint,
            p256dh,
            auth,
            ..
        } => Asked::Register {
            name,
            endpoint,
            p256dh,
            auth,
            owner: Some(owner.to_owned()),
        },
        other => other,
    }
}

fn listed(ctx: &crate::app::Ctx, owner: Option<&str>) -> Vec<String> {
    match asked(
        ctx,
        Asked::List {
            owner: owner.map(str::to_owned),
        },
    ) {
        Ok(Outcome::PushEndpoints(listing)) => {
            listing.endpoints.into_iter().map(|one| one.name).collect()
        }
        _ => Vec::new(),
    }
}

#[tokio::test]
async fn a_member_lists_and_removes_only_their_own_phone() {
    let (ctx, _) = machine("push-owned", 201);
    let _ = asked(
        &ctx,
        registering("operator-phone", "https://push.example/o"),
    );
    let _ = asked(&ctx, theirs("ada-phone", "ada"));

    assert_eq!(listed(&ctx, Some("ada")), ["ada-phone"]);
    assert_eq!(listed(&ctx, None), ["operator-phone", "ada-phone"]);
    assert!(owns_one(&ctx, "ada"));
    assert!(!owns_one(&ctx, "bo"));
    let stranger = asked(
        &ctx,
        Asked::Remove {
            name: "ada-phone".to_owned(),
            owner: Some("bo".to_owned()),
        },
    );
    assert_eq!(
        stranger.err().map(|problem| problem.code),
        Some(PUSH_NOT_REGISTERED)
    );
    let own = asked(
        &ctx,
        Asked::Remove {
            name: "ada-phone".to_owned(),
            owner: Some("ada".to_owned()),
        },
    );
    assert!(own.is_ok());
    assert_eq!(listed(&ctx, None), ["operator-phone"]);
}

#[tokio::test]
async fn a_members_phone_is_never_woken_for_the_stack_and_once_for_what_is_ready() {
    let (ctx, _) = machine("push-owned-woken", 201);
    let _ = asked(
        &ctx,
        registering("operator-phone", "https://push.example/o"),
    );
    let _ = asked(&ctx, theirs("ada-phone", "ada"));
    let ctx = std::sync::Arc::new(ctx);
    let waking = Waking::default();

    waking.owing(&ctx, &[]);
    waking.owing(&ctx, &[alert("jellyfin", Moment::Onset)]);
    let after_alert = owed(&ctx);
    ready(&ctx, "ada");

    assert_eq!(after_alert.get(1), Some(&0));
    assert_eq!(owed(&ctx).get(1), Some(&1));
}
