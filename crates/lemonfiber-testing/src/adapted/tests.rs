use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::config::paths::STARTING_KEPT;
use lemonfiber_core::config::Settings;
use lemonfiber_core::plugin::Register;
use lemonfiber_core::ports::filesystem::{Beneath, Confined, Lying, Readable};
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::context::a_context;
use super::super::curators::TELEVISION;
use super::super::usenet::{ADAPTER, PLUGIN};
use super::{asked_at, installed_parts, Adapted, Keyed, KEY, PORT};

const MOVED: Adapted = Adapted {
    plugin: PLUGIN,
    adapter: ADAPTER,
    port: PORT,
};

static NO_STACK: lemonfiber_core::stack::Carried = lemonfiber_core::stack::Carried {
    manifest: "",
    files: &NOTHING,
};

static NOTHING: include_dir::Dir<'static> = include_dir::Dir::new("", &[]);

fn embedded(stack_dir: Option<&Path>) -> lemonfiber_core::app::Ctx {
    a_context()
        .over(Source::Embedded(&NO_STACK))
        .settings(Settings {
            stack_dir: stack_dir.map(Path::to_path_buf),
            ..Settings::default()
        })
        .build()
}

fn disk() -> Arc<dyn Confined> {
    Arc::new(lemonfiber_adapters::Disk)
}

fn sent(url: &str, body: Option<&str>) -> Request {
    Request {
        method: Method::Post,
        url: format!("http://127.0.0.1:8080{url}"),
        headers: Vec::new(),
        body: body.map(str::to_owned),
        pinned: None,
    }
}

#[test]
fn an_embedded_stack_keys_the_adapter_in_the_stack_directory_its_settings_name() {
    let directory = Scratch::new("adapted-embedded-keyed");

    assert_eq!(
        MOVED.key_at(&embedded(Some(&directory))),
        Some(lemonfiber_core::plugin::key_file(&directory, ADAPTER))
    );
    assert_eq!(MOVED.key_at(&embedded(None)), None);
}

#[test]
fn an_external_stack_keys_the_adapter_in_its_own_directory() {
    let directory = Scratch::new("adapted-external-keyed");
    let ctx = a_context()
        .over(Source::External(Box::leak(
            directory.to_path_buf().into_boxed_path(),
        )))
        .build();

    assert_eq!(
        MOVED.key_at(&ctx),
        Some(lemonfiber_core::plugin::key_file(&directory, ADAPTER))
    );
}

#[test]
fn a_context_naming_an_env_file_records_the_moved_plugin_once_beside_it() {
    let directory = Scratch::new("adapted-env-file");
    let mut ctx = embedded(Some(&directory));
    ctx.settings.env_file = Some(directory.join(".env"));

    MOVED.moved_in(&ctx);
    MOVED.moved_in(&ctx);

    let kept = std::fs::read_to_string(directory.join(lemonfiber_core::config::paths::PLUGINS))
        .unwrap_or_default();
    let register = Register::parse(&kept).unwrap_or_else(|_| Register::empty());
    assert_eq!(
        register
            .installed()
            .iter()
            .map(|one| one.plugin.as_str())
            .collect::<Vec<_>>(),
        vec![PLUGIN]
    );
}

