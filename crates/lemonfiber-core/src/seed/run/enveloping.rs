//! The stream guard, applied the envelope it enforces, and the pass service told the media
//! server's live-stream paths, both from the declaration the media server's filler
//! answers over `media.serve`.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, SystemTime};

use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::capabilities::stream::guard;

use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::pass::{File, Live, LivePath, PATH, PORT, SERVICE};
use crate::ports::service::{Declared, Declaring, Guarded, Guarding};
use crate::project::entry::CONFIGURATION_TREE;
use crate::seed::{State, Wiring};
use crate::topology::{PASS_ON_PASS, PROXY_ON_PROXY};
use crate::wiring::{Filler, Fillers};

/// What the report calls the guard's envelope.
pub(super) const ENVELOPE: &str = "The stream guard's envelope";

/// Why a stream guard was handed no envelope while it does not answer over its contract.
const UNANSWERED: &str = "the stream guard does not answer over its contract yet";

/// What the report calls the pass service's live-stream paths.
pub(super) const LIVE: &str = "The pass service's live-stream paths";

/// Apply the stream guard the envelope it enforces where it speaks the contract, and write
/// the pass service the media server's live-stream paths where it runs, each said as a
/// connection of its own: one passing nothing where the media server is `kept_off` the
/// household.
pub(super) async fn seed_envelope(
    ctx: &Ctx,
    fillers: &Fillers,
    project: Option<&Path>,
    kept_off: &BTreeSet<String>,
) -> Vec<Wiring> {
    let Some((guarding, _)) = fillers.filling(guard::CAPABILITY) else {
        return Vec::new();
    };
    Box::pin(envelope_through(ctx, fillers, guarding, project, kept_off)).await
}

/// Apply `guarding` the envelope it enforces where it speaks the contract, and write the
/// pass service the media server's live-stream paths where it runs: none of either where
/// the media server is `kept_off` the household, so the guard passes it nothing.
async fn envelope_through(
    ctx: &Ctx,
    fillers: &Fillers,
    guarding: &Filler,
    project: Option<&Path>,
    kept_off: &BTreeSet<String>,
) -> Vec<Wiring> {
    let Some((served, _)) = fillers.filling(serve::CAPABILITY) else {
        return Vec::new();
    };
    let told = project.filter(|_| crate::app::project::pinned(ctx).is_some());
    let guard = match Box::pin(spoken(ctx, guarding, guard::CAPABILITY, guard::MAJOR)).await {
        Spoken::Over(adapter) => Some(guard::Adapter(adapter)),
        Spoken::Unanswered | Spoken::Not => None,
    };
    if told.is_none() && guard.is_none() {
        return Vec::new();
    }
    let declared = match declared(ctx, served).await {
        Ok(declared) if super::owning::off(fillers, served, kept_off) => closed(declared),
        Ok(declared) => declared,
        Err(state) => {
            let needing = if guard.is_some() { ENVELOPE } else { LIVE };
            return vec![settled(needing, state)];
        }
    };
    let mut wirings = Vec::new();
    if let Some(project) = told {
        wirings.push(settled(LIVE, live(ctx, project, &declared)));
    }
    if let Some(guard) = guard {
        let envelope = enveloped(fillers, served, declared);
        wirings.push(settled(ENVELOPE, applied(ctx, &guard, envelope).await));
    }
    wirings
}

/// `declared` passing nothing: no byte path, no live stream and no question, and every path
/// refused, for a media server kept off the household.
fn closed(declared: Declared) -> Declared {
    Declared {
        bytes: Vec::new(),
        live: Vec::new(),
        refused: vec![EVERY_PATH.to_owned()],
        question: None,
        ..declared
    }
}

/// The pattern refusing every path.
const EVERY_PATH: &str = "^/";

/// How often a guard that does not answer yet is asked again.
pub(super) const POLL: Duration = Duration::from_millis(500);

/// How applying the stream guard among `installed`'s services the envelope it enforces came
/// out, the plugin taken as installed beside every other the register holds, asked again
/// while the guard does not answer over its contract until `until`; nothing where none of
/// its services fills `stream.guard` or the stack cannot be read.
pub(crate) async fn guarding_installed(
    ctx: &Ctx,
    installed: &crate::plugin::Installed,
    until: SystemTime,
) -> Option<State> {
    let (fillers, every) = super::every_beside(ctx, installed)?;
    let (baseline, _) = super::load_baseline(ctx).starting();
    let kept_off = super::owning::kept_off(&fillers, &every, &baseline);
    let guarding = fillers.services().find(|one| {
        one.brought_by() == Some(installed.plugin.as_str())
            && one.provides.iter().any(|name| name == guard::CAPABILITY)
    })?;
    loop {
        let wirings = Box::pin(envelope_through(ctx, &fillers, guarding, None, &kept_off)).await;
        let state = wirings
            .into_iter()
            .find(|wiring| wiring.connection == ENVELOPE)
            .map_or_else(
                || State::Skipped {
                    reason: UNANSWERED.to_owned(),
                },
                |wiring| wiring.state,
            );
        match &state {
            State::Skipped { reason } if reason == UNANSWERED && ctx.seams.clock.now() < until => {
                tokio::time::sleep(POLL).await;
            }
            _ => return Some(state),
        }
    }
}

