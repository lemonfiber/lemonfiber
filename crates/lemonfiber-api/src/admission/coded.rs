//! The door a household member signs in at by a code the outside service holding their
//! account shows them, never by a password.
//!
//! The id the identity source answers for a code is the one handle on a token to come, so
//! it never leaves the core: the caller is handed a handle the core drew, and the id is
//! kept against it and the session that asked, for that caller alone.

use std::net::SocketAddr;

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Extension, Json};
use lemonfiber_core::instant::Instant;
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::ports::service::Failure;
use lemonfiber_core::sign_in_codes::{self, Asker, NotKept};
use serde::{Deserialize, Serialize};

use super::claiming::{device, vouched};
use super::sessions::Opened;
use crate::guard::{Arrived, Presented};
use crate::read::enveloped;
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

/// A code to sign in by, or the sign-in by a code still waiting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SignInCode {
    /// The code the member enters where they approve it, where one was just asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Where the member approves the code, where one was just asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approve_at: Option<String>,
    /// When the code lapses.
    pub lapses_at: Instant,
    /// The handle that finishes signing in by the code.
    pub waiting: String,
}

/// The body the door takes, with the route it is sent to.
pub(crate) fn body() -> (&'static str, schemars::Schema) {
    (CODE, schemars::schema_for!(Given))
}

/// Ask for a code to sign in by, or finish signing in by one.
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
    let asker = Asker::new(
        presented
            .as_ref()
            .map(|Extension(Presented(carried))| carried.as_str()),
        from,
    );
    match given.waiting {
        None => asked(&serving, asker).await,
        Some(handle) => finished(&serving, &asker, handle).await,
    }
}

/// A code asked for by `asker`, kept under a fresh handle.
async fn asked(serving: &Serving, asker: Asker) -> Response {
    let ctx = &serving.ctx;
    let limits = sign_in_codes::limits(ctx);
    let codes = &serving.admitting.codes;
    if let Err(left) = codes.asking(&asker, &limits, ctx.seams.clock.now()) {
        return too_many(left.as_secs());
    }
    let Some(household) = serving.admitting.household_now().await else {
        return Refusal::Unconfirmed.answered();
    };
    let random = ctx.seams.random.as_ref();
    let Some(device) = device(random) else {
        return Refusal::Unconfirmed.answered();
    };
    let issued = match household.sign_in_by_code(&device).await {
        Ok(issued) => issued,
        Err(Failure::Unsupported { .. }) => return Refusal::SignsInByPassword.answered(),
        Err(_) => return Refusal::Unconfirmed.answered(),
    };
    let kept = codes.kept(
        random,
        (issued.id, asker, issued.lapses_at.time()),
        &limits,
        ctx.seams.clock.now(),
    );
    match kept {
        Ok(waiting) => answered(SignInCode {
            code: Some(issued.code),
            approve_at: Some(issued.approve_at),
            lapses_at: issued.lapses_at,
            waiting,
        }),
        Err(not_kept) => unkept(not_kept),
    }
}

/// The refusal a code the table would not keep is answered with.
fn unkept(not_kept: NotKept) -> Response {
    match not_kept {
        NotKept::Waiting(left) => too_many(left.as_secs()),
        NotKept::Undrawn => Refusal::Unconfirmed.answered(),
    }
}

/// The sign-in by the code `handle` names, finished for `asker` once the member approved
/// it, or still waiting.
async fn finished(serving: &Serving, asker: &Asker, handle: String) -> Response {
    let ctx = &serving.ctx;
    let limits = sign_in_codes::limits(ctx);
    let codes = &serving.admitting.codes;
    let Some(finishing) = codes.claimed(&handle, asker, &limits, ctx.seams.clock.now()) else {
        return Refusal::NotWaiting.answered();
    };
    let (Some(at), Some(household)) = (
        serving.admitting.household.clone(),
        serving.admitting.household_now().await,
    ) else {
        return Refusal::Unconfirmed.answered();
    };
    let signed = match household.signed_by_code(&finishing.id).await {
        Ok(Some(signed)) => signed,
        Ok(None) => {
            return answered(SignInCode {
                code: None,
                approve_at: None,
                lapses_at: Instant::of(finishing.lapses),
                waiting: handle,
            })
        }
        Err(_) => return Refusal::Unconfirmed.answered(),
    };
    finishing.spent();
    if !vouched(at, &signed.id).await {
        return Refusal::NotWaiting.answered();
    }
    let Some(name) = serving.admitting.member_named(&signed.id).await else {
        return Refusal::Unconfirmed.answered();
    };
    let opened = serving
        .admitting
        .sessions
        .opened(
            ctx.seams.random.as_ref(),
            ctx.seams.clock.now(),
            Opened::Member(signed),
        )
        .await
        .map(|opened| opened.called(Some(name)));
    enveloped(
        StatusCode::OK,
        opened.and_then(|opened| Envelope::new(kind::ADMISSION, opened).to_json()),
    )
}

/// The code or the sign-in still waiting, as the door answers it.
fn answered(code: SignInCode) -> Response {
    enveloped(
        StatusCode::OK,
        Envelope::new(kind::SIGN_IN_CODE, code).to_json(),
    )
}

/// Too many codes asked for by one source, and how many seconds are left.
fn too_many(seconds: u64) -> Response {
    let seconds = seconds.max(1);
    let mut response = Refusal::TooManyCodes.saying(format!(
        "Too many sign-in codes asked for. Try again in {seconds} seconds."
    ));
    response
        .headers_mut()
        .insert(super::RETRY_AFTER, axum::http::HeaderValue::from(seconds));
    response
}

#[cfg(test)]
mod tests;
