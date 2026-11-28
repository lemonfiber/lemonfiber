use std::sync::Arc;

use axum::http::StatusCode;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::keys::Scope;

use super::{carried_in, permitted};
use crate::admission::{Caller, Keyed};
use crate::device_keys::A_DEVICE_KEY;
use crate::keys::A_KEY;
use crate::push_endpoints::A_PUSH_ENDPOINT;

fn surface() -> Arc<Ctx> {
    Arc::new(lemonfiber_testing::a_context().build())
}

fn rehearses(body: &str) -> Option<bool> {
    carried_in(&surface(), body.as_bytes())
        .ok()
        .map(|ctx| ctx.dry_run)
}

#[test]
fn no_body_and_dry_run_false_revoke_and_dry_run_true_rehearses() {
    assert_eq!(rehearses(""), Some(false));
    assert_eq!(rehearses("{}"), Some(false));
    assert_eq!(rehearses(r#"{"dry_run":false}"#), Some(false));
    assert_eq!(rehearses(r#"{"dry_run":true}"#), Some(true));
}

#[test]
fn a_body_that_is_not_dry_run_alone_is_refused_before_anything_is_revoked() {
    for body in [
        r#"{"dry_run":true,"name":"ha"}"#,
        r#"{"dry_run":"yes"}"#,
        "true",
        "{",
    ] {
        let refused = carried_in(&surface(), body.as_bytes()).err();
        assert_eq!(
            refused.map(|response| response.status()),
            Some(StatusCode::BAD_REQUEST),
            "{body}"
        );
    }
}

#[test]
fn each_revocation_is_permitted_as_its_route_decides() {
    let each = |caller: &Caller| permitted(caller).map(|(_, permitted)| permitted);
    assert_eq!(
        permitted(&Caller::Operator).map(|(path, _)| path),
        [A_KEY, A_DEVICE_KEY, A_PUSH_ENDPOINT]
    );
    assert_eq!(each(&Caller::Operator), [true, true, true]);
    assert_eq!(each(&Caller::Machine), [true, true, true]);
    assert_eq!(each(&Caller::Member("ana".to_owned())), [true, false, true]);
    let key = Caller::Key(Keyed {
        name: "ha".to_owned(),
        scope: Scope::Act,
    });
    assert_eq!(each(&key), [false, false, false]);
}
