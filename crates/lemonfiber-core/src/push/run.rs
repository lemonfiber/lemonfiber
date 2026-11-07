//! Registering, listing and removing push endpoints, and waking them.
//!
//! Every registration and removal is journaled by name alone, an endpoint's distributor
//! answering that it is gone among them. Nothing reverses one: only the phone holds what a
//! registration is made from.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

use super::{sealing, Kept, Listed, Listing, Record, Registered, FILE};
use crate::alert::Alert;
use crate::app::{Ctx, Outcome};
use crate::config::store::Failure;
use crate::config::REACH_WAKING_KEY;
use crate::error::codes::push::{
    NOT_A_PUSH_NAME, PUSH_KEYS_UNREADABLE, PUSH_NAME_TAKEN, PUSH_NOT_HTTPS, PUSH_NOT_REGISTERED,
};
use crate::error::{Diagnose, Problem, Remedy, State};
use crate::journal::{Change, Kind};
use crate::keys::names_a_key;
use crate::ports::http::{Method, Request};

/// What the endpoint's distributor is asked to hold a message for, a day being as long as
/// a returning stream is replayed.
const HELD_FOR_SECONDS: &str = "86400";

/// How long one endpoint is waited for, so one that hangs delays a gather no longer.
const WAITED_FOR: Duration = Duration::from_secs(10);

/// The operation every journal entry about a push endpoint is written under.
const OPERATION: &str = "push-endpoint";

/// The statuses a distributor answers an endpoint that is no more with.
const GONE: [u16; 2] = [404, 410];

/// What was asked of the push endpoints.
#[derive(Clone, PartialEq, Eq)]
pub enum Asked {
    /// Register an endpoint under a name.
    Register {
        /// What to call it.
        name: String,
        /// The `https` address its distributor gave.
        endpoint: String,
        /// The phone's `WebPush` public key, in base64url.
        p256dh: String,
        /// The phone's `WebPush` secret, in base64url.
        auth: String,
    },
    /// List the endpoints.
    List,
    /// Remove the endpoint holding a name.
    Remove {
        /// The endpoint's name.
        name: String,
    },
}

/// What was asked, its keys and its address withheld.
impl std::fmt::Debug for Asked {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Register { name, .. } => out
                .debug_struct("Register")
                .field("name", name)
                .finish_non_exhaustive(),
            Self::List => out.write_str("List"),
            Self::Remove { name } => out.debug_struct("Remove").field("name", name).finish(),
        }
    }
}

/// Where this machine keeps its push endpoints, or nothing where it keeps no configuration.
fn at(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, FILE)
}

/// Carry out what was asked of the push endpoints.
///
/// # Errors
///
/// The [`Problem`] whichever of the three refuses with.
pub fn asked(ctx: &Ctx, asked: Asked) -> Result<Outcome, Box<Problem>> {
    match asked {
        Asked::Register {
            name,
            endpoint,
            p256dh,
            auth,
        } => register(ctx, name, endpoint, p256dh, auth).map(Outcome::PushEndpoint),
        Asked::List => listing(ctx, None).map(Outcome::PushEndpoints),
        Asked::Remove { name } => remove(ctx, &name).map(Outcome::PushEndpoints),
    }
}

/// Register an endpoint, once its name, its address and its keys can be used.
fn register(
    ctx: &Ctx,
    name: String,
    endpoint: String,
    p256dh: String,
    auth: String,
) -> Result<Registered, Box<Problem>> {
    if !names_a_key(&name) {
        return Err(Box::new(bad_name(&name)));
    }
    if !is_https(&endpoint) {
        return Err(Box::new(not_https(&name)));
    }
    if sealing::readable(&p256dh, &auth).is_none() {
        return Err(Box::new(unreadable_keys(&name)));
    }
    let path = at(ctx).ok_or_else(nowhere)?;
    let record = Record {
        name,
        endpoint,
        p256dh,
        auth,
        registered: crate::instant::written(ctx.seams.clock.now()).unwrap_or_default(),
    };
    let answer = Registered {
        name: record.name.clone(),
        endpoint: record.endpoint.clone(),
        registered: record.registered.clone(),
        rehearsed: ctx.dry_run,
    };
    let mut refused = None;
    let kept = if ctx.dry_run {
        read(&path).map(|kept| refused = taken(&kept, &record.name))
    } else {
        crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
            refused = taken(kept, &record.name).or_else(|| {
                journalled(
                    ctx,
                    &record.name,
                    Kind::PushEndpointRegistered {
                        name: record.name.clone(),
                    },
                )
                .err()
            });
            if refused.is_none() {
                kept.endpoints.push(record.clone());
            }
            Ok(())
        })
    };
    kept.map_err(|failure| Box::new(failure.problem()))?;
    refused.map_or(Ok(answer), Err)
}

