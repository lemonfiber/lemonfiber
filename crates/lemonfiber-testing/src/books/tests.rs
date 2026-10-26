use lemonfiber_core::ports::filesystem::Beneath;
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::ports::service::{
    AggregatorAt, Category, Credential, DownloadClient, Protocol, RegisteredClient,
    RegisteredFolder, RootFolder,
};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::context::a_context;
use super::{
    add_aggregator, added, aggregators, asking, clients_registered, curating, download_clients,
    folders_registered, keyed, moved, pulling, reaching, reading_from, root_folders, taken,
    ADAPTER, PLUGIN,
};

#[test]
fn the_moved_plugin_is_binderys_with_an_adapter_pulling_and_curating_over_their_contracts() {
    let installed = moved();

    assert_eq!(
        installed.as_ref().map(|one| one.plugin.as_str()),
        Some(PLUGIN)
    );
    assert!(
        installed.is_some_and(|one| one.services.iter().any(|placed| {
            placed.service == ADAPTER
                && placed.fronts.as_deref() == Some(PLUGIN)
                && ["indexer.pull@1", "library.curate@1"]
                    .iter()
                    .all(|contract| placed.speaks.iter().any(|spoken| spoken == contract))
        }))
    );
}

#[test]
fn each_operation_is_asked_on_its_contracts_path() {
    assert_eq!(
        pulling("aggregators"),
        "/lemonfiber/indexer.pull/v1/aggregators"
    );
    assert_eq!(
        curating("root_folders"),
        "/lemonfiber/library.curate/v1/root_folders"
    );
}

#[test]
fn what_the_curator_reads_from_is_answered_on_the_aggregators_path() {
    let (at, answer) = aggregators(&[reading_from("http://prowlarr:9696", false)]);

    assert_eq!(at, pulling("aggregators"));
    assert!(
        matches!(&answer, Answer::Reply(200, body)
            if body == r#"[{"id":"1","url":"http://prowlarr:9696","keyed":false}]"#),
        "{answer:?}"
    );
    let (at, answer) = add_aggregator(taken());
    assert_eq!(at, pulling("add_aggregator"));
    assert!(matches!(answer, Answer::Reply(204, _)));
}

#[test]
fn what_the_curator_holds_is_answered_on_the_curate_paths() {
    let (folders, held) = root_folders(&[RegisteredFolder {
        id: "1".to_owned(),
        path: "/data/media/books".to_owned(),
    }]);
    let (clients, none) = download_clients(&[]);

    assert_eq!(folders, curating("root_folders"));
    assert!(
        matches!(&held, Answer::Reply(200, body) if body.contains("/data/media/books")),
        "{held:?}"
    );
    assert_eq!(clients, curating("download_clients"));
    assert!(
        matches!(&none, Answer::Reply(200, body) if body == "[]"),
        "{none:?}"
    );
}

/// A request posted to `path` carrying `body`.
fn posted(path: &str, body: &serde_json::Value) -> Request {
    Request {
        method: Method::Post,
        url: format!("http://127.0.0.1:8080{path}"),
        headers: Vec::new(),
        body: Some(body.to_string()),
        pinned: None,
    }
}

#[test]
fn what_a_pass_told_the_curator_is_read_back_from_each_operation_it_asked() {
    let aggregator = AggregatorAt {
        name: "Prowlarr".to_owned(),
        url: "http://prowlarr:9696".to_owned(),
        protocol: Protocol("servarr".to_owned()),
        key: "the-key".to_owned(),
    };
    let folder = RootFolder {
        path: "/data/media/books".to_owned(),
        media_type: "books".to_owned(),
    };
    let client = DownloadClient {
        name: "SABnzbd".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        protocol: Protocol("sabnzbd".to_owned()),
        credential: Credential::ApiKey("the-key".to_owned()),
        category: Category {
            field: "category".to_owned(),
            value: "books".to_owned(),
        },
    };
    let sent = vec![
        posted(pulling("aggregators"), &serde_json::json!({})),
        posted(
            pulling("add_aggregator"),
            &serde_json::json!({ "aggregator": aggregator }),
        ),
        posted(
            curating("register_root_folder"),
            &serde_json::json!({ "folder": folder }),
        ),
        posted(
            curating("register_download_client"),
            &serde_json::json!({ "client": client }),
        ),
        posted(pulling("add_aggregator"), &serde_json::json!({ "else": 1 })),
    ];

    assert_eq!(added(&sent), vec![aggregator]);
    assert_eq!(folders_registered(&sent), vec![folder]);
    assert_eq!(clients_registered(&sent), vec![client]);
}

#[test]
fn a_curator_moved_in_is_keyed_where_its_stack_is_and_one_asked_holds_its_key_unwritten() {
    let over = |directory: &Scratch| {
        Source::External(Box::leak(directory.to_path_buf().into_boxed_path()))
    };
    let written = Scratch::new("books-moved-in");
    let mut ctx = a_context().over(over(&written)).build();
    ctx.settings.env_file = Some(written.join(".env"));
    let _ = reaching(ctx);

    let shared = Scratch::new("books-keyed");
    let mut beside = a_context().over(over(&shared)).build();
    beside.settings.env_file = Some(shared.join(".env"));
    let beside = keyed(beside);

    let elsewhere = Scratch::new("books-asking");
    let asked = asking(a_context().over(over(&elsewhere)).build());

    let at = |directory: &Scratch| lemonfiber_core::plugin::key_file(directory, ADAPTER);
    assert!(at(&written).exists());
    assert!(!at(&shared).exists());
    assert!(!at(&elsewhere).exists());
    for (ctx, directory) in [(&beside, &shared), (&asked, &elsewhere)] {
        assert_eq!(
            ctx.seams.confined.read(&at(directory), directory),
            Beneath::Read(super::super::adapted::KEY.to_owned())
        );
    }
    assert!(shared
        .join(lemonfiber_core::config::paths::PLUGINS)
        .exists());
    assert!(!elsewhere
        .join(lemonfiber_core::config::paths::PLUGINS)
        .exists());
}

#[test]
fn a_held_client_is_answered_as_the_contract_carries_it() {
    let (_, answer) = download_clients(&[RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "category".to_owned(),
            value: "books".to_owned(),
        }),
    }]);

    assert!(
        matches!(&answer, Answer::Reply(200, body) if body.contains("\"category\"")),
        "{answer:?}"
    );
}
