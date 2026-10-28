//! A plugin's secrets in the inventory.

use super::{asked, beside, ctx, recorded, silent, WITH_A_SECRET};
use lemonfiber_core::app::{dispatch, Asking, Command};
use lemonfiber_core::config::store;
use lemonfiber_core::credential::{Origin, Rotation, Settled, State};
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
            agreement: None,
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

/// A plugin's secret its recipe captured is read from the secret store: held there, with
/// a likeness of its value and no advisory, and printed only when asked for and confirmed.
#[tokio::test]
async fn a_captured_plugin_secret_is_read_from_the_secret_store() {
    let env = beside("pluginheld", WITH_A_SECRET);
    let secrets = env.with_file_name("secrets.json");
    assert!(std::fs::write(
        &secrets,
        r#"{"schema":1,"written_by":"","held":{"comics:api_key":{"value":"v4lue","from":"recipe","at":1}}}"#
    )
    .is_ok());
    let ctx = ctx(env, Files::empty(), silent());

    let inventory = asked(&ctx, Asking::Read).await;

    let line = inventory
        .held
        .iter()
        .find(|one| one.plugins() == Some("comics"));
    assert_eq!(
        line.map(|one| (
            one.setting.as_str(),
            one.state,
            one.fingerprint.clone(),
            one.advisory.is_none(),
            one.location == secrets.display().to_string()
        )),
        Some((
            "comics:api_key",
            State::Active,
            Some(lemonfiber_core::credential::fingerprint("v4lue")),
            true,
            true
        )),
        "{:?}",
        inventory.held
    );
    assert!(!format!("{inventory:?}").contains("v4lue"));

    let revealed = asked(
        &ctx,
        Asking::Reveal {
            credential: "comics api_key".to_owned(),
            confirmed: true,
        },
    )
    .await
    .revealed;
    assert_eq!(revealed.and_then(|one| one.value).as_deref(), Some("v4lue"));
}

/// A plugin's secret that is another service's credential is never kept for it: its line
/// says whose it is, stands as that credential stands, and carries no likeness of it.
#[tokio::test]
async fn a_crossing_secret_is_read_from_its_holder_and_never_kept() {
    let env = beside(
        "plugincrossing",
        &WITH_A_SECRET.replace(r#""of": "komga""#, r#""of": "sonarr""#),
    );
    let absent = asked(&ctx(env.clone(), Files::empty(), silent()), Asking::Read).await;
    assert!(store::set(&env, "LIBRARY_CURATE_TV_API_KEY", "abc").is_ok());

    let inventory = asked(&ctx(env, Files::empty(), silent()), Asking::Read).await;

    let line = |inventory: &lemonfiber_core::credential::Inventory| {
        inventory
            .held
            .iter()
            .find(|one| one.plugins() == Some("comics"))
            .map(|one| (one.state, one.location.clone(), one.fingerprint.is_none()))
    };
    let said = "not kept — it is sonarr's, and read from there".to_owned();
    assert_eq!(line(&absent), Some((State::Absent, said.clone(), true)));
    assert_eq!(line(&inventory), Some((State::Active, said, true)));
}

/// A secret lemonfiber mints for a plugin is lemonfiber's on the list, said to be minted
/// at install while nothing holds it, and rotating it mints one and holds it.
#[tokio::test]
async fn a_minted_plugin_secret_is_lemonfibers_and_rotating_it_holds_a_new_one() {
    let env = beside(
        "pluginminted",
        &WITH_A_SECRET.replace(r#""of": "komga","#, r#""of": "komga", "minted": true,"#),
    );
    let ctx = ctx(env.clone(), Files::empty(), silent());

    let listed = asked(&ctx, Asking::Read).await;
    let line = listed
        .held
        .iter()
        .find(|one| one.plugins() == Some("comics"));
    assert!(
        line.is_some_and(|one| one.origin == Origin::Lemonfiber
            && one
                .location
                .contains("mints it when the plugin is installed")),
        "{:?}",
        listed.held
    );

    let rotated = asked(
        &ctx,
        Asking::Rotate {
            credential: "comics api_key".to_owned(),
            agreement: None,
        },
    )
    .await
    .rotated;
    assert!(
        rotated
            .as_ref()
            .is_some_and(|rotation| matches!(rotation.settled, Settled::Replaced { .. })),
        "{rotated:?}"
    );
    let held = std::fs::read_to_string(env.with_file_name("secrets.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|store| {
            store
                .pointer("/held/comics:api_key/value")
                .and_then(serde_json::Value::as_str)
                .map(str::len)
        });
    assert_eq!(held, Some(32));
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

/// The setting sonarr's credential is held under, granted to the plugin in [`granted`].
const GRANTED: &str = "LIBRARY_CURATE_TV_API_KEY";

/// The register beside [`WITH_A_SECRET`] with its secret sonarr's credential and that
/// credential granted to the plugin.
fn granted() -> String {
    WITH_A_SECRET
        .replace(
            r#""of": "komga""#,
            r#""of": "sonarr", "for": "media.serve""#,
        )
        .replace(
            r#""declared": {"#,
            &format!(
                r#""granted": [
        {{ "credential": "{GRANTED}", "of": "sonarr", "for": "media.serve", "at": "1" }}
      ],
      "declared": {{"#
            ),
        )
}

/// A grant ended through the credentials ask is offered first and ends only over that
/// offer, leaving the inventory with no grant; a grant the plugin does not hold is refused.
#[tokio::test]
async fn a_grant_is_revoked_over_its_offer_and_one_not_held_is_refused() {
    let env = beside("pluginrevoke", &granted());
    assert!(store::set(&env, GRANTED, "abc").is_ok());
    let ctx = ctx(env, Files::empty(), silent());
    let revoking = |credential: &str, agreement: Option<String>| Asking::Revoke {
        plugin: "comics".to_owned(),
        credential: credential.to_owned(),
        agreement,
    };

    let offered = asked(&ctx, revoking(GRANTED, None)).await;
    let agreement = offered
        .revoked
        .as_ref()
        .map(|revoked| revoked.agreement.clone());
    let ended = asked(&ctx, revoking(GRANTED, agreement)).await;
    let refused = dispatch(
        Command::Credentials(revoking("LIBRARY_CURATE_MOVIES_API_KEY", None)),
        &ctx,
    )
    .await;

    assert!(
        offered.grants.len() == 1
            && offered
                .revoked
                .as_ref()
                .is_some_and(|revoked| !revoked.ended),
        "{offered:?}"
    );
    assert!(
        ended.grants.is_empty() && ended.revoked.as_ref().is_some_and(|revoked| revoked.ended),
        "{ended:?}"
    );
    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::error::codes::plugin::NO_GRANT)
    );
}
