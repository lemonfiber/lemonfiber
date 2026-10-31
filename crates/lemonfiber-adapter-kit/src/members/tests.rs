use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use lemonfiber_contract::adapter::About;
use lemonfiber_ports::service::{Answered, Failure, Presentation};
use tower::ServiceExt as _;

use super::{Asking, ITEM_MOST, MAY, TOKEN_MOST};
use crate::Kit;

const KEY: &str = "the-plugins-key";

/// The member's token the upstream knows.
const MEMBER: &str = "member-token";

/// What the upstream was asked, each as the item and the token it was asked with.
type Heard = Arc<Mutex<Vec<(String, String)>>>;

/// An upstream letting [`MEMBER`] have item `7`, refusing any other token, and answering
/// nothing for item `down`.
fn upstream(heard: &Heard) -> Asking {
    let heard = Arc::clone(heard);
    Arc::new(move |item: String, token: String| {
        if let Ok(mut heard) = heard.lock() {
            heard.push((item.clone(), token.clone()));
        }
        Box::pin(async move {
            match (item.as_str(), token == MEMBER) {
                ("down", _) => Err(Failure::Unavailable {
                    service: "plex".to_owned(),
                }),
                (_, false) => Ok(Answered::NoSession),
                ("7", true) => Ok(Answered::May),
                _ => Ok(Answered::MayNot),
            }
        })
    })
}

/// A kit asking that upstream, the token presented as `presented`.
fn kit(presented: Presentation, heard: &Heard) -> Kit {
    Kit::new(KEY, about_nothing()).asked_by_members(presented, upstream(heard))
}

/// An adapter saying it speaks nothing in front of nothing.
fn about_nothing() -> About {
    About {
        speaks: Vec::new(),
        upstream: String::new(),
        releases: Vec::new(),
    }
}

/// The status the kit answers `GET` at `uri` with, carrying `headers`.
async fn asked(kit: Kit, uri: &str, headers: &[(&str, &str)]) -> StatusCode {
    let mut request = Request::builder().method("GET").uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = kit
        .router()
        .oneshot(request.body(Body::empty()).unwrap_or_default())
        .await;
    let Ok(response) = response;
    response.status()
}

/// Headers a question carries, each by name.
type Headers<'h> = Vec<(&'h str, &'h str)>;

/// A presentation of the token, the question asked, and the headers it carries.
type Presented<'h> = (Presentation, String, Headers<'h>);

/// The question about `item`.
fn about(item: &str) -> String {
    MAY.replace("{item}", item)
}

/// A header presentation of the token as Plex reads it.
fn plex() -> Presentation {
    Presentation::Header {
        name: "x-plex-token".to_owned(),
        value: "{token}".to_owned(),
    }
}

#[tokio::test]
async fn the_upstream_decides_with_the_members_own_token_and_no_key() {
    let heard = Heard::default();
    for (item, token, status) in [
        ("7", MEMBER, StatusCode::NO_CONTENT),
        ("8", MEMBER, StatusCode::NOT_FOUND),
        ("7", "someone-else", StatusCode::UNAUTHORIZED),
        ("down", MEMBER, StatusCode::BAD_GATEWAY),
    ] {
        let said = asked(
            kit(plex(), &heard),
            &about(item),
            &[("x-plex-token", token)],
        )
        .await;
        assert_eq!(said, status, "{item} {token}");
    }
    let heard = heard.lock().map(|heard| heard.clone()).unwrap_or_default();
    assert_eq!(heard.first(), Some(&("7".to_owned(), MEMBER.to_owned())));
    assert_eq!(heard.len(), 4);
}

#[tokio::test]
async fn a_question_without_a_token_or_carrying_the_plugins_key_asks_nothing() {
    let heard = Heard::default();
    let keyed = format!("Bearer {KEY}");
    let cases: [(Presentation, Headers<'_>); 5] = [
        (plex(), Vec::new()),
        (plex(), vec![("x-plex-token", KEY)]),
        (
            plex(),
            vec![("x-plex-token", MEMBER), ("x-plex-token", MEMBER)],
        ),
        (Presentation::AsIs, vec![("authorization", &keyed)]),
        (Presentation::AsIs, vec![("authorization", MEMBER)]),
    ];
    for (presented, headers) in cases {
        let said = asked(kit(presented, &heard), &about("7"), &headers).await;
        assert_eq!(said, StatusCode::UNAUTHORIZED, "{headers:?}");
    }
    let long = "t".repeat(TOKEN_MOST + 1);
    let said = asked(kit(plex(), &heard), &about("7"), &[("x-plex-token", &long)]).await;
    assert_eq!(said, StatusCode::UNAUTHORIZED);
    assert!(heard.lock().is_ok_and(|heard| heard.is_empty()));
}

#[tokio::test]
async fn the_token_is_read_where_the_declaration_presents_it() {
    let heard = Heard::default();
    let bearer = format!("Bearer {MEMBER}");
    let wrapped = format!("MediaBrowser Token=\"{MEMBER}\"");
    let cases: [Presented<'_>; 4] = [
        (
            Presentation::AsIs,
            about("7"),
            vec![("authorization", &bearer)],
        ),
        (
            Presentation::Header {
                name: "authorization".to_owned(),
                value: "MediaBrowser Token=\"{token}\"".to_owned(),
            },
            about("7"),
            vec![("authorization", &wrapped)],
        ),
        (
            Presentation::Query {
                parameter: "X-Plex-Token".to_owned(),
            },
            format!("{}&X-Plex-Token={MEMBER}", about("7")),
            Vec::new(),
        ),
        (
            Presentation::Query {
                parameter: "X-Plex-Token".to_owned(),
            },
            format!("{}&X-Plex-Token={MEMBER}&X-Plex-Token={MEMBER}", about("7")),
            Vec::new(),
        ),
    ];
    let mut said = Vec::new();
    for (presented, uri, headers) in cases {
        said.push(asked(kit(presented, &heard), &uri, &headers).await);
    }
    assert_eq!(
        said,
        [
            StatusCode::NO_CONTENT,
            StatusCode::NO_CONTENT,
            StatusCode::NO_CONTENT,
            StatusCode::UNAUTHORIZED
        ]
    );
}

#[tokio::test]
async fn a_question_naming_no_item_one_twice_or_one_out_of_shape_is_answered_may_not() {
    let heard = Heard::default();
    let long = "1".repeat(ITEM_MOST + 1);
    for uri in [
        "/lemonfiber/media.serve/v1/may".to_owned(),
        format!("{}&item=8", about("7")),
        about("..%2Fadmin"),
        about(""),
        about(&long),
    ] {
        let said = asked(kit(plex(), &heard), &uri, &[("x-plex-token", MEMBER)]).await;
        assert_eq!(said, StatusCode::NOT_FOUND, "{uri}");
    }
    assert!(heard.lock().is_ok_and(|heard| heard.is_empty()));
}

#[tokio::test]
async fn an_adapter_nobody_asks_about_members_answers_no_question() {
    let said = asked(
        Kit::new(KEY, about_nothing()),
        &about("7"),
        &[("x-plex-token", MEMBER)],
    )
    .await;
    assert_eq!(said, StatusCode::NOT_FOUND);
}
