//! Starting what an install placed, asking it what the manifest said it would answer,
//! and taking it back off the machine where it did not.
//!
//! **The proofs are the install's condition rather than a report on it.** A plugin
//! declares what must hold for it to be worth installing at all, and until now nothing
//! asked: `plugin claims` ran them against the recordings the author shipped, which is
//! what an author and a catalogue can do and is not a fact about anybody's machine.
//! This asks the service, on the machine it was just installed on, which is the only
//! place the question has an answer.
//!
//! **Asked by the evaluator everything else is asked by.** A live answer is read into
//! the shape a recorded one is read into, and [`crate::plugin::judging::judge`] decides
//! both — so a proof means the same thing whoever answered it, and a plugin cannot pass
//! against a recording under one rule and against its own service under another.
//!
//! **Nothing here is a second reversal.** The files an install wrote are put back by
//! the rollback layer, over the journal entries it already made. What this adds is the
//! one thing no journal entry describes: a container is not a file, nothing on disk
//! records that it is running, and a document removed out from under a running
//! container leaves something Compose will never be asked about again. So the container
//! comes off first and the rest is somebody else's machinery.

use std::path::Path;
use std::time::Duration;

use lemonfiber_plugin::{Manifest, Proof};

use crate::error::{Problem, Remedy, Severity, State};
use crate::plugin::judging::{judge, live, method};
use crate::plugin::{Evidence, Installed, Proving, Verdict};
use crate::ports::http::Request;
use crate::stack::closure::Plan;
use crate::stack::compose::{build, Action};

use super::super::putting_back::Reversal;
use super::super::Ctx;
use super::UNPROVED;

/// How often a service that has not answered yet is asked again.
///
/// The same interval the start's own wait polls the engine on, because it is the same
/// question wearing a different coat: a container Compose has created is not yet a
/// service that answers, and what an operator is waiting for is the second one.
const POLL: Duration = Duration::from_millis(500);

/// Start the plugin's own services, and nothing else of the stack.
///
/// A plan of its own rather than a form's: what has to come up is the profile this
/// plugin's container was written into and the services in it, and starting a form
/// would start whatever else that form carries. The settings are the machine's with
/// this plugin added, because the document that declares the service is layered off
/// the register and the register does not hold it yet — it is written last, once the
/// proofs have held.
///
/// # Errors
///
/// Where the engine could not be run, or ran and refused — and in either case the
/// install is put back first, so what the refusal says about the machine is what the
/// reversal left rather than what it was hoped to leave.
pub(crate) async fn started(
    ctx: &Ctx,
    installed: &Installed,
    stack: &Path,
    stamp: &str,
) -> Result<(), Box<Problem>> {
    let services: Vec<String> = installed
        .services
        .iter()
        .map(|placed| placed.service.clone())
        .collect();
    let command = invocation(ctx, installed, stack, &Action::Start(services));
    let answered = match ctx.seams.runner.run(&command).await {
        Ok(output) if output.succeeded() => return Ok(()),
        other => other,
    };
    // Put back before the sentence about it is written. A failure here is the same
    // failure a proof that did not hold is — the install got no further — so it goes
    // back the same way, through the one reversal rather than a second of its own.
    let back = super::reversing(ctx, installed, stack, stamp).await;
    let problem = unstarted(&installed.plugin, &back);
    Err(Box::new(match answered {
        Ok(refused) => problem.with_detail(refused.stderr.trim().to_owned()),
        // The engine's own account underneath the install's, rather than in place of
        // it: what an operator has to act on is that nothing is installed, and *docker
        // is not on this machine* is why rather than what.
        Err(why) => problem.caused_by(crate::error::Diagnose::problem(&why)),
    }))
}

/// Bring the plugin's containers up, answering with why not where they did not come.
///
/// For an update rather than an install. [`started`] puts the install back when the
/// engine refuses, which is right for an install and wrong for an update: what an
/// update starts is either the new version — whose reversal the update carries out
/// itself, beside putting the old one back — or the old version being put back, where
/// the only thing worse than it not starting is a reversal that took its files away
/// again for not starting.
pub(crate) async fn up(ctx: &Ctx, installed: &Installed, stack: &Path) -> Option<String> {
    let services: Vec<String> = installed
        .services
        .iter()
        .map(|placed| placed.service.clone())
        .collect();
    let command = invocation(ctx, installed, stack, &Action::Start(services));
    match ctx.seams.runner.run(&command).await {
        Ok(output) if output.succeeded() => None,
        Ok(refused) => Some(format!(
            "the container engine refused to start it: {}",
            refused.stderr.trim()
        )),
        Err(why) => Some(why.to_string()),
    }
}

