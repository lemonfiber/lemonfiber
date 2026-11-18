//! The door a household member signs in at by a code the outside service holding their
//! account shows them, never by a password.

use std::net::SocketAddr;

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, State};
use axum::response::Response;
use axum::{Extension, Json};
use lemonfiber_core::app::admitting::{Asked, Coded};
use lemonfiber_core::app::Command;
use serde::Deserialize;

use crate::guard::{Arrived, Presented};
use crate::read::carried_out;
use crate::refusal::Refusal;
use crate::router::Serving;

/// Where a member asks for a code to sign in by, and finishes signing in with it.
pub const CODE: &str = "/api/session/code";

/// What a caller sends to the door: nothing to ask for a code, the handle to finish one.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SignInByCode")]
pub(super) struct Given {
    /// The handle a code was answered with, where this finishes signing in by it.
    #[serde(default)]
    waiting: Option<String>,
}

/// The body signing in by a code takes, with the route it is sent to.
pub(crate) fn body() -> (&'static str, schemars::Schema) {
    (CODE, schemars::schema_for!(Given))
}

/// Ask for a code, or finish signing in with one.
pub(super) async fn coded(
    State(serving): State<Serving>,
    arrived: Option<Extension<Arrived>>,
    connected: Option<Extension<ConnectInfo<SocketAddr>>>,
    presented: Option<Extension<Presented>>,
    given: Result<Json<Given>, JsonRejection>,
) -> Response {
    let Ok(Json(given)) = given else {
        return Refusal::NotACodeRequest.answered();
    };
    let from = arrived
        .map(|Extension(arrived)| arrived.from)
        .or_else(|| connected.map(|Extension(ConnectInfo(at))| at.ip()));
    let asked = Coded {
        session: presented.map(|Extension(Presented(carried))| carried),
        from,
        waiting: given.waiting,
    };
    carried_out(&serving.ctx, Command::Admission(Asked::Code(asked))).await
}