#[test]
fn minting_for_a_second_curator_keeps_what_the_store_held_before() {
    use super::super::curators::{MUSIC, TELEVISION};
    let directory = Scratch::new("adapted-minted");
    let mut ctx = embedded(Some(&directory));
    ctx.settings.env_file = Some(directory.join(".env"));

    TELEVISION.minted(&ctx, "television-key");
    MUSIC.minted(&ctx, "music-key");

    let store: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.join(lemonfiber_core::config::paths::SECRETS))
            .unwrap_or_default(),
    )
    .unwrap_or_default();
    let held: Vec<(String, String)> = store
        .get("held")
        .and_then(serde_json::Value::as_object)
        .map(|held| {
            held.iter()
                .map(|(name, one)| {
                    (
                        name.split(lemonfiber_manifest::container::NAMESPACED)
                            .next()
                            .unwrap_or_default()
                            .to_owned(),
                        one.get("value")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    for (plugin, value) in [
        (TELEVISION.plugin, "television-key"),
        (MUSIC.plugin, "music-key"),
    ] {
        assert!(
            held.contains(&(plugin.to_owned(), value.to_owned())),
            "{plugin}: {held:?}"
        );
    }
}

#[test]
fn a_context_naming_no_env_file_is_keyed_and_records_no_plugin() {
    let directory = Scratch::new("adapted-no-env-file");
    let ctx = embedded(Some(&directory));

    MOVED.moved_in(&ctx);

    let key = MOVED.key_at(&ctx).unwrap_or_default();
    assert_eq!(std::fs::read_to_string(key).unwrap_or_default(), KEY);
    assert!(!directory
        .join(lemonfiber_core::config::paths::PLUGINS)
        .exists());
}

#[test]
fn a_keyed_disk_holds_the_adapters_key_and_reads_every_other_file_as_the_disk_does() {
    let directory = Scratch::new("adapted-keyed-disk");
    let keyed = Keyed::holding(disk(), &[ADAPTER]);
    let key = lemonfiber_core::plugin::key_file(&directory, ADAPTER);
    let other = directory.join("other.txt");

    assert_eq!(keyed.read(&key, &directory), Beneath::Read(KEY.to_owned()));
    assert_eq!(keyed.read(&other, &directory), Beneath::Absent);
    assert_eq!(keyed.lies(&other, &directory), Lying::Absent);

    assert_eq!(keyed.overwrite(&other, &directory, b"written"), Ok(()));
    assert_eq!(
        keyed.read(&other, &directory),
        Beneath::Read("written".to_owned())
    );
    assert_eq!(keyed.lies(&other, &directory), Lying::Plain);

    let moved = directory.join("moved").join("other.txt");
    assert_eq!(keyed.moved(&other, &moved, &directory), Ok(()));
    assert_eq!(keyed.lies(&other, &directory), Lying::Absent);
    assert_eq!(
        keyed.read(&moved, &directory),
        Beneath::Read("written".to_owned())
    );

    let seeded = directory.join("seeded").join("other.txt");
    assert_eq!(
        keyed.written(&seeded, &directory, b"seeded", Readable::Owner),
        Ok(())
    );
    assert_eq!(
        keyed.read(&seeded, &directory),
        Beneath::Read("seeded".to_owned())
    );

    let replaced = directory.join("replaced").join("other.bin");
    assert_eq!(
        keyed.made(&directory.join("replaced"), &directory, Readable::Owner),
        Ok(())
    );
    assert_eq!(
        keyed.replaced(&replaced, &directory, b"whole", Readable::Owner),
        Ok(())
    );
    assert_eq!(
        keyed.bytes(&replaced, &directory),
        Ok(Some(b"whole".to_vec()))
    );
    assert_eq!(keyed.removed(&replaced, &directory), Ok(()));
    assert_eq!(keyed.bytes(&replaced, &directory), Ok(None));
}

#[test]
fn a_disk_keyed_for_other_services_holds_no_key_for_the_adapter() {
    let directory = Scratch::new("adapted-keyed-elsewhere");
    let keyed = Keyed::holding(disk(), &["another-adapter"]);

    assert_eq!(
        keyed.read(
            &lemonfiber_core::plugin::key_file(&directory, "another-adapter"),
            &directory
        ),
        Beneath::Read(KEY.to_owned())
    );
    assert_eq!(
        keyed.read(
            &lemonfiber_core::plugin::key_file(&directory, ADAPTER),
            &directory
        ),
        Beneath::Absent
    );
}

#[test]
fn keyed_in_moves_the_service_in_and_holds_its_key_wherever_its_stack_is() {
    let directory = Scratch::new("adapted-keyed-in");
    let ctx = MOVED.keyed_in(embedded(Some(&directory)));
    let elsewhere = Scratch::new("adapted-keyed-in-elsewhere");

    assert_eq!(
        std::fs::read_to_string(MOVED.key_at(&ctx).unwrap_or_default()).unwrap_or_default(),
        KEY
    );
    assert_eq!(
        ctx.seams.confined.read(
            &lemonfiber_core::plugin::key_file(&elsewhere, ADAPTER),
            &elsewhere
        ),
        Beneath::Read(KEY.to_owned())
    );
}

#[test]
fn what_was_asked_at_one_path_is_read_in_order() {
    let requests = [
        sent("/one", Some(r#"{"id":"1"}"#)),
        sent("/two", Some(r#"{"id":"2"}"#)),
        sent("/one", None),
        sent("/one", Some("not json")),
        sent("/one", Some(r#"{"id":"3"}"#)),
    ];

    assert_eq!(
        asked_at(&requests, "/one"),
        vec![
            serde_json::json!({"id": "1"}),
            serde_json::json!({"id": "3"})
        ]
    );
    assert!(asked_at(&requests, "/three").is_empty());
}

#[test]
fn a_register_is_recorded_in_a_directory_made_for_it_where_none_is_yet() {
    let directory = Scratch::new("adapted-unmade");
    let unmade = directory.join("not-yet").join(".env");
    let mut ctx = embedded(None);
    ctx.settings.env_file = Some(unmade.clone());

    MOVED.recorded(&ctx);

    let kept =
        std::fs::read_to_string(unmade.with_file_name(lemonfiber_core::config::paths::PLUGINS))
            .unwrap_or_default();
    let register = Register::parse(&kept).unwrap_or_else(|_| Register::empty());
    assert_eq!(register.installed().len(), 1);
}

#[test]
fn keyed_records_the_plugin_and_holds_its_key_without_writing_one() {
    let directory = Scratch::new("adapted-keyed");
    let mut ctx = embedded(Some(&directory));
    ctx.settings.env_file = Some(directory.join(".env"));

    let ctx = MOVED.keyed(ctx);

    let key = MOVED.key_at(&ctx).unwrap_or_default();
    assert!(!key.exists());
    assert_eq!(
        ctx.seams.confined.read(&key, &directory),
        Beneath::Read(KEY.to_owned())
    );
    assert!(directory
        .join(lemonfiber_core::config::paths::PLUGINS)
        .exists());
}

#[test]
fn a_claim_is_asked_on_the_owner_contracts_path_and_answered_with_who_holds_the_part() {
    let (at, answer) = super::held_by("claim", lemonfiber_core::ports::service::Owner::Ours);

    assert_eq!(super::claiming("owner"), "/lemonfiber/owner.claim/v1/owner");
    assert_eq!(at, "/lemonfiber/owner.claim/v1/claim");
    assert!(
        matches!(&answer, lemonfiber_fixtures::http::Answer::Reply(200, body) if body == "\"ours\""),
        "{answer:?}"
    );
}

/// The digests of every plugin, by its id.
type Digests = Vec<(String, String, String)>;

/// Every part the bundle pins as installed is recorded first-party, by the digests it is
/// pinned at, and nothing else is.
#[test]
fn every_part_the_bundle_installs_is_recorded_as_it_is_pinned() {
    let recorded: Digests = installed_parts()
        .into_iter()
        .map(|one| (one.plugin, one.manifest, one.tree))
        .collect();
    let pinned: Digests = lemonfiber_bundled::PINNED
        .iter()
        .filter(|one| one.stage == Some(lemonfiber_bundled::Stage::Installed))
        .map(|one| {
            (
                one.first_party.plugin.to_owned(),
                one.first_party.manifest.to_owned(),
                one.first_party.tree.to_owned(),
            )
        })
        .collect();

    assert!(!recorded.is_empty());
    assert_eq!(recorded, pinned);
}

/// A plugin's pinned copy is kept beside the env file a context names, and nowhere where
/// it names none.
#[test]
fn a_kept_manifest_is_the_pinned_copy_beside_the_env_file() {
    let env = Scratch::new("adapted-kept").join(".env");
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            ..Settings::default()
        })
        .build();
    let unnamed = a_context().build();

    TELEVISION.kept(&ctx);
    TELEVISION.kept(&unnamed);

    let kept = std::fs::read_to_string(
        env.with_file_name(STARTING_KEPT)
            .join(TELEVISION.plugin)
            .join(lemonfiber_bundle::MANIFEST),
    )
    .ok();
    let pinned = lemonfiber_bundled::PINNED
        .iter()
        .find(|one| one.first_party.plugin == TELEVISION.plugin)
        .map(|one| one.copy.to_owned());
    assert!(pinned.is_some());
    assert_eq!(kept, pinned);
    assert_eq!(unnamed.settings.env_file, None);
}
