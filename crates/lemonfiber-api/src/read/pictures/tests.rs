use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use axum::response::Response;
use lemonfiber_core::app::{Command, Viewing, Whom};
use lemonfiber_core::keys::Scope;
use lemonfiber_core::screening::Pictured;

use super::{shown, titled};
use crate::admission::{Caller, Keyed};
use crate::entitled::{may, Door, Permitted};
use crate::refusal::Refusal;

/// A title on the shelf, by an id shaped like the media server's.
const FILM: &str = "0123456789abcdef0123456789abcdef";

/// The title read naming `member`.
fn title(member: Whom) -> Command {
    Command::Viewing(Viewing::Title {
        member,
        id: FILM.to_owned(),
    })
}

/// A key with this scope.
fn key(scope: Scope) -> Caller {
    Caller::Key(Keyed {
        name: "home-assistant".to_owned(),
        scope,
    })
}

/// The status and body an answer carries.
async fn said(response: Response) -> (StatusCode, Vec<u8>) {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    (status, body)
}

#[test]
fn a_picture_is_shown_as_its_raster_type_and_kept_by_its_asker_alone() {
    let response = shown(Pictured {
        media_type: "image/webp",
        bytes: b"webp".to_vec(),
    });
    let header = |name| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    };
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(header::CONTENT_TYPE), Some("image/webp"));
    assert_eq!(header(header::CACHE_CONTROL), Some("private"));
    assert_eq!(header(header::X_CONTENT_TYPE_OPTIONS), Some("nosniff"));
}

#[test]
fn a_picture_is_read_as_whoever_the_title_read_is_ruled_to_be_read_as() {
    let ana = Whom::Named("a7f3".to_owned());
    let bo = Whom::Named("bo".to_owned());
    let their_key = key(Scope::Member {
        id: "a7f3".to_owned(),
        name: "ana".to_owned(),
    });
    for (caller, asked, read_as) in [
        (Caller::Member("a7f3".to_owned()), bo.clone(), ana.clone()),
        (their_key, bo.clone(), ana.clone()),
        (Caller::Operator, ana.clone(), ana),
        (Caller::Machine, bo.clone(), bo.clone()),
        (key(Scope::Read), bo.clone(), bo),
        (key(Scope::Act), Whom::Defaults, Whom::Defaults),
    ] {
        let granted = may(&caller, Door::Reading, title(asked)).granted();
        assert_eq!(
            titled(granted).ok(),
            Some((read_as, FILM.to_owned())),
            "{caller:?}"
        );
    }
}

#[tokio::test]
async fn a_ruling_that_grants_no_title_read_is_answered_with_its_own_refusal() {
    let refusals: [fn() -> Permitted; 2] = [
        || Permitted::Nothing,
        || Permitted::NotForAKey("read".to_owned()),
    ];
    for refusal in refusals {
        let (Err(answered), Err(expected)) = (titled(refusal().granted()), refusal().granted())
        else {
            unreachable!("a refusal is never granted")
        };
        assert_eq!(said(*answered).await, said(*expected).await);
    }
    let Err(answered) = titled(Ok(Command::Version)) else {
        unreachable!("the versions in play are not a title read")
    };
    assert_eq!(
        said(*answered).await,
        said(Refusal::NoSuchRead.answered()).await
    );
}
