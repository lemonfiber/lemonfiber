//! Replacing the upstream key lemonfiber minted, where the service's settings hold it now.

use super::*;
use crate::credential::{Reach, Rotation, Settled};

/// The minted key's credential, as an operator names it.
const CREDENTIAL: &str = "komga upstream-key";

/// What rotating the minted key comes to in `ctx`.
async fn rotating(ctx: &Ctx) -> Option<Rotation> {
    crate::app::credentials::credentials(
        ctx,
        crate::app::command::Asking::Rotate {
            credential: CREDENTIAL.to_owned(),
            agreement: None,
        },
    )
    .await
    .ok()
    .and_then(|inventory| inventory.rotated)
}

/// Where the seeded upstream keeps its settings in `ctx`'s stack.
fn starting_of(ctx: &Ctx) -> PathBuf {
    stack_of(ctx).join("config/komga/application.yml")
}

/// The upstream's settings once it has run, holding `key`.
fn settled(key: &str) -> String {
    format!("server:\n  port: 25600\nkomga:\n  api-key: {key}\n  remember: true\n")
}

/// A context the seeded plugin is installed into, its upstream's settings rewritten as a
/// run leaves them, with the key it minted.
async fn installed(name: &str) -> (Ctx, String) {
    let ctx = seeding(name);
    let _ = installing(&ctx, &shipped(name)).await;
    let key = minted(&ctx).unwrap_or_default();
    let _ = std::fs::write(starting_of(&ctx), settled(&key));
    (ctx, key)
}

/// What a refused rotation said, or nothing where it was not refused.
fn refusal(rotation: Option<&Rotation>) -> Option<String> {
    match &rotation?.settled {
        Settled::Refused { detail } => Some(detail.clone()),
        _ => None,
    }
}

/// How far `rotation` reached `consumer`, where it names it.
fn reach(rotation: Option<&Rotation>, consumer: &str) -> Option<Reach> {
    rotation?
        .consumers
        .iter()
        .find(|one| one.consumer == consumer)
        .map(|one| one.reach.clone())
}

