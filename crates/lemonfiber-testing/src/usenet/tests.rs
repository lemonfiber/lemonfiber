use lemonfiber_core::config::Settings;
use lemonfiber_core::plugin::Register;
use lemonfiber_core::ports::service::Download;
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::context::a_context;
use super::{carrying, moved, operation, transfers, unkeyed, ADAPTER, CARRIED, PLUGIN};

#[test]
fn the_moved_plugin_is_sabnzbds_with_an_adapter_speaking_the_usenet_contract() {
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
                    .any(|spoken| spoken == "download.usenet@1")
        }))
    );
}

#[test]
fn each_operation_is_asked_on_the_usenet_contracts_path() {
    assert_eq!(
        operation("transfers"),
        "/lemonfiber/download.usenet/v1/transfers"
    );
}

#[test]
fn an_unkeyed_client_is_recorded_with_no_key_written() {
    let directory = Scratch::new("usenet-unkeyed");
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
    let (at, answer) = transfers(&[carrying()]);
    let carried: Option<Vec<Download>> = match answer {
        Answer::Reply(200, body) => serde_json::from_str(&body).ok(),
        _ => None,
    };

    assert_eq!(at, operation("transfers"));
    assert_eq!(
        carried.map(|downloads| {
            downloads
                .into_iter()
                .map(|one| one.name)
                .collect::<Vec<_>>()
        }),
        Some(vec![CARRIED.to_owned()])
    );
}
