use super::declined;
use crate::Recipe;

/// A recipe signing in to its own service, fetching a cover from outside with what it
/// was given, posting the cover back, and reading its own library apart from both.
const COVERS: &str = r#"
id    = "covers"
title = "Give new comics their covers"
why   = "A comic without a cover is hard to find on a shelf."

[[input]]
name   = "title"
origin = "operator"
ask    = "Which comic"

[[step]]
id      = "sign-in"
call    = { method = "POST", to = "komga", path = "/api/v1/login" }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

[[step]]
id      = "art"
call    = { method = "GET", to = "covers.example.com", path = "/find?title={{title}}" }
capture = [{ name = "cover", from = "url", origin = "external-response" }]

[[step]]
id      = "hang"
call    = { method = "POST", to = "komga", path = "/api/v1/covers", headers = { Authorization = "Bearer {{token}}" }, body = "{{cover}}" }

[[step]]
id      = "found"
when    = { step = "art", status = 200 }
call    = { method = "GET", to = "komga", path = "/api/v1/found" }

[[step]]
id      = "libraries"
call    = { method = "GET", to = "komga", path = "/api/v1/libraries", headers = { Authorization = "Bearer {{token}}" } }
"#;

fn stopped(refused: &str) -> Vec<String> {
    toml::from_str::<Recipe>(COVERS).map_or_else(
        |why| vec![why.to_string()],
        |recipe| {
            declined(&recipe, &|to| to == refused)
                .into_iter()
                .map(|step| step.id.clone())
                .collect()
        },
    )
}

#[test]
fn refusing_a_host_stops_the_steps_calling_it_and_every_step_depending_on_them() {
    assert_eq!(stopped("covers.example.com"), ["art", "hang", "found"]);
}

#[test]
fn refusing_a_host_no_step_calls_stops_nothing() {
    assert!(stopped("posters.example.com").is_empty());
}

#[test]
fn refusing_its_own_service_leaves_a_step_reading_nothing_it_captured() {
    assert_eq!(stopped("komga"), ["sign-in", "hang", "found", "libraries"]);
}
