use lemonfiber_core::config::Settings;
use lemonfiber_core::plugin::Register;
use lemonfiber_core::ports::filesystem::Beneath;
use lemonfiber_core::ports::service::Download;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::context::a_context;
use super::{
    carrying, finished, keyed, moved, moved_in, operation, reaching, transfers, unkeyed, ADAPTER,
    CARRIED, FINISHED, PLUGIN,
};

#[test]
fn the_moved_plugin_is_qbittorrents_with_an_adapter_speaking_the_torrent_contract() {
    let installed = moved();

    assert_eq!(
        installed.as_ref().map(|one| one.plugin.as_str()),
        Some(PLUGIN)
    );
    assert!(
        installed.is_some_and(|one| one.services.iter().any(|placed| {
            placed.service == ADAPTER
                && placed
                    .speaks
                    .iter()
                    .any(|spoken| spoken == "download.torrent@1")
        }))
    );
}

#[test]
fn each_operation_is_asked_on_the_torrent_contracts_path() {
    assert_eq!(
        operation("transfers"),
        "/lemonfiber/download.torrent/v1/transfers"
    );
}

#[test]
fn an_unkeyed_client_is_recorded_with_no_key_written() {
    let directory = Scratch::new("torrent-unkeyed");
    let mut ctx = a_context()
        .settings(Settings {
            stack_dir: Some(directory.to_path_buf()),
            ..Settings::default()
        })
        .build();
    ctx.settings.env_file = Some(directory.join(".env"));

    unkeyed(&ctx);

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
    assert!(!lemonfiber_core::plugin::key_file(&directory, ADAPTER).exists());
}

#[test]
fn what_is_coming_down_is_answered_on_the_transfers_path_as_the_contract_carries_it() {
    let (at, answer) = transfers(&[carrying(), finished()]);
    let carried: Option<Vec<Download>> = match answer {
        Answer::Reply(200, body) => serde_json::from_str(&body).ok(),
        _ => None,
    };

    assert_eq!(at, operation("transfers"));
    assert_eq!(carried, Some(vec![carrying(), finished()]));
}

#[test]
fn a_carried_download_has_bytes_left_and_a_finished_one_none() {
    assert_eq!(
        (carrying().name, carrying().remaining),
        (CARRIED.to_owned(), Some(700))
    );
    assert_eq!(
        (finished().name, finished().progress, finished().remaining),
        (FINISHED.to_owned(), 100, Some(0))
    );
}

#[test]
fn a_client_moved_in_is_keyed_where_its_stack_is_and_one_keyed_holds_its_key_unwritten() {
    let over = |directory: &Scratch| {
        Source::External(Box::leak(directory.to_path_buf().into_boxed_path()))
    };
    let written = Scratch::new("torrent-moved-in");
    let mut ctx = a_context().over(over(&written)).build();
    ctx.settings.env_file = Some(written.join(".env"));
    let _ = reaching(ctx);

    let shared = Scratch::new("torrent-keyed");
    let mut beside = a_context().over(over(&shared)).build();
    beside.settings.env_file = Some(shared.join(".env"));
    let beside = keyed(beside);

    let at = |directory: &Scratch| lemonfiber_core::plugin::key_file(directory, ADAPTER);
    assert!(at(&written).exists());
    assert!(!at(&shared).exists());
    assert_eq!(
        beside.seams.confined.read(&at(&shared), &shared),
        Beneath::Read(super::super::adapted::KEY.to_owned())
    );
    assert!(shared
        .join(lemonfiber_core::config::paths::PLUGINS)
        .exists());
}

#[test]
fn a_client_moved_into_a_stack_it_was_already_in_is_recorded_once() {
    let directory = Scratch::new("torrent-moved-twice");
    let mut ctx = a_context()
        .over(Source::External(Box::leak(
            directory.to_path_buf().into_boxed_path(),
        )))
        .build();
    ctx.settings.env_file = Some(directory.join(".env"));

    moved_in(&ctx);
    moved_in(&ctx);

    let kept = std::fs::read_to_string(directory.join(lemonfiber_core::config::paths::PLUGINS))
        .unwrap_or_default();
    let register = Register::parse(&kept).unwrap_or_else(|_| Register::empty());
    assert_eq!(register.installed().len(), 1);
}
