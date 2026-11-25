use std::sync::Arc;

use lemonfiber_fixtures::ports::Chance;

use super::{standing, StackStanding};
use crate::app::{dispatch, Command, Ctx, Outcome, Surfacing};
use crate::config::{Reaching, Settings, MEMBER_KEYS_KEY, REACH_REGISTRY_KEY};
use crate::test_support::a_context;

/// A machine keeping everything in a directory of this case's own, with `reaching` as
/// what it may reach.
fn a_machine(named: &str, reaching: Reaching) -> Ctx {
    let scratch = lemonfiber_fixtures::scratch::Scratch::named(&format!("standing-{named}"));
    let dir = scratch.to_path_buf();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    a_context()
        .holding(scratch)
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            stack_dir: Some(dir.join("stack")),
            companion: Some(dir.join("companion")),
            reaching,
            ..Settings::default()
        })
        .build()
        .with_random(Arc::new(Chance::cycling()))
}

#[test]
fn a_machine_with_nothing_configured_stands_with_every_predicate_off() {
    let ctx = a_machine("bare", Reaching::default());

    let read = standing(&ctx);

    assert!(read.stack.is_some(), "{read:?}");
    assert_eq!(
        read,
        StackStanding {
            stack: read.stack.clone(),
            switched_off: Vec::new(),
            told: false,
            waiting: false,
            members_mint: false,
        }
    );
}

#[test]
fn what_the_operator_switched_on_and_off_is_read_as_it_stands() {
    let ctx = a_machine("set", Reaching::without(REACH_REGISTRY_KEY));
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(crate::config::store::set(&env, MEMBER_KEYS_KEY, "on").is_ok());

    let read = standing(&ctx);

    assert_eq!(read.switched_off, [REACH_REGISTRY_KEY]);
    assert!(read.members_mint);
}

#[tokio::test]
async fn the_standing_is_dispatched_like_any_other_read() {
    let ctx = a_machine("dispatched", Reaching::default());

    let dispatched = dispatch(Command::Surface(Surfacing::Standing), &ctx).await;

    assert_eq!(
        dispatched,
        Ok(Outcome::Surfaced(crate::app::surfacing::Answer::Standing(
            standing(&ctx)
        )))
    );
}