#[tokio::test]
async fn a_replaced_upstream_key_is_written_where_the_old_one_stands_and_handed_to_the_adapter() {
    let (ctx, before) = installed("seeded-replaced").await;

    let rotated = rotating(&ctx).await;
    assert!(
        rotated
            .as_ref()
            .is_some_and(|rotation| matches!(rotation.settled, Settled::Replaced { .. })),
        "{rotated:?}"
    );

    let after = minted(&ctx).unwrap_or_default();
    assert_eq!(after.len(), 32);
    assert_ne!(after, before);
    assert_eq!(read(&starting_of(&ctx)), settled(&after));
    assert_eq!(
        read(&stack_of(&ctx).join("config/komga-adapter/upstream.json")),
        format!(r#"{{"api_key":"{after}"}}"#)
    );
    assert_eq!(
        reach(rotated.as_ref(), "komga-adapter"),
        Some(Reach::Updated)
    );
    assert!(journalled(&ctx).iter().any(|change| matches!(
        &change.kind,
        crate::journal::Kind::Secret { name, previous: Some(_), .. } if name == "komga:upstream-key"
    )));
}

#[tokio::test]
async fn a_replacement_whose_line_the_service_took_away_is_refused_and_keeps_the_old_key() {
    let (ctx, before) = installed("seeded-unreplaced").await;
    let _ = std::fs::write(starting_of(&ctx), "server:\n  port: 25600\n");

    let rotated = rotating(&ctx).await;
    assert!(
        refusal(rotated.as_ref()).is_some_and(|said| said.contains("application.yml")),
        "{rotated:?}"
    );
    assert_eq!(minted(&ctx), Some(before));
    assert_eq!(read(&starting_of(&ctx)), "server:\n  port: 25600\n");
}

#[tokio::test]
async fn a_replacement_whose_settings_file_is_gone_is_refused_and_keeps_the_old_key() {
    let (ctx, before) = installed("seeded-replaced-gone").await;
    let _ = std::fs::remove_file(starting_of(&ctx));

    let rotated = rotating(&ctx).await;
    assert!(
        refusal(rotated.as_ref()).is_some_and(|said| said.contains("is not there")),
        "{rotated:?}"
    );
    assert_eq!(minted(&ctx), Some(before));
}

#[cfg(unix)]
#[tokio::test]
async fn a_replacement_whose_settings_file_is_a_link_is_refused_and_writes_nothing_through_it() {
    let (ctx, before) = installed("seeded-replaced-linked").await;
    let elsewhere = lemonfiber_fixtures::scratch::Scratch::new("seeded-replaced-elsewhere");
    let target = elsewhere.path().join("application.yml");
    let _ = std::fs::write(&target, settled(&before));
    let _ = std::fs::remove_file(starting_of(&ctx));
    let _ = std::os::unix::fs::symlink(&target, starting_of(&ctx));

    let rotated = rotating(&ctx).await;
    assert!(
        refusal(rotated.as_ref()).is_some_and(|said| said.contains("reached through a link")),
        "{rotated:?}"
    );
    assert_eq!(minted(&ctx), Some(before.clone()));
    assert_eq!(read(&target), settled(&before));
}

#[tokio::test]
async fn a_service_reading_the_key_from_its_environment_is_handed_it_in_its_secrets_file() {
    let name = "seeded-replaced-environment";
    let ctx = seeding(name);
    let at = shipped(name);
    let _ = std::fs::write(
        at.join("plugin.toml"),
        seeded().replacen(
            "config_path = \"/app/data\"\n",
            "config_path = \"/app/data\"\n\n[service.env]\nKOMGA_TZ = \"UTC\"\nKOMGA_KEY = { secret = \"upstream-key\" }\n",
            1,
        ),
    );
    let _ = installing(&ctx, &at).await;
    let before = minted(&ctx).unwrap_or_default();
    let _ = std::fs::write(starting_of(&ctx), settled(&before));
    let file = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name("env").join("komga.env"))
        .unwrap_or_default();
    assert!(read(&file).contains(&before), "{}", read(&file));

    let rotated = rotating(&ctx).await;
    let after = minted(&ctx).unwrap_or_default();
    assert_ne!(after, before);
    assert!(
        read(&file).contains(&after) && !read(&file).contains(&before),
        "{}",
        read(&file)
    );
    assert_eq!(
        rotated.map(|rotation| rotation
            .consumers
            .into_iter()
            .filter(|one| one.consumer == "komga" && one.reach == Reach::Updated)
            .count()),
        Some(2)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_replacement_whose_kept_starting_files_cannot_be_read_is_refused() {
    let (ctx, before) = installed("seeded-replaced-unkept").await;
    let kept = crate::app::plugins::kept::root(&ctx, "komga").unwrap_or_default();
    let _ = std::os::unix::fs::symlink(
        kept.join("config/komga/application.yml"),
        kept.join("config/komga/linked.yml"),
    );

    let rotated = rotating(&ctx).await;
    assert!(
        refusal(rotated.as_ref())
            .is_some_and(|said| said.contains("no starting file may be either")),
        "{rotated:?}"
    );
    assert_eq!(minted(&ctx), Some(before));
}

#[tokio::test]
async fn a_replacement_with_nowhere_to_keep_the_store_is_refused_and_keeps_the_old_key() {
    let (mut ctx, before) = installed("seeded-replaced-nowhere").await;
    ctx.settings.stack_dir = None;

    let rotated = rotating(&ctx).await;
    assert_eq!(
        refusal(rotated.as_ref()),
        Some(crate::config::store::Failure::Nowhere.to_string()),
        "{rotated:?}"
    );
    assert_eq!(minted(&ctx), Some(before.clone()));
    assert_eq!(read(&starting_of(&ctx)), settled(&before));
}

#[tokio::test]
async fn a_rehearsed_replacement_says_what_it_would_write_and_replaces_nothing() {
    let (mut ctx, before) = installed("seeded-replaced-rehearsed").await;
    ctx.dry_run = true;

    let rotated = rotating(&ctx).await;
    assert!(
        rotated.as_ref().is_some_and(|rotation| matches!(
            &rotation.settled,
            Settled::Rehearsed { detail, .. } if detail.contains("Nothing was replaced")
        )),
        "{rotated:?}"
    );
    assert_eq!(minted(&ctx), Some(before.clone()));
    assert_eq!(read(&starting_of(&ctx)), settled(&before));
}

#[tokio::test]
async fn a_replacement_with_no_stack_directory_is_refused_and_keeps_the_old_key() {
    let (mut ctx, before) = installed("seeded-replaced-stackless").await;
    ctx.stack = crate::test_support::embedded();
    ctx.settings.stack_dir = None;

    let rotated = rotating(&ctx).await;
    assert!(
        refusal(rotated.as_ref()).is_some_and(|said| said.contains("no stack directory")),
        "{rotated:?}"
    );
    assert_eq!(minted(&ctx), Some(before));
}

#[tokio::test]
async fn a_replacement_the_machine_has_no_random_bytes_for_is_refused_and_keeps_the_old_key() {
    let (mut ctx, before) = installed("seeded-replaced-unminted").await;
    ctx.seams.random = std::sync::Arc::new(lemonfiber_fixtures::ports::Chance::exactly(None));

    let rotated = rotating(&ctx).await;
    assert_eq!(
        refusal(rotated.as_ref()).as_deref(),
        Some(crate::plugin::seeding::UNMINTED),
        "{rotated:?}"
    );
    assert_eq!(minted(&ctx), Some(before.clone()));
    assert_eq!(read(&starting_of(&ctx)), settled(&before));
}

#[tokio::test]
async fn a_replacement_the_store_will_not_take_is_refused_before_any_file_is_written() {
    let (ctx, before) = installed("seeded-replaced-unstored").await;
    assert!(super::super::unrewritable(
        &crate::settings::secrets_at(&ctx).unwrap_or_default()
    ));

    let rotated = rotating(&ctx).await;
    assert!(refusal(rotated.as_ref()).is_some(), "{rotated:?}");
    assert_eq!(minted(&ctx), Some(before.clone()));
    assert_eq!(read(&starting_of(&ctx)), settled(&before));
}

#[tokio::test]
async fn an_adapter_the_new_key_cannot_be_handed_to_is_named_as_not_reached() {
    let (ctx, _) = installed("seeded-replaced-unhanded").await;
    let upstream = stack_of(&ctx).join("config/komga-adapter/upstream.json");
    let _ = std::fs::remove_file(&upstream);
    let _ = std::fs::create_dir_all(upstream.join("held"));

    let rotated = rotating(&ctx).await;
    assert!(
        matches!(
            reach(rotated.as_ref(), "komga-adapter"),
            Some(Reach::Failed { .. })
        ),
        "{rotated:?}"
    );
    assert_eq!(
        read(&starting_of(&ctx)),
        settled(&minted(&ctx).unwrap_or_default())
    );
}

#[tokio::test]
async fn services_that_do_not_restart_are_left_to_lemonfiber_restart() {
    let (mut ctx, _) = installed("seeded-replaced-unrestarted").await;
    ctx.seams.runner = std::sync::Arc::new(Recording::answering(Err(
        lemonfiber_ports::process::Failure::NotFound {
            program: "docker".to_owned(),
        },
    )));

    let rotated = rotating(&ctx).await;
    assert!(
        matches!(
            reach(rotated.as_ref(), "the services reading it"),
            Some(Reach::Pending { detail }) if detail.contains("lemonfiber restart finishes it")
        ),
        "{rotated:?}"
    );
}

#[tokio::test]
async fn a_minted_key_nothing_holds_yet_is_said_to_be_minted_at_install() {
    let (ctx, _) = installed("seeded-unheld-listed").await;
    let _ = std::fs::remove_file(crate::settings::secrets_at(&ctx).unwrap_or_default());

    let inventory = crate::app::credentials::credentials(&ctx, crate::app::command::Asking::Read)
        .await
        .ok();
    let held = inventory
        .iter()
        .flat_map(|inventory| inventory.held.iter())
        .find(|held| held.name == CREDENTIAL);
    assert!(
        held.is_some_and(|held| held
            .location
            .contains("mints it when the plugin is installed")
            && held.origin == crate::credential::Origin::Lemonfiber),
        "{held:?}"
    );
}
