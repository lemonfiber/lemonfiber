//! What this stack can do, as the credential that asked may do it.
//!
//! A client on a phone can be older or newer than the stack it talks to, so it reads
//! what the stack can do rather than deducing it from a version. Each capability is
//! named by the path the surface serves its request at, and the set is generated from
//! the lists the surface routes by: a request the surface gains is a capability it
//! declares, and one it does not have is absent.
//!
//! **Decided where every request is decided.** Whether this credential may have a
//! request is [`may`]'s answer for the command that request reaches, the one a
//! request itself meets, so there is no second permission model here. A request is
//! named by every argument any request could carry, which reaches its command
//! without carrying anything out: nothing here dispatches.
//!
//! **Unconfigured is a setting that is off.** A request the core refuses until a
//! setting this machine has switched off is turned back on is the stack's to offer
//! once it is, and is said to be so rather than missing or forbidden.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{switched, Command, Ctx};
use lemonfiber_core::model::{kind, Envelope};
use serde::Serialize;

use crate::actions::{reached, Arguments, ACTION};
use crate::admission::Caller;
use crate::entitled::{may, Door, Permitted};
use crate::read::enveloped;
use crate::read::table::{self, Wanted};
use crate::router::Serving;

/// Where what this stack can do is read.
pub const CAPABILITIES: &str = "/api/capabilities";

/// What one capability comes to for the credential that asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "CapabilityState")]
pub enum Standing {
    /// This stack can do it, and this credential may.
    Available,
    /// The stack has it, and a setting this machine has switched off has to be turned
    /// back on first.
    Unconfigured,
    /// The stack has it, and this credential may not ask for it.
    Unpermitted,
}

/// Every capability this stack has, by the path its request is served at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "Capabilities")]
pub struct Capabilities {
    /// What each comes to for the credential that asked. A request this stack does
    /// not have is absent rather than listed as anything.
    pub capabilities: BTreeMap<String, Standing>,
}

/// The route the capabilities are read at.
pub fn routes() -> Router<Serving> {
    Router::new().route(
        CAPABILITIES,
        get(
            |State(serving): State<Serving>, caller: Caller| async move {
                answered(&serving.ctx, &caller)
            },
        ),
    )
}

/// The capabilities, in the envelope every read answers with.
fn answered(ctx: &Ctx, caller: &Caller) -> Response {
    let said = Envelope::new(kind::CAPABILITIES, &declared(ctx, caller)).to_json();
    enveloped(StatusCode::OK, said)
}

/// What this stack can do, as `caller` may do it.
#[must_use]
pub fn declared(ctx: &Ctx, caller: &Caller) -> Capabilities {
    let actions = crate::actions::OFFERED.iter().map(|action| {
        (
            served_at(action),
            Door::Acting,
            reached(action, Arguments::naming_everything()).ok(),
        )
    });
    let reads = table::OFFERED.iter().map(|read| {
        (
            (*read).to_owned(),
            Door::Reading,
            table::named(read, Wanted::naming_everything()).ok(),
        )
    });
    Capabilities {
        capabilities: actions
            .chain(reads)
            .filter_map(|(path, door, command)| Some((path, standing(ctx, caller, door, command?))))
            .collect(),
    }
}

/// The path the action named `action` is served at.
fn served_at(action: &str) -> String {
    ACTION.replace("{action}", action)
}

/// What one request comes to for `caller`: what [`may`] answers for its command at the
/// door it arrives at, and whether a setting it needs is off.
fn standing(ctx: &Ctx, caller: &Caller, door: Door, command: Command) -> Standing {
    match may(caller, door, command) {
        Permitted::Nothing | Permitted::NotForAKey(_) => Standing::Unpermitted,
        Permitted::This(command) => match switched::off(ctx, &command) {
            Some(_) => Standing::Unconfigured,
            None => Standing::Available,
        },
    }
}

#[cfg(test)]
mod tests;
