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
    ctx.stack = crate::test_support::carried_stack();
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
            reach(rotated.as_ref(), "its services"),
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

/// The stack's own service the moving manifest's upstream moves in place of.
const RESERVED: &str = "jellyfin";

/// The key the stack published for [`RESERVED`].
const RECORDED: &str = "recorded-by-the-stack";

/// The seeded manifest, its upstream moving in place of [`RESERVED`].
fn moving() -> Option<lemonfiber_plugin::Manifest> {
    lemonfiber_plugin::Manifest::from_toml(&seeded().replacen(
        "id          = \"komga\"\nname        = \"Komga\"\nimage",
        &format!(
            "id          = \"komga\"\nname        = \"Komga\"\nreplaces    = \"{RESERVED}\"\nimage"
        ),
        1,
    ))
    .ok()
}

/// What minting the moving manifest leaves held, `trusted` as first-party, in a stack that
/// published [`RECORDED`] for [`RESERVED`].
fn minted_moving(name: &str, trusted: &[crate::plugin::first_party::FirstParty]) -> Option<String> {
    let ctx = seeding(name);
    let manifest = moving()?;
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let recorded = format!(
        "{}{}={RECORDED}\n",
        std::fs::read_to_string(&env).unwrap_or_default(),
        crate::seed::run::published_as(RESERVED)
    );
    let _ = std::fs::write(&env, recorded);
    let mut would = crate::plugin::Installed::of(&manifest);
    would.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    would.tree = crate::test_support::A_TREE.to_owned();

    super::super::super::minting::minted(&ctx, &would, &manifest, "1", trusted).ok()?;
    minted(&ctx)
}

#[test]
fn a_first_party_part_moved_in_place_keeps_the_key_the_stack_published_for_it() {
    assert!(moving().is_some());
    assert_eq!(
        minted_moving(
            "seeded-moved-trusted",
            &crate::test_support::first_party("komga")
        )
        .as_deref(),
        Some(RECORDED)
    );
    let stranger = minted_moving("seeded-moved-stranger", &[]).unwrap_or_default();
    assert_eq!(stranger.len(), 32, "{stranger:?}");
    assert_ne!(stranger, RECORDED);
}

