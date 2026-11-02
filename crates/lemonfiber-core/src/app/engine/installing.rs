//! The parts the bundle installs that a form holds and this machine has never run: each
//! offered to `up` and `switch` as an install from the tree this build carries, offline,
//! through the plugin path any install takes.
//!
//! The offer is the yes. A rehearsal names it, a start answering it installs every part
//! it names before anything of the form is written or started, and a start answering
//! nothing, or an offer that has moved, writes and starts nothing at all.

use lemonfiber_bundled::{Pinned, PINNED};
use schemars::JsonSchema;
use serde::Serialize;

use crate::agreement::over;
use crate::app::plugins::moving_in::{uninstalled, Ran};
use crate::app::plugins::Consent;
use crate::app::Ctx;
use crate::error::codes::life::{
    OFFLINE_INSTALL_MOVED, OFFLINE_INSTALL_UNANSWERED, OFFLINE_INSTALL_UNHELD,
};
use crate::error::{Problem, Remedy, State};
use crate::model::LifecycleReport;
use crate::stack::closure::Plan;

/// Every refusal a start answering what it offers to install may meet, beside the offer
/// that moved.
pub const REFUSALS: [crate::error::Code; 2] = [OFFLINE_INSTALL_UNANSWERED, OFFLINE_INSTALL_UNHELD];

/// An install of a part the bundle installs that a start offers, by the plugin and the
/// offer its install reads as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[schemars(rename = "UpInstallOffered")]
pub struct InstallOffered {
    /// The plugin the bundle installs, in place of the stack's own service of its id.
    pub plugin: String,
    /// The name its install's offer goes by, where it could be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offer: Option<String>,
    /// Why its install could not be read, where it could not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// Every install a start offers over a form, and the one offer that answers them all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offline {
    /// Each install on offer.
    pub installs: Vec<InstallOffered>,
    /// The offer the start answers: each plugin, by its install's offer.
    pub offer: String,
}

/// Every part the bundle installs that `plan` holds, whose plugin is not installed and
/// whose stack service has never run on this machine: nothing where the record of what is
/// installed cannot be read, since an install offered over one already there is one
/// nothing can take, nor where lemonfiber keeps no files of its own here, since nothing
/// is set up for an install to be made into.
async fn unrun(ctx: &Ctx, plan: &Plan) -> Vec<&'static Pinned> {
    if crate::app::targets::layout(ctx).is_none() {
        return Vec::new();
    }
    let Ok(register) = crate::app::plugins::read(ctx) else {
        return Vec::new();
    };
    let ran = Ran::here(ctx).await;
    uninstalled(PINNED, register.installed())
        .into_iter()
        .filter(|one| {
            plan.services
                .iter()
                .any(|service| service == one.first_party.plugin)
        })
        .filter(|one| !ran.has_run(one.first_party.plugin))
        .collect()
}

/// What a start offers to install over `plan`, each install read through the plugin path:
/// nothing where every part the form holds is installed or has run here.
pub(crate) async fn offered(ctx: &Ctx, plan: &Plan) -> Option<Offline> {
    let mut installs = Vec::new();
    for pinned in unrun(ctx, plan).await {
        let plugin = pinned.first_party.plugin;
        let read = crate::app::plugins::from_this_build(ctx, plugin, Consent::default()).await;
        installs.push(InstallOffered {
            plugin: plugin.to_owned(),
            offer: read
                .as_ref()
                .ok()
                .map(|installs| installs.agreement.clone().unwrap_or_default()),
            why: read.err().map(|problem| problem.summary),
        });
    }
    if installs.is_empty() {
        return None;
    }
    let words: Vec<&str> = installs
        .iter()
        .flat_map(|one| {
            [
                one.plugin.as_str(),
                one.offer.as_deref().unwrap_or_default(),
            ]
        })
        .collect();
    let offer = over(&words);
    Some(Offline { installs, offer })
}

/// What a start offers to install over `forms`, read through the plugin path: nothing where
/// every part they hold is installed or has run here.
///
/// # Errors
///
/// Where the forms cannot be resolved.
pub async fn offline(ctx: &Ctx, forms: &[String]) -> Result<Option<Offline>, Box<Problem>> {
    let (_, plan) = super::resolved(ctx, forms)?;
    Ok(offered(ctx, &plan).await)
}

/// What a start offers to install over `forms`, where they resolve: a start over forms
/// that do not is refused by the pre-flight it shares with every lifecycle command, in
/// that pre-flight's order, rather than here.
pub(crate) async fn pending(ctx: &Ctx, forms: &[String]) -> Option<Offline> {
    offline(ctx, forms).await.ok().flatten()
}

