use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::config::Settings;
use lemonfiber_core::plugin::Register;
use lemonfiber_core::ports::filesystem::{Beneath, Confined, Lying, Readable};
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::context::a_context;
use super::{asked, asked_about, key_at, keyed_in, moved_in, served, Keyed, ADAPTER, KEY, PLUGIN};

static NO_STACK: include_dir::Dir<'static> = include_dir::Dir::new("", &[]);

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
    let directory = Scratch::new("media-embedded-keyed");

    assert_eq!(
        key_at(&embedded(Some(&directory))),
        Some(lemonfiber_core::plugin::key_file(&directory, ADAPTER))
    );
    assert_eq!(key_at(&embedded(None)), None);
}

#[test]
fn an_external_stack_keys_the_adapter_in_its_own_directory() {
    let directory = Scratch::new("media-external-keyed");
    let ctx = a_context()
        .over(Source::External(Box::leak(
            directory.to_path_buf().into_boxed_path(),
        )))
        .build();

    assert_eq!(
        key_at(&ctx),
        Some(lemonfiber_core::plugin::key_file(&directory, ADAPTER))
    );
}

#[test]
fn a_context_naming_an_env_file_records_the_moved_plugin_once_beside_it() {
    let directory = Scratch::new("media-env-file");
    let mut ctx = embedded(Some(&directory));
    ctx.settings.env_file = Some(directory.join(".env"));

    moved_in(&ctx);
    moved_in(&ctx);

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
fn a_context_naming_no_env_file_is_keyed_and_records_no_plugin() {
    let directory = Scratch::new("media-no-env-file");
    let ctx = embedded(Some(&directory));

    moved_in(&ctx);

    let key = key_at(&ctx).unwrap_or_default();
    assert_eq!(std::fs::read_to_string(key).unwrap_or_default(), KEY);
    assert!(!directory
        .join(lemonfiber_core::config::paths::PLUGINS)
        .exists());
}

#[test]
fn a_keyed_disk_holds_the_adapters_key_and_reads_every_other_file_as_the_disk_does() {
    let directory = Scratch::new("media-keyed-disk");
    let keyed = Keyed::over(disk());
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
}

#[test]
fn a_disk_keyed_for_other_services_holds_no_key_for_the_adapter() {
    let directory = Scratch::new("media-keyed-elsewhere");
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
fn keyed_in_moves_the_server_in_and_holds_its_key_wherever_its_stack_is() {
    let directory = Scratch::new("media-keyed-in");
    let ctx = keyed_in(embedded(Some(&directory)));
    let elsewhere = Scratch::new("media-keyed-in-elsewhere");

    assert_eq!(
        std::fs::read_to_string(key_at(&ctx).unwrap_or_default()).unwrap_or_default(),
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
fn what_was_asked_is_read_per_operation_in_order() {
    let invited = super::identity("invite");
    let withdrawn = super::identity("withdraw");
    let refreshed = super::serve("refresh");
    let requests = [
        sent(invited, Some(r#"{"id":"1","name":"ana"}"#)),
        sent(withdrawn, Some(r#"{"id":"2"}"#)),
        sent(invited, None),
        sent(invited, Some("not json")),
        sent(withdrawn, Some(r#"{"name":"nobody"}"#)),
        sent(withdrawn, Some(r#"{"id":"3"}"#)),
        sent(refreshed, Some(r#"{"id":"4"}"#)),
    ];

    assert_eq!(
        asked(&requests, "invite"),
        vec![serde_json::json!({"id": "1", "name": "ana"})]
    );
    assert_eq!(asked_about(&requests, "withdraw"), vec!["2", "3"]);
    assert_eq!(
        served(&requests, "refresh"),
        vec![serde_json::json!({"id": "4"})]
    );
    assert!(asked(&requests, "refresh").is_empty());
}
