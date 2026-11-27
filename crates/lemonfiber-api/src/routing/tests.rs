use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use lemonfiber_core::model::kind;

use super::{Route, Serves, Taking};

fn two_on_one_path() -> Serves<()> {
    Serves::new()
        .get("/api/thing/{name}", || async { "asked" }, &[kind::KEYS])
        .merge(Serves::new().delete(
            "/api/thing/{name}",
            || async { "gone" },
            Taking::Body,
            &[kind::KEYS, kind::MINTED_KEY],
        ))
}

async fn status_of(router: Router, method: &str) -> StatusCode {
    let request = Request::builder()
        .method(method)
        .uri("/api/thing/a")
        .body(Body::empty());
    let Ok(request) = request else {
        unreachable!("a request built from fixed parts cannot fail");
    };
    let served = tower::ServiceExt::oneshot(router, request).await.ok();
    let Some(response) = served else {
        unreachable!("the router is infallible");
    };
    response.status()
}

#[test]
fn each_route_declared_is_listed_with_its_method_body_and_kinds() {
    assert_eq!(
        two_on_one_path().routes(),
        &[
            Route {
                method: "GET",
                path: "/api/thing/{name}",
                body: false,
                kinds: vec!["keys"],
            },
            Route {
                method: "DELETE",
                path: "/api/thing/{name}",
                body: true,
                kinds: vec!["keys", "minted-key"],
            },
        ]
    );
}

#[tokio::test]
async fn each_route_listed_is_served_at_its_method() {
    let router = two_on_one_path().with_state(());
    assert_eq!(status_of(router.clone(), "GET").await, StatusCode::OK);
    assert_eq!(status_of(router.clone(), "DELETE").await, StatusCode::OK);
    assert_eq!(
        status_of(router, "POST").await,
        StatusCode::METHOD_NOT_ALLOWED
    );
}

#[test]
fn a_route_taking_nothing_is_listed_without_a_body() {
    let posted = Serves::<()>::new().post("/api/x", || async {}, Taking::Nothing, &[]);
    assert!(posted.routes().iter().all(|route| !route.body));
    assert!(posted.routes().iter().all(|route| route.kinds.is_empty()));
}

/// Every Rust file under `dir`, at any depth.
fn sources(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .flat_map(|path| {
            if path.is_dir() {
                sources(&path)
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}

/// A route is only ever declared through [`Serves`], so the surface cannot serve one the
/// contract does not list.
#[test]
fn no_route_is_declared_past_the_list() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let declaring = [".route(", ".route_service(", ".nest(", ".nest_service("];
    let routing = crate_root.join("src").join("routing");
    let mut past: Vec<String> = sources(&crate_root.join("src"))
        .into_iter()
        .filter(|path| path.with_extension("") != routing && !path.starts_with(&routing))
        .filter(|path| {
            std::fs::read_to_string(path)
                .is_ok_and(|text| declaring.iter().any(|call| text.contains(call)))
        })
        .map(|path| path.display().to_string())
        .collect();
    past.sort();
    assert!(past.is_empty(), "routed outside `Serves`: {past:?}");
    assert!(sources(&crate_root.join("src")).len() > 1);
}