/// Answer the installs `offline` offers with `answered`: every install taken where it
/// names the offer standing now, refused where it names another, and still on offer,
/// handed back, where it names none.
///
/// # Errors
///
/// Where `answered` names an offer that has moved, where an install refuses, and where one
/// does not hold.
pub(crate) async fn answer(
    ctx: &Ctx,
    offline: Offline,
    answered: Option<&str>,
) -> Result<Option<Offline>, Box<Problem>> {
    let Some(answered) = answered else {
        return Ok(Some(offline));
    };
    if answered != offline.offer {
        return Err(Box::new(moved(answered, &offline)));
    }
    for one in &offline.installs {
        let consent = Consent {
            agreement: one.offer.clone(),
            ..Consent::default()
        };
        let installs = crate::app::plugins::from_this_build(ctx, &one.plugin, consent).await?;
        if !installs.install.is_some_and(|install| install.recorded) {
            return Err(Box::new(unheld(&one.plugin)));
        }
        ctx.narrator
            .say(&format!(
                "installed {} from this build, offline",
                one.plugin
            ))
            .await;
    }
    Ok(None)
}

/// What a start at boot starts while installs wait: every service the forms hold but each
/// part awaiting its install and every service placed inside one's network, which Compose
/// would otherwise start with it. Nothing where no install waits.
///
/// Boot is no `up`: nobody is there to answer, and a machine left with nothing running
/// after a restart costs the household everything, so the rest of the forms start and
/// the waiting install is left for the operator to answer.
///
/// # Errors
///
/// Where the forms or the stack's topology cannot be resolved.
pub(crate) async fn without_waiting(
    ctx: &Ctx,
    forms: &[String],
) -> Result<Option<(Vec<String>, Offline)>, Box<Problem>> {
    let (manifest, plan) = super::resolved(ctx, forms)?;
    let Some(offline) = Box::pin(offered(ctx, &plan)).await else {
        return Ok(None);
    };
    let topology = crate::app::project::topology(ctx, &manifest)?;
    let left = left_out(
        offline
            .installs
            .iter()
            .map(|one| one.plugin.clone())
            .collect(),
        &topology.placed,
    );
    let started = plan
        .services
        .into_iter()
        .filter(|service| !left.contains(service))
        .collect();
    Ok(Some((started, offline)))
}

/// `waiting`, and every service `placed` puts inside the network of one of them or of a
/// service already left out, which Compose would start along with it.
#[must_use]
pub(crate) fn left_out(
    mut waiting: Vec<String>,
    placed: &std::collections::BTreeMap<String, crate::topology::Placed>,
) -> Vec<String> {
    loop {
        let inside: Vec<String> = placed
            .iter()
            .filter_map(|(service, placed)| match placed {
                crate::topology::Placed::Inside(host) if waiting.contains(host) => {
                    Some(service.clone())
                }
                _ => None,
            })
            .filter(|service| !waiting.contains(service))
            .collect();
        if inside.is_empty() {
            return waiting;
        }
        waiting.extend(inside);
    }
}

/// The report of a start that wrote and started nothing, because it answered none of the
/// installs `offline` offers over `plan`.
#[must_use]
pub fn unstarted(action: &str, plan: Plan, offline: Offline) -> LifecycleReport {
    LifecycleReport {
        action: action.to_owned(),
        plan,
        command: Vec::new(),
        rehearsed: false,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits: Vec::new(),
        port_conflicts: Vec::new(),
        forwarding: None,
        secured: None,
        switched: None,
        held: None,
        offer: Some(offline.offer),
        unvetted: None,
        moves: Vec::new(),
        installing: offline.installs,
        guards: Vec::new(),
        swept: Vec::new(),
        passing: None,
        fencing: None,
        unfenced: Vec::new(),
    }
}

/// A start that would install a part and was given no yes to it.
#[must_use]
pub(crate) fn unanswered(offline: &Offline) -> Problem {
    Problem::new(
        OFFLINE_INSTALL_UNANSWERED,
        format!(
            "Starting this installs {} from this build, and nobody agreed to it",
            plugins(offline)
        ),
        "Nothing of the form was written or started.",
        Remedy::new("Rehearse the start, and answer the install it offers"),
    )
    .in_state(State::Guided)
}

/// A start answering an offer that is not the one standing now.
fn moved(answered: &str, offline: &Offline) -> Problem {
    Problem::new(
        OFFLINE_INSTALL_MOVED,
        "That agreement was given for a different install",
        format!(
            "It answered {answered}, and a start now would install {}, which is offered as {}. \
             Nothing was installed, written or started.",
            plugins(offline),
            offline.offer
        ),
        Remedy::new("Rehearse the start again, and answer the offer it gives now"),
    )
    .in_state(State::Guided)
}

/// An install agreed to that did not hold.
fn unheld(plugin: &str) -> Problem {
    Problem::new(
        OFFLINE_INSTALL_UNHELD,
        format!("The install of {plugin} from this build did not hold"),
        "It was put back, and nothing of the form was started.",
        Remedy::new(format!(
            "Run lemonfiber plugin install {plugin} to see what its proofs found, then start \
             the form again"
        )),
    )
    .in_state(State::Guided)
}

/// The plugins `offline` offers, as a list a sentence reads.
#[must_use]
pub(crate) fn plugins(offline: &Offline) -> String {
    offline
        .installs
        .iter()
        .map(|one| one.plugin.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
