use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::ports::service::{Application, ApplicationKind};

use super::{answering, registered, searching, INDEXER};

#[test]
fn the_moved_plugin_is_prowlarrs_with_an_adapter_searching_in_front_of_it() {
    let installed = INDEXER.installed();

    assert_eq!(
        installed.as_ref().map(|one| one.plugin.as_str()),
        Some(INDEXER.plugin)
    );
    assert!(
        installed.is_some_and(|one| one.services.iter().any(|placed| {
            placed.service == INDEXER.adapter
                && placed.fronts.as_deref() == Some(INDEXER.plugin)
                && placed
                    .speaks
                    .iter()
                    .any(|spoken| spoken == "indexer.search@1")
        }))
    );
}

#[test]
fn each_application_registered_is_read_back_from_what_was_posted() {
    let application = Application {
        name: "Sonarr".to_owned(),
        kind: ApplicationKind::Tv,
        indexer_url: "http://prowlarr:9696".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
        api_key: "a-key".to_owned(),
    };
    let sent = [Request {
        method: Method::Post,
        url: format!("http://{}", searching("register_application")),
        headers: Vec::new(),
        body: Some(serde_json::json!({ "application": application }).to_string()),
        pinned: None,
    }];

    assert_eq!(registered(&sent), vec![application]);
    assert_eq!(
        searching("applications"),
        "127.0.0.1:8104/lemonfiber/indexer.search/v1/applications"
    );
    let (route, _) = answering("applications", &Vec::<String>::new());
    assert_eq!(route, searching("applications"));
}