/// Remove the endpoint holding `name`.
fn remove(ctx: &Ctx, name: &str) -> Result<Listing, Box<Problem>> {
    let path = at(ctx).ok_or_else(nowhere)?;
    let mut refused = None;
    if ctx.dry_run {
        let kept = read(&path).map_err(|failure| Box::new(failure.problem()))?;
        if !kept.endpoints.iter().any(|one| one.name == name) {
            return Err(Box::new(not_registered(name)));
        }
    } else {
        crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
            refused = if kept.endpoints.iter().any(|one| one.name == name) {
                forgotten(ctx, kept, name).err()
            } else {
                Some(Box::new(not_registered(name)))
            };
            Ok(())
        })
        .map_err(|failure| Box::new(failure.problem()))?;
    }
    refused.map_or_else(|| listing(ctx, Some(name.to_owned())), Err)
}

/// Take `name` out of `kept`, once the journal says so, and forget what was sent with the
/// last of them, so a phone registered later is not sent what was said meanwhile.
fn forgotten(ctx: &Ctx, kept: &mut Kept, name: &str) -> Result<(), Box<Problem>> {
    journalled(
        ctx,
        name,
        Kind::PushEndpointRemoved {
            name: name.to_owned(),
        },
    )?;
    kept.endpoints.retain(|one| one.name != name);
    if kept.endpoints.is_empty() {
        kept.told = None;
    }
    Ok(())
}

/// Every endpoint, and the one this request removed.
fn listing(ctx: &Ctx, removed: Option<String>) -> Result<Listing, Box<Problem>> {
    let path = at(ctx).ok_or_else(nowhere)?;
    let kept = read(&path).map_err(|failure| Box::new(failure.problem()))?;
    Ok(Listing {
        endpoints: kept.endpoints.iter().map(Listed::from).collect(),
        removed,
        rehearsed: ctx.dry_run,
    })
}

/// Whether any endpoint is waiting to be woken, so the stack is worth reading with nobody
/// listening: none where waking is switched off, or where the record cannot be read.
#[must_use]
pub fn any_waiting(ctx: &Ctx) -> bool {
    ctx.settings.reaching.allows(REACH_WAKING_KEY)
        && at(ctx)
            .and_then(|path| read(&path).ok())
            .is_some_and(|kept| !kept.endpoints.is_empty())
}

/// Send every endpoint one message for each alert in `alerts` none has been sent, and
/// forget each endpoint its distributor answers is gone.
pub async fn woken(ctx: &Ctx, alerts: &[Alert]) {
    if !any_waiting(ctx) {
        return;
    }
    let Some(path) = at(ctx) else {
        return;
    };
    let carried: BTreeSet<String> = alerts.iter().filter_map(said_as).collect();
    let mut owed = 0;
    let mut endpoints = Vec::new();
    let written = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
        let before: Option<BTreeSet<&String>> =
            kept.told.as_ref().map(|told| told.iter().collect());
        owed = before.map_or(0, |before| {
            carried.iter().filter(|one| !before.contains(one)).count()
        });
        kept.told = Some(carried.iter().cloned().collect());
        endpoints.clone_from(&kept.endpoints);
        Ok(())
    });
    if written.is_err() || owed == 0 {
        return;
    }
    let sends = endpoints.iter().map(|record| woke(ctx, record, owed));
    let answered = futures_util::future::join_all(sends).await;
    let gone: Vec<&str> = endpoints
        .iter()
        .zip(answered)
        .filter(|(_, gone)| *gone)
        .map(|(record, _)| record.name.as_str())
        .collect();
    forget_all(ctx, &path, &gone);
}

/// What names an alert and which way it went, where anything does.
fn said_as(alert: &Alert) -> Option<String> {
    Some(format!("{} {}", alert.id.as_ref()?, alert.moment.said()))
}

