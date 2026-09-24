//! A plugin's secrets in the inventory.

use crate::{asked, beside, ctx, recorded, silent, WITH_A_SECRET};
use lemonfiber_core::app::{dispatch, Asking, Command};
use lemonfiber_core::credential::{fingerprint, Rotation, State};
use lemonfiber_fixtures::files::Files;

/// A plugin's declared secret is on the same list as every other, saying whose it is,
/// that nothing holds it yet, and carrying no likeness of a value there is not.
#[tokio::test]
async fn a_plugins_secret_is_listed_as_the_plugins_and_as_held_by_nothing() {
    let ctx = ctx(beside("plugin", WITH_A_SECRET), Files::empty(), silent());

    let inventory = asked(&ctx, Asking::Read).await;

    let line = inventory
        .held
        .iter()
        .find(|one| one.plugins() == Some("comics"));
    assert_eq!(
        line.map(|one| (one.name.as_str(), one.state, one.fingerprint.is_none())),
        Some(("comics api_key", State::Absent, true)),
        "{:?}",
        inventory.held
    );
    assert_eq!(
        line.map(|one| one.consumers.clone()),
        Some(vec!["komga".to_owned()])
    );
    assert!(
        inventory
            .advisories()
            .iter()
            .any(|said| said.contains("The plugin comics declared")),
        "{:?}",
        inventory.advisories()
    );
}

/// Asked for, a plugin's secret says nothing holds it rather than pointing at seeding
/// or printing anything.
#[tokio::test]
async fn asking_to_see_a_plugins_secret_says_nothing_holds_it() {
    let ctx = ctx(
        beside("pluginreveal", WITH_A_SECRET),
        Files::empty(),
        silent(),
    );

    let inventory = asked(
        &ctx,
        Asking::Reveal {
            credential: "comics api_key".to_owned(),
            confirmed: true,
        },
    )
    .await;

    let revealed = inventory.revealed;
    assert_eq!(revealed.as_ref().and_then(|one| one.value.clone()), None);
    let said = revealed.map(|one| one.warning).unwrap_or_default();
    assert!(said.contains("nothing holds it yet"), "{said}");
}

/// A rotation of a plugin's secret writes nothing and says there is nothing to
/// replace, rather than taking the path a service's own key takes.
#[tokio::test]
async fn rotating_a_plugins_secret_replaces_nothing_and_says_why() {
    let env = beside("pluginrotate", WITH_A_SECRET);
    let ctx = ctx(env.clone(), Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "comics api_key".to_owned(),
        },
    )
    .await;

    let rotated = inventory.rotated;
    assert!(
        rotated.as_ref().is_some_and(Rotation::kept_the_existing),
        "{rotated:?}"
    );
    assert!(
        format!("{rotated:?}").contains("nothing holds it yet"),
        "{rotated:?}"
    );
    assert_eq!(
        recorded(&env, "comics/api_key"),
        None,
        "nothing was written"
    );
}

/// A register that is there and will not read refuses the inventory rather than
/// being read past: a list of what this stack holds with a plugin's secrets quietly
/// missing from it would be believed.
#[tokio::test]
async fn a_register_that_will_not_read_refuses_the_inventory() {
    let ctx = ctx(
        beside("pluginbroken", "{ not a register"),
        Files::empty(),
        silent(),
    );

    let refused = dispatch(Command::Credentials(Asking::Read), &ctx).await;

    assert_eq!(
        refused.err().map(|problem| problem.code.to_string()),
        Some("PLUGIN-4".to_owned())
    );
}
