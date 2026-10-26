use lemonfiber_core::ports::http::{Method, Request};

use super::{answering, intake, told, REQUESTS};

#[test]
fn the_moved_plugin_is_seerrs_with_an_adapter_taking_requests_in_front_of_it() {
    let installed = REQUESTS.installed();

    assert_eq!(
        installed.as_ref().map(|one| one.plugin.as_str()),
        Some(REQUESTS.plugin)
    );
    assert!(
        installed.is_some_and(|one| one.services.iter().any(|placed| {
            placed.service == REQUESTS.adapter
                && placed.fronts.as_deref() == Some(REQUESTS.plugin)
                && placed
                    .speaks
                    .iter()
                    .any(|spoken| spoken == "request.intake@1")
        }))
    );
}

#[test]
fn what_the_request_service_was_told_is_read_back_from_what_was_posted() {
    let sent = [Request {
        method: Method::Post,
        url: format!("http://{}", intake("link_members")),
        headers: Vec::new(),
        body: Some(serde_json::json!({ "members": ["one"] }).to_string()),
        pinned: None,
    }];

    let told: Vec<Vec<String>> = told(&sent, "link_members", "members");

    assert_eq!(told, vec![vec!["one".to_owned()]]);
    assert_eq!(
        intake("requests"),
        "127.0.0.1:8105/lemonfiber/request.intake/v1/requests"
    );
    let (route, _) = answering("requests", &Vec::<String>::new());
    assert_eq!(route, intake("requests"));
}