/// Send `record` `owed` messages, and whether its distributor answered that it is gone.
async fn woke(ctx: &Ctx, record: &Record, owed: usize) -> bool {
    let keys = URL_SAFE_NO_PAD
        .decode(&record.p256dh)
        .ok()
        .zip(URL_SAFE_NO_PAD.decode(&record.auth).ok());
    let Some((public, secret)) = keys else {
        return false;
    };
    let request = Request {
        method: Method::Post,
        url: record.endpoint.clone(),
        headers: vec![
            ("Content-Encoding".to_owned(), "aes128gcm".to_owned()),
            (
                "Content-Type".to_owned(),
                "application/octet-stream".to_owned(),
            ),
            ("TTL".to_owned(), HELD_FOR_SECONDS.to_owned()),
            ("Urgency".to_owned(), "high".to_owned()),
        ],
        body: None,
        pinned: None,
    };
    for _ in 0..owed {
        let Some(sealed) = sealing::sealed(&public, &secret) else {
            return false;
        };
        let sent = tokio::time::timeout(WAITED_FOR, ctx.seams.http.send_octets(&request, &sealed));
        if let Ok(Ok(answered)) = sent.await {
            if GONE.contains(&answered.status) {
                return true;
            }
        }
    }
    false
}

/// Forget each of `gone`, best effort: one still kept is told again and answers again.
fn forget_all(ctx: &Ctx, path: &Path, gone: &[&str]) {
    if gone.is_empty() {
        return;
    }
    let _ = crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        for name in gone {
            let _ = forgotten(ctx, kept, name);
        }
        Ok(())
    });
}

/// The record at `path`.
fn read(path: &Path) -> Result<Kept, Failure> {
    crate::settings::kept::read(path)
}

/// The refusal of a registration under a name `kept` holds, where it holds it.
fn taken(kept: &Kept, name: &str) -> Option<Box<Problem>> {
    kept.endpoints
        .iter()
        .any(|one| one.name == name)
        .then(|| Box::new(name_taken(name)))
}

/// Whether `endpoint` is an `https` address with a host.
fn is_https(endpoint: &str) -> bool {
    url::Url::parse(endpoint).is_ok_and(|url| url.scheme() == "https" && url.has_host())
}

/// Write a change to the journal, by the endpoint's name alone.
fn journalled(ctx: &Ctx, name: &str, kind: Kind) -> Result<(), Box<Problem>> {
    let paths = crate::app::targets::layout(ctx).ok_or_else(nowhere)?;
    let change = Change {
        at: ctx.stamp(),
        operation: OPERATION.to_owned(),
        target: name.to_owned(),
        kind,
    };
    crate::app::recover::journalled(&paths.journal(), &[change])
        .map_err(|failure| Box::new(failure.problem()))
}

/// There is nowhere to keep an endpoint.
fn nowhere() -> Box<Problem> {
    Box::new(Failure::Nowhere.problem())
}

/// The word cannot name an endpoint.
fn bad_name(name: &str) -> Problem {
    Problem::new(
        NOT_A_PUSH_NAME,
        format!("{name} cannot name a push endpoint"),
        "A name is lower-case letters, digits, dots, dashes and underscores.",
        Remedy::new("Name it after the phone it wakes, as in pixel-8"),
    )
    .in_state(State::Guided)
}

/// Another endpoint holds the name.
fn name_taken(name: &str) -> Problem {
    Problem::new(
        PUSH_NAME_TAKEN,
        format!("A push endpoint named {name} is already registered"),
        "Nothing was registered.",
        Remedy::new("Remove that one, or register this one under another name"),
    )
    .in_state(State::Guided)
}

/// The address is not `https`.
fn not_https(name: &str) -> Problem {
    Problem::new(
        PUSH_NOT_HTTPS,
        format!("The address given for the push endpoint {name} is not an https address"),
        "A push endpoint is reached over https and nothing else, so nothing was registered.",
        Remedy::new("Register the https address the phone's distributor gives"),
    )
    .in_state(State::Guided)
}

/// The keys cannot be encrypted to.
fn unreadable_keys(name: &str) -> Problem {
    Problem::new(
        PUSH_KEYS_UNREADABLE,
        format!("The WebPush keys given for the push endpoint {name} cannot be encrypted to"),
        "They are a P-256 public key and a 16-byte secret, each in base64url, so nothing was \
         registered.",
        Remedy::new("Make a new WebPush key pair on the phone, and register that"),
    )
    .in_state(State::Guided)
}

/// No endpoint holds the name.
fn not_registered(name: &str) -> Problem {
    Problem::new(
        PUSH_NOT_REGISTERED,
        format!("No push endpoint is named {name}"),
        "Nothing was removed.",
        Remedy::new("List the push endpoints, and name one of them"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