/// Take the plugin's containers back off the machine.
///
/// Answers whether it could, rather than failing: this runs inside an install that is
/// already being put back, and a reversal that stopped at its first difficulty would
/// leave more behind than one that carried on and said what it could not do.
pub(crate) async fn removed(ctx: &Ctx, installed: &Installed, stack: &Path) -> bool {
    let services: Vec<String> = installed
        .services
        .iter()
        .map(|placed| placed.service.clone())
        .collect();
    let command = invocation(ctx, installed, stack, &Action::Remove(services));
    matches!(ctx.seams.runner.run(&command).await, Ok(output) if output.succeeded())
}

/// The Compose invocation for this plugin's own services.
///
/// One builder for the starting and the taking back, because the two differ in the
/// verb and in nothing else — and an invocation built twice is an invocation that can
/// name a different project on the way out than it did on the way in.
fn invocation(ctx: &Ctx, installed: &Installed, stack: &Path, action: &Action) -> Vec<String> {
    let mut settings = ctx.settings.clone();
    settings.plugins.push(installed.plugin.clone());
    let plan = Plan {
        forms: Vec::new(),
        profiles: std::iter::once(crate::plugin::profile(&installed.plugin)).collect(),
        services: installed
            .services
            .iter()
            .map(|placed| placed.service.clone())
            .collect(),
        dropped: Vec::new(),
        filtered: Vec::new(),
        footprint: crate::stack::closure::Footprint::default(),
    };
    build(&plan, &settings, stack, action, ctx.environment)
}

/// The stack's proxy, as its Compose service and the profile that runs it.
const PROXY: (&str, &str) = ("caddy", "proxy");

/// Whether these writes put a route into the proxy's file.
pub(crate) fn routes_written(planned: &[crate::plugin::Write]) -> bool {
    planned.iter().any(|write| {
        matches!(&write.lands, crate::plugin::Lands::Region { key, .. } if key == crate::plugin::PROXY)
    })
}

/// Whether this reversal took a route back out of the proxy's file.
pub(crate) fn routes_withdrawn(back: &Reversal) -> bool {
    back.reversed.iter().any(|undo| {
        matches!(&undo.action, crate::journal::Action::Withdraw { key, .. } if key == crate::plugin::PROXY)
    })
}

/// Have the stack's proxy read its configuration again, where a route was just written
/// into it or taken out of it.
///
/// The proxy reads its file once, when it starts, so a route written while it runs is
/// a route nothing serves until it is restarted. A restart and not a start: a proxy the
/// operator does not run is not started by a plugin, and restarting what is not running
/// is nothing. Best effort, and said nowhere, because the file is already right. What
/// a restart that did not happen costs is a route that is served from the next time
/// the proxy starts, which is the same thing a bundled stanza gets.
///
/// Told whether a route changed rather than finding out, because the writes and the
/// reversal already say so, and a second look at the disk would be a second answer.
pub(crate) async fn refronted(ctx: &Ctx, stack: &Path, routed: bool) {
    if !routed {
        return;
    }
    let (service, profile) = PROXY;
    let plan = Plan {
        forms: Vec::new(),
        profiles: std::iter::once(profile.to_owned()).collect(),
        services: vec![service.to_owned()],
        dropped: Vec::new(),
        filtered: Vec::new(),
        footprint: crate::stack::closure::Footprint::default(),
    };
    let restart = Action::Restart(vec![service.to_owned()]);
    let command = build(&plan, &ctx.settings, stack, &restart, ctx.environment);
    let _ = ctx.seams.runner.run(&command).await;
}

/// Ask every proof of the service it names, now that there is one to ask.
///
/// The stated list and the manifest's own proofs are one list read twice — the first
/// is built one entry per declared proof, in the manifest's order — so they walk
/// together rather than being matched up by a name either could be missing.
///
/// One budget for the whole phase rather than one per proof. What an operator is
/// waiting out is a service that has not come up, and a plugin with five proofs
/// against a service that never answers must not wait five times as long as one with
/// a single proof.
pub(crate) async fn asked(
    ctx: &Ctx,
    manifest: &Manifest,
    installed: &Installed,
    stated: &mut [Proving],
) {
    let deadline = ctx.seams.clock.now() + ctx.patience;
    // Where each of this plugin's services answers, read through the one answer every
    // caller that asks a plugin's service reads: a second way of composing an address
    // is a second port to be wrong about.
    let reached = crate::plugin::answering(std::slice::from_ref(installed));
    for (proof, stated) in manifest.proofs.iter().zip(stated.iter_mut()) {
        let at = stated.of.as_deref().and_then(|named| reached.get(named));
        stated.came_to = Some(match at {
            None => Verdict::Unproven {
                why: format!(
                    "{} publishes no port this machine can reach, so there is nowhere to ask it",
                    stated.of.as_deref().unwrap_or("the service it names")
                ),
            },
            Some(address) => answering(ctx, proof, address, deadline).await,
        });
    }
}