/// What minting the bundled subtitle finder's plugin leaves held for its upstream's key,
/// first-party as this build embeds it, in a stack that published `recorded` for its own
/// subtitle finder, or nothing.
fn minted_subtitle_finder(name: &str, recorded: Option<&str>) -> Option<String> {
    use crate::test_support::subtitles;
    let ctx = seeding(name);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    if let Some(key) = recorded {
        let _ = crate::config::store::set(
            &env,
            &crate::seed::run::published_as(subtitles::PLUGIN),
            key,
        );
    }
    let manifest = lemonfiber_bundled::PINNED
        .iter()
        .find(|one| one.first_party.plugin == subtitles::PLUGIN)
        .and_then(|one| lemonfiber_plugin::Manifest::from_toml(one.copy).ok())?;
    let would = subtitles::moved()?;

    super::super::super::minting::minted(
        &ctx,
        &would,
        &manifest,
        "1",
        crate::plugin::first_party::EMBEDDED,
    )
    .ok()?;
    crate::settings::secrets_at(&ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .and_then(|held| {
            held.value(&crate::settings::named(subtitles::PLUGIN, "upstream-key"))
                .map(str::to_owned)
        })
}

/// The subtitle finder moved into its plugin is handed the key the stack published for its
/// own, which the Bazarr it moves in place of already answers to; a stack that published
/// none has one minted.
#[test]
fn the_moved_subtitle_finder_keeps_the_key_the_stack_published_for_its_own() {
    assert_eq!(
        minted_subtitle_finder("seeded-moved-subtitles", Some(RECORDED)).as_deref(),
        Some(RECORDED)
    );
    let fresh = minted_subtitle_finder("seeded-moved-subtitles-fresh", None).unwrap_or_default();
    assert_eq!(fresh.len(), 32, "{:?}", fresh.len());
}

/// The seeded manifest, its adapter handing its upstream an account lemonfiber mints and
/// its upstream answering in `native` in place of the stack's service of that name.
fn moving_an_account(native: &str) -> Option<lemonfiber_plugin::Manifest> {
    let account = "upstream    = { username = { secret = \"upstream-username\" }, password = \
                   { secret = \"upstream-password\" } }";
    let text = seeded()
        .replace(
            "upstream    = { api_key = { secret = \"upstream-key\" } }",
            account,
        )
        .replace(
            "id     = \"upstream-key\"\nof     = \"komga\"\nminted = true\nwhy    = \"The key the \
             adapter signs in to Komga with\"\n",
            "id     = \"upstream-username\"\nof     = \"komga\"\nminted = true\nwhy    = \"The \
             account the adapter signs in to Komga as\"\n\n[[secret]]\nid     = \
             \"upstream-password\"\nof     = \"komga\"\nminted = true\nwhy    = \"That \
             account's password\"\n",
        )
        .replacen(
            "id          = \"komga\"\nname        = \"Komga\"\nimage",
            &format!(
                "id          = \"komga\"\nname        = \"Komga\"\nreplaces    = \
                 \"{native}\"\nnative      = \"{native}\"\nimage"
            ),
            1,
        );
    lemonfiber_plugin::Manifest::from_toml(&text).ok()
}

/// The account minting the moving-an-account manifest leaves held for its upstream
/// answering in `native`, `trusted` as first-party, in a stack that recorded `recorded` as
/// its torrent client's password, or none.
fn minted_account(
    name: &str,
    native: &str,
    recorded: Option<&str>,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> Option<(String, String)> {
    let ctx = seeding(name);
    let manifest = moving_an_account(native)?;
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let _ = match recorded {
        Some(password) => {
            crate::config::store::set(&env, crate::config::TORRENT_PASSWORD_KEY, password)
        }
        None => crate::config::store::unset(&env, crate::config::TORRENT_PASSWORD_KEY),
    };
    let mut would = crate::plugin::Installed::of(&manifest);
    would.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    would.tree = crate::test_support::A_TREE.to_owned();

    super::super::super::minting::minted(&ctx, &would, &manifest, "1", trusted).ok()?;
    let held =
        crate::settings::secrets_at(&ctx).and_then(|at| crate::settings::Secrets::at(&at).ok())?;
    let part = |id: &str| held.value(&format!("komga:{id}")).map(str::to_owned);
    Some((part("upstream-username")?, part("upstream-password")?))
}

/// A first-party upstream moved in place of the stack's torrent client keeps the account
/// lemonfiber set up on it: the name the stack signs in under, and the password recorded
/// for it.
#[test]
fn a_first_party_part_moved_in_place_keeps_the_account_the_stack_set_up_on_it() {
    assert!(moving_an_account("qbittorrent").is_some());
    let trusted = crate::test_support::first_party("komga");

    let kept = minted_account(
        "seeded-account-trusted",
        "qbittorrent",
        Some(RECORDED),
        &trusted,
    );
    assert_eq!(kept, Some(("admin".to_owned(), RECORDED.to_owned())));

    let stranger = minted_account(
        "seeded-account-stranger",
        "qbittorrent",
        Some(RECORDED),
        &[],
    )
    .unwrap_or_default();
    assert_eq!((stranger.0.len(), stranger.1.len()), (32, 32));
    assert_ne!(stranger.1, RECORDED);
}

/// An upstream answering in an API the stack sets up no account on has a fresh account
/// minted, whatever the stack recorded.
#[test]
fn a_part_moved_in_place_of_a_service_with_no_account_is_minted_one() {
    let minted = minted_account(
        "seeded-account-none",
        "servarr",
        Some(RECORDED),
        &crate::test_support::first_party("komga"),
    )
    .unwrap_or_default();

    assert_eq!((minted.0.len(), minted.1.len()), (32, 32));
    assert_ne!(minted.1, RECORDED);
}

/// A password the stack never recorded for the service a part moves in place of is minted
/// afresh, while the name the stack signs in under is still kept.
#[test]
fn a_part_moved_in_place_of_a_service_with_no_password_recorded_is_minted_one() {
    let minted = minted_account(
        "seeded-account-unrecorded",
        "qbittorrent",
        None,
        &crate::test_support::first_party("komga"),
    )
    .unwrap_or_default();

    assert_eq!(minted.0, "admin");
    assert_eq!(minted.1.len(), 32, "{:?}", minted.1.len());
}
