//! Registering, listing and removing push endpoints, and waking them.
//!
//! Reading the stack only says what is owed: each endpoint's delivery runs on its own, so an
//! endpoint that hangs costs that read nothing, and what is owed behind it is bounded.
//!
//! Every registration and removal is journaled by name alone, an endpoint's distributor
//! answering that it is gone among them. Nothing reverses one: only the phone holds what a
//! registration is made from.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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

/// How long one endpoint is waited for on one message.
pub const WAITED_FOR: Duration = Duration::from_secs(10);

/// The most messages an endpoint is owed: as many alerts as a returning stream replays.
const MOST_OWED: usize = crate::alert::KEPT;

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
        owed: 0,
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

/// Where the record of the endpoints waiting to be woken is, where any are: nowhere where
/// waking is switched off, or where the record cannot be read.
fn waiting_at(ctx: &Ctx) -> Option<PathBuf> {
    at(ctx)
        .filter(|_| ctx.settings.reaching.allows(REACH_WAKING_KEY))
        .filter(|path| read(path).is_ok_and(|kept| !kept.endpoints.is_empty()))
}

/// Every endpoint as a back-end the stack delivers to, by the host its distributor is on.
pub(crate) fn as_backends(ctx: &Ctx) -> Vec<crate::model::BackendReport> {
    at(ctx)
        .and_then(|path| read(&path).ok())
        .map(|kept| kept.endpoints.iter().map(as_backend).collect())
        .unwrap_or_default()
}

/// One endpoint as a back-end.
fn as_backend(record: &Record) -> crate::model::BackendReport {
    crate::model::BackendReport {
        id: record.name.clone(),
        kind: crate::model::BackendKind::Unifiedpush,
        reaches: url::Url::parse(&record.endpoint)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_default(),
        chosen_by: crate::model::ChosenBy::Operator,
        hosted: None,
        proof: crate::model::BackendProof::Unproven,
        last: None,
        failures: Vec::new(),
        retry: None,
    }
}

/// Whether any endpoint is waiting to be woken, so the stack is worth reading with nobody
/// listening.
#[must_use]
pub fn any_waiting(ctx: &Ctx) -> bool {
    waiting_at(ctx).is_some()
}

/// The deliveries under way, one to an endpoint, each apart from whatever reads the stack
/// and from every other endpoint's, so one that hangs delays nothing but itself.
#[derive(Debug, Clone, Default)]
pub struct Waking {
    /// The name of each endpoint a delivery is under way to.
    delivering: Arc<Mutex<BTreeSet<String>>>,
}

impl Waking {
    /// Owe every endpoint one message for each alert in `alerts` none has been owed, and
    /// start delivering to each endpoint owed any. Waits on no endpoint.
    pub fn owing(&self, ctx: &Arc<Ctx>, alerts: &[Alert]) {
        let Some(path) = waiting_at(ctx) else {
            return;
        };
        let carried: BTreeSet<String> = alerts.iter().filter_map(said_as).collect();
        let mut owing = Vec::new();
        let written = crate::settings::kept::changed(&path, &mut |kept: &mut Kept| {
            let new = kept.told.as_ref().map_or(0, |told| {
                carried.iter().filter(|one| !told.contains(one)).count()
            });
            kept.told = Some(carried.iter().cloned().collect());
            for one in &mut kept.endpoints {
                one.owed = one.owed.saturating_add(new).min(MOST_OWED);
            }
            owing = kept
                .endpoints
                .iter()
                .filter(|one| one.owed > 0)
                .map(|one| one.name.clone())
                .collect();
            Ok(())
        });
        for name in owing.into_iter().filter(|_| written.is_ok()) {
            self.delivering_to(ctx, &path, name);
        }
    }

    /// Deliver what `name` is owed, unless a delivery to it is already under way.
    ///
    /// One that has just found nothing owed can miss what is owed a moment later, and the
    /// next reading starts another for it.
    fn delivering_to(&self, ctx: &Arc<Ctx>, path: &Path, name: String) {
        let started = self
            .delivering
            .lock()
            .is_ok_and(|mut delivering| delivering.insert(name.clone()));
        if started {
            let delivering = Arc::clone(&self.delivering);
            let ctx = Arc::clone(ctx);
            let path = path.to_path_buf();
            tokio::spawn(async move {
                delivered(&ctx, &path, &name).await;
                let _ = delivering
                    .lock()
                    .map(|mut delivering| delivering.remove(&name));
            });
        }
    }
}

/// Send `name` everything it is owed, one message at a time, and forget it if its
/// distributor answers that it is gone.
async fn delivered(ctx: &Ctx, path: &Path, name: &str) {
    while let Some(record) = next_owed(path, name) {
        if woke(ctx, &record).await {
            forget(ctx, path, name);
            return;
        }
    }
}

/// `name`'s record, with the message about to be sent no longer owed: nothing where it owes
/// none, has been removed, or the record cannot be written, so nothing is sent twice.
fn next_owed(path: &Path, name: &str) -> Option<Record> {
    let mut next = None;
    let written = crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        next = kept
            .endpoints
            .iter_mut()
            .find(|one| one.name == name && one.owed > 0)
            .map(|one| {
                one.owed -= 1;
                one.clone()
            });
        Ok(())
    });
    written.ok().and(next)
}

/// What names an alert and which way it went, where anything does.
fn said_as(alert: &Alert) -> Option<String> {
    Some(format!("{} {}", alert.id.as_ref()?, alert.moment.said()))
}

/// The message that wakes `record`, where its keys can be read.
fn sealed_for(record: &Record) -> Option<Vec<u8>> {
    let public = URL_SAFE_NO_PAD.decode(&record.p256dh).ok()?;
    let secret = URL_SAFE_NO_PAD.decode(&record.auth).ok()?;
    sealing::sealed(&public, &secret)
}

/// Send `record` one message, and whether its distributor answered that it is gone.
async fn woke(ctx: &Ctx, record: &Record) -> bool {
    let Some(sealed) = sealed_for(record) else {
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
    let sent = tokio::time::timeout(WAITED_FOR, ctx.seams.http.send_octets(&request, &sealed));
    matches!(sent.await, Ok(Ok(answered)) if GONE.contains(&answered.status))
}

/// Forget `name`, best effort: one still kept is told again and answers again.
fn forget(ctx: &Ctx, path: &Path, name: &str) {
    let _ = crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        let _ = forgotten(ctx, kept, name);
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