/// What asking one proof of one service came to.
///
/// A service that has not answered *yet* is asked again until the budget runs out,
/// because a container Compose has just created is not a service that is listening —
/// the gap is seconds and an install that took the first refusal would fail on every
/// image that takes a moment to open its socket. A service that answers something is
/// never asked twice: what it said is the evidence, and asking again until it says
/// something else is not proving, it is waiting for luck.
pub(super) async fn answering(
    ctx: &Ctx,
    proof: &Proof,
    address: &str,
    deadline: std::time::SystemTime,
) -> Verdict {
    // The method the transport can carry, or nothing where the manifest named one it
    // cannot. Reported as unproven rather than sent as something else: a proof asked
    // with a verb nobody wrote down is a proof about a question nobody declared.
    let Some(method) = method(&proof.request.method) else {
        return Verdict::Unproven {
            why: format!(
                "{} is not a method lemonfiber can send, so the proof was never put",
                proof.request.method
            ),
        };
    };
    // A route on the service lemonfiber resolved, and nothing that could name another
    // host once it is joined to that address. The reader refused any other path when
    // the manifest was read; this is where the join happens, so it is asked again here.
    if !lemonfiber_plugin::refusing::carried::is_route(&proof.request.path) {
        return Verdict::Unproven {
            why: format!(
                "{} is not a route on {address}, so the proof was never put",
                proof.request.path
            ),
        };
    }
    // No headers. A proof cannot present a credential, because there is nowhere in the
    // block to write one — which is what keeps *this asks as nobody* a property of the
    // format rather than a convention somebody has to hold.
    let request = Request {
        method,
        url: format!("{address}{}", proof.request.path),
        headers: Vec::new(),
        body: None,
    };
    loop {
        match ctx.seams.http.send(&request).await {
            Ok(response) => {
                let faults = judge(&proof.expect, &live(&response));
                return if faults.is_empty() {
                    Verdict::Passed
                } else {
                    Verdict::Failed { faults }
                };
            }
            // Checked after the asking rather than before it, so a budget of nothing
            // still reports what the service said rather than reporting that it was
            // never asked.
            Err(unreachable) if ctx.seams.clock.now() >= deadline => {
                return Verdict::Unproven {
                    why: format!("{} did not answer: {}", unreachable.url, unreachable.reason),
                }
            }
            Err(_) => tokio::time::sleep(POLL).await,
        }
    }
}

/// Whether every proof held.
///
/// **Unproven does not pass here, and that is where this parts company with the
/// author's own read.** Against the recordings a plugin ships, unproven means the
/// evidence is missing and the author is the one who can add it, so it leaves the
/// plugin installable. Against the service, unproven means the service did not answer
/// the thing the plugin said it would — which is exactly what the proof exists to
/// establish, and installing over it would be reporting an install as complete on the
/// strength of a question nobody got an answer to.
///
/// Reported as unproven either way. What it is called and what it costs are two
/// decisions, and a verdict renamed to justify the cost would tell an operator their
/// service is broken when what happened is that nothing answered.
pub(crate) fn held(proofs: &[Proving]) -> bool {
    proofs
        .iter()
        .all(|one| matches!(one.came_to, Some(Verdict::Passed)))
}

/// What the verdicts on this run were reached against.
///
/// `Service` and never `Recordings`, because nothing here reads a recording. It is a
/// field rather than a sentence for the reason the weaker value is one: a consumer
/// handed a verdict has nothing else in the document to tell a recording that answered
/// from a service that did.
pub(crate) const AGAINST: Evidence = Evidence::Service;

/// The plugin's own container would not start.
fn unstarted(plugin: &str, back: &Reversal) -> Problem {
    Problem::new(
        UNPROVED,
        Severity::Error,
        format!("{plugin}'s service would not start, so nothing about it could be proved"),
        super::left_behind(back),
        Remedy::new("Check that the stack is set up and the container engine is running"),
    )
    .in_state(State::Guided)
}