/// The media server's declaration, checked, as its filler answers it over `media.serve`,
/// or the state that says why there is none.
async fn declared(ctx: &Ctx, served: &Filler) -> Result<Declared, State> {
    let asked = match Box::pin(spoken(ctx, served, serve::CAPABILITY, serve::MAJOR)).await {
        Spoken::Over(adapter) => serve::Adapter(adapter),
        Spoken::Unanswered => {
            return Err(State::Skipped {
                reason: format!("{} cannot be asked over media.serve", served.name),
            })
        }
        Spoken::Not => {
            return Err(State::Skipped {
                reason: format!(
                    "{} speaks no media.serve contract, so nothing declares the paths it serves",
                    served.name
                ),
            })
        }
    };
    let declared = asked
        .declared()
        .await
        .map_err(|failure| crate::seed::unreached(&failure))?;
    declared.checked().map_err(|refused| State::Failed {
        detail: format!("the media server's declaration was refused: {refused}"),
    })
}

/// The envelope the guard enforces: `declared`, where it reaches the media server and the
/// adapter in front of it, the one proxy it believes and where it asks about a pass.
fn enveloped(fillers: &Fillers, served: &Filler, declared: Declared) -> Guarded {
    let upstream = fillers.fronted_by(served).unwrap_or(served);
    let address = |filler: &Filler| {
        filler
            .address
            .as_ref()
            .map(crate::wiring::Address::url)
            .unwrap_or_default()
    };
    Guarded {
        declared,
        upstream: address(upstream),
        adapter: served.fronts.is_some().then(|| address(served)),
        proxy: PROXY_ON_PROXY.to_string(),
        pass: format!("http://{}{PATH}", SocketAddr::from((PASS_ON_PASS, PORT))),
    }
}

/// Hand the guard `envelope` where it does not already enforce it.
async fn applied(ctx: &Ctx, guarding: &dyn Guarding, envelope: Guarded) -> State {
    let held = match guarding.guarding().await {
        Ok(held) => held,
        Err(failure) => return crate::seed::unreached(&failure),
    };
    if held.as_ref() == Some(&envelope) {
        return State::AlreadyWired;
    }
    if ctx.dry_run {
        return State::WouldWire {
            yours: held.map(|held| said(&held)),
            ours: Some(said(&envelope)),
        };
    }
    match crate::screening::guarded::guard(guarding, envelope).await {
        Ok(()) => State::Wired,
        Err(unguarded) => State::Failed {
            detail: unguarded.to_string(),
        },
    }
}

/// Write the pass service's file of live-stream paths beneath `project` as `declared`
/// gives them, where it holds others.
fn live(ctx: &Ctx, project: &Path, declared: &Declared) -> State {
    let wanted = Live::of(
        declared
            .live
            .iter()
            .map(|path| LivePath {
                pattern: path.pattern.clone(),
                stream: path.stream.clone(),
            })
            .collect(),
    )
    .written();
    let at = project
        .join(CONFIGURATION_TREE)
        .join(SERVICE)
        .join(File::Live.name());
    let held = crate::within::read_unlinked(ctx.seams.confined.as_ref(), &at, project)
        .ok()
        .flatten();
    if held.as_deref() == Some(wanted.as_str()) {
        return State::AlreadyWired;
    }
    if ctx.dry_run {
        return State::WouldWire {
            yours: held,
            ours: Some(wanted),
        };
    }
    match crate::config::store::write(&at, &wanted) {
        Ok(()) => State::Wired,
        Err(failure) => State::Failed {
            detail: failure.to_string(),
        },
    }
}

/// An envelope by what it declares and what it points the guard at.
fn said(guarded: &Guarded) -> String {
    format!(
        "{} byte, {} live-stream and {} refused paths; the media server at {}{}, the proxy at \
         {}, passes asked at {}",
        guarded.declared.bytes.len(),
        guarded.declared.live.len(),
        guarded.declared.refused.len(),
        guarded.upstream,
        guarded
            .adapter
            .as_deref()
            .map(|adapter| format!(" through {adapter}"))
            .unwrap_or_default(),
        guarded.proxy,
        guarded.pass
    )
}

/// The connection `name`, resting in `state`.
fn settled(name: &str, state: State) -> Wiring {
    Wiring::settled(name.to_owned(), state)
}
