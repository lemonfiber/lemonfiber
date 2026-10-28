//! The envelope a stream guard is handed before its own requests are asked: the
//! declaration the media server the bundle fills `identity.source` with is recorded
//! answering, a stand-in for that media server as its upstream, and a stand-in pass
//! service.

use lemonfiber_contract::capabilities::identity::source;
use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::conformance::cases;
use lemonfiber_manifest::container::STREAM_GUARD;
use lemonfiber_ports::service::{Declared, Guarded};
use lemonfiber_ports::serving::StandIn;
use serde_json::Value;

use super::speaking::fixture;
use super::Faulted;
use crate::plugin::container::LOOPBACK;
use crate::plugin::installed::Placed;

/// The host the guard reaches the stand-in media server by.
const UPSTREAM: &str = "recording-media-server";

/// The port the stand-in media server answers on.
const UPSTREAM_PORT: u16 = 58_082;

/// The host the guard asks the stand-in pass service at.
const PASS: &str = "recording-pass";

/// The port the stand-in pass service answers on.
const PASS_PORT: u16 = 58_081;

/// What the step that hands a guard its envelope is called where it fails.
pub(super) const STEP: &str = "envelope";

/// A plugin's files, each by its path from the plugin's root.
type Files<'a> = &'a [(&'a str, &'a [u8])];

/// Where the envelope's declaration sits in a recording.
const ANSWERED: &str = "/response/json";

/// Whether `placed` is a stream guard, which enforces an envelope before it answers.
pub(super) fn guards(placed: &Placed) -> bool {
    placed.provides.iter().any(|name| name == STREAM_GUARD)
}

/// What the media server the bundle fills `identity.source` with, the one a guard stands
/// in front of, is recorded declaring over `media.serve`.
///
/// # Errors
///
/// Where the bundle names no such media server, or none of its files is a recording of
/// the declaration that reads as one.
pub(super) fn declared() -> Result<Declared, Faulted> {
    let capability = serve::capability();
    let spoken = lemonfiber_contract::spoken(serve::CAPABILITY, serve::MAJOR);
    let case = cases(&capability)
        .into_iter()
        .find(|case| case.operation.name == "declared")
        .map(|case| fixture(&spoken, &case.case))
        .unwrap_or_default();
    let filler = lemonfiber_bundled::PINNED.iter().find(|pinned| {
        pinned.fills.contains(&source::CAPABILITY) && pinned.fills.contains(&serve::CAPABILITY)
    });
    read(
        filler.map(|pinned| (pinned.first_party.plugin, pinned.files)),
        &case,
    )
}

/// The declaration the recording at `case` among `filler`'s files answers, `filler` as
/// its plugin and its files.
fn read(filler: Option<(&str, Files<'_>)>, case: &str) -> Result<Declared, Faulted> {
    answered(filler, case).map_err(|why| Faulted::new(STEP, why, Vec::new()))
}

/// The declaration the recording at `case` among `filler`'s files answers, or why there
/// is none.
fn answered(filler: Option<(&str, Files<'_>)>, case: &str) -> Result<Declared, String> {
    let (plugin, files) = filler.ok_or_else(|| {
        format!(
            "the bundle names no filler of {} that fills {}",
            source::CAPABILITY,
            serve::CAPABILITY
        )
    })?;
    let (_, bytes) = files
        .iter()
        .find(|(path, _)| *path == case)
        .ok_or_else(|| format!("{plugin} carries no {case}"))?;
    let recorded: Value =
        serde_json::from_slice(bytes).map_err(|why| format!("{case} does not read: {why}"))?;
    let answer = recorded
        .pointer(ANSWERED)
        .cloned()
        .ok_or_else(|| format!("{case} holds no answer"))?;
    serde_json::from_value(answer).map_err(|why| format!("{case} is not a declaration: {why}"))
}

/// The envelope the guard is handed: `declared`, behind the stand-in media server and
/// asking the stand-in pass service.
pub(super) fn guarded(declared: Declared) -> Guarded {
    Guarded {
        declared,
        upstream: format!("http://{UPSTREAM}:{UPSTREAM_PORT}"),
        adapter: None,
        proxy: LOOPBACK.to_owned(),
        pass: format!("http://{PASS}:{PASS_PORT}/pass"),
    }
}

/// Each stand-in the envelope points the guard at, by the host it names it on: the media
/// server and the pass service, answering nothing, so the guard decides every request
/// its own requests make on its own.
pub(super) fn parties() -> Vec<(&'static str, StandIn)> {
    let refusing = |port: u16| StandIn {
        port,
        headers: Vec::new(),
        answers: Vec::new(),
    };
    vec![
        (UPSTREAM, refusing(UPSTREAM_PORT)),
        (PASS, refusing(PASS_PORT)),
    ]
}

#[cfg(test)]
mod tests;
