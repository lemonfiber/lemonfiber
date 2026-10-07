//! Running a recipe: each step in order, once, held to what reading it allowed.
//!
//! **A step is made at most once, in the order written,** and a guard can skip it. A
//! step with a retry is made again, a bounded number of times and a bounded wait apart,
//! until its answer is the one it waits for; a step without one is made once. The first
//! step that does not come to what it should ends the recipe, and the steps after it are
//! said not to have been reached.
//!
//! **Where a call goes is where the manifest says.** A service in the stack is reached
//! on this machine at the port it publishes. A host outside it is resolved for that call
//! and the call held to the addresses that passed ([`crate::outward::checked`]), so a
//! name standing for somewhere on this machine or its network is refused before
//! anything is sent, and a name that moves after the check is not followed.
//!
//! **What landed is said.** A call that reached anything but this plugin's own services
//! changed something an install going back cannot reach into, so each such call is
//! marked as having landed, which is what a failure has to name.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use lemonfiber_plugin::{Recipe, Retry, Step, LARGEST_ANSWER};
use serde::{Deserialize, Serialize};

pub use crate::error::Came;

use crate::outward::{checked, Inward};
use crate::ports::http::{Http, Request, Response, Unreachable};
use crate::ports::resolve::Resolver;

mod bounding;
mod calling;
mod reading;

use bounding::Bounds;
use calling::{Unbuilt, Whither};

/// How the services a recipe can call are reached from this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reaching {
    /// The port each service in the stack publishes on this machine, by its id: the
    /// stack's own and this plugin's.
    pub ports: BTreeMap<String, u16>,
    /// This plugin's own services, which a call to does not land anywhere an install
    /// going back cannot reach.
    pub own: BTreeSet<String>,
}

/// What a run is made with.
pub struct Running<'a> {
    /// The transport every call is sent over.
    pub http: &'a dyn Http,
    /// What a host outside the stack is resolved with, for each call.
    pub resolver: &'a dyn Resolver,
    /// How the stack's services are reached.
    pub reaching: &'a Reaching,
    /// Every pair to a host outside the stack this act approved, as
    /// `<value>@<destination>`.
    pub approved: &'a [String],
}

/// What one recipe came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginRecipeRan")]
pub struct Ran {
    /// The recipe's id.
    pub recipe: String,
    /// Whether every step it made came to what it should.
    pub held: bool,
    /// Why it did not, where it did not.
    pub why: Option<String>,
    /// Every step it declares, in order, with what each came to.
    pub steps: Vec<StepRan>,
}

/// What one step came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginStepRan")]
pub struct StepRan {
    /// The step's id.
    pub step: String,
    /// Where it calls, by the name the manifest gives it.
    pub to: String,
    /// What it came to.
    pub came: Came,
    /// The status its last answer carried, where anything answered.
    pub status: Option<u16>,
    /// How many times it was made.
    pub tries: u32,
    /// Whether it reached somewhere other than this plugin's own services, which an
    /// install going back cannot undo.
    pub landed: bool,
    /// Why it came to what it did, where that wants saying.
    pub why: Option<String>,
}

/// A recipe's run, and every value it captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// What it came to.
    pub ran: Ran,
    /// What its steps captured, by name.
    pub captured: BTreeMap<String, String>,
}

/// Run one recipe with these inputs.
pub async fn run(
    running: &Running<'_>,
    recipe: &Recipe,
    inputs: &BTreeMap<String, String>,
) -> Outcome {
    let mut values = inputs.clone();
    let mut bounds = Bounds::of(recipe, running.approved);
    let mut captured = BTreeMap::new();
    let mut answered: BTreeMap<String, u16> = BTreeMap::new();
    let mut steps = Vec::new();
    let mut why = None;
    for step in &recipe.steps {
        if why.is_some() {
            steps.push(said(step, Came::NotReached, None));
            continue;
        }
        if let Some(guard) = &step.when {
            if !reading::holds(guard, &answered, &values) {
                steps.push(said(step, Came::Skipped, None));
                continue;
            }
        }
        let made = made(running, step, &values, &bounds).await;
        if made.ran.came == Came::Answered {
            answered.extend(made.ran.status.map(|status| (step.id.clone(), status)));
            bounds.traded(&made.carried, made.captured.keys());
            values.extend(made.captured.clone());
            captured.extend(made.captured);
        } else {
            why = Some(format!(
                "step {} {}",
                step.id,
                made.ran.why.as_deref().unwrap_or_default()
            ));
        }
        steps.push(made.ran);
    }
    Outcome {
        ran: Ran {
            recipe: recipe.id.clone(),
            held: why.is_none(),
            why,
            steps,
        },
        captured,
    }
}

/// What one step made came to, and what it captured.
struct Made {
    /// What it came to.
    ran: StepRan,
    /// What it captured, where it came to what it should.
    captured: BTreeMap<String, String>,
    /// The name of every value its call carried, where it was made.
    carried: BTreeSet<String>,
}

/// A step that was not made, or made and said in one line.
fn said(step: &Step, came: Came, why: Option<String>) -> StepRan {
    StepRan {
        step: step.id.clone(),
        to: step.call.to.clone(),
        came,
        status: None,
        tries: 0,
        landed: false,
        why,
    }
}

/// Make one step: build its call, hold it and every value it carries to where they may
/// go, and send it until it comes to an answer or runs out of tries.
async fn made(
    running: &Running<'_>,
    step: &Step,
    values: &BTreeMap<String, String>,
    bounds: &Bounds<'_>,
) -> Made {
    match prepared(running, step, values, bounds).await {
        Ok((request, carried)) => {
            let elsewhere = !running.reaching.own.contains(&step.call.to);
            Made {
                carried,
                ..tried(running.http, step, &request, values, elsewhere).await
            }
        }
        Err(unmade) => unmade,
    }
}

/// A step's call as it is handed to the transport, and every value it carries; or the
/// step said as not made, and why.
async fn prepared(
    running: &Running<'_>,
    step: &Step,
    values: &BTreeMap<String, String>,
    bounds: &Bounds<'_>,
) -> Result<(Request, BTreeSet<String>), Made> {
    let whither = running
        .reaching
        .ports
        .get(&step.call.to)
        .map_or(Whither::Outside, |port| Whither::Stack(*port));
    let unmade = |came: Came, why: String| Made {
        ran: said(step, came, Some(why)),
        captured: BTreeMap::new(),
        carried: BTreeSet::new(),
    };
    let built =
        calling::request(&step.call, whither, values, bounds).map_err(|unbuilt| match unbuilt {
            Unbuilt::Withheld(why) => unmade(Came::Withheld, format!("was not sent: it {why}")),
            Unbuilt::Unaddressed(why) => unmade(Came::Refused, format!("was not sent: {why}")),
            Unbuilt::Method => unmade(
                Came::Refused,
                format!(
                    "calls with {}, which lemonfiber does not send",
                    step.call.method
                ),
            ),
        })?;
    let refused = |why: String| unmade(Came::Refused, why);
    let request = match whither {
        Whither::Stack(_) => built.request,
        Whither::Outside => checked(running.resolver, built.request)
            .await
            .map_err(|inward| inward_said(&step.call.to, &inward))
            .map_err(refused)?,
    };
    let request = calling::sending(request, whither, &step.call.to).map_err(refused)?;
    Ok((request, built.carried))
}

/// Why a host outside the stack was not called, as a step says it.
fn inward_said(to: &str, inward: &Inward) -> String {
    match inward {
        Inward::Internal(address) => {
            format!("was not sent: {to} stands for {address}, which is not out on the internet")
        }
        Inward::Unresolved(why) => format!("was not sent: {to} could not be resolved: {why}"),
        Inward::Nowhere | Inward::Nameless => format!("was not sent: {to} stands for no address"),
    }
}

/// What one try came to.
enum Read {
    /// Nothing answered, and whether a connection was made before it failed.
    Unreachable(Unreachable),
    /// It answered more than a recipe reads.
    Oversized(u16),
    /// It answered, with what each capture took or why one could not.
    Answered(
        crate::plugin::Answer,
        Result<BTreeMap<String, String>, String>,
    ),
}

/// One try's answer, read the way a step reads it.
fn read(step: &Step, sent: Result<Response, Unreachable>) -> Read {
    match sent {
        Err(unreachable) => Read::Unreachable(unreachable),
        Ok(response) if response.body.len() > LARGEST_ANSWER => Read::Oversized(response.status),
        Ok(response) => {
            let answer = crate::plugin::judging::live(&response);
            let captured = reading::captured(&step.capture, &answer);
            Read::Answered(answer, captured)
        }
    }
}

/// Send one step's call, again where it retries, until it comes to the answer it waits
/// for or runs out of tries.
async fn tried(
    http: &dyn Http,
    step: &Step,
    request: &Request,
    values: &BTreeMap<String, String>,
    elsewhere: bool,
) -> Made {
    let allowed = 1 + step.retry.as_ref().map_or(0, |retry| retry.times);
    let wait = step
        .retry
        .as_ref()
        .and_then(Retry::seconds)
        .unwrap_or_default();
    let mut tries = 0;
    let mut reached = false;
    loop {
        tries += 1;
        let this = read(step, http.send(request).await);
        reached |= match &this {
            Read::Unreachable(unreachable) => unreachable.connected,
            Read::Oversized(_) | Read::Answered(..) => true,
        };
        let waited = step
            .retry
            .as_ref()
            .is_some_and(|retry| ended(retry, &this, values));
        if tries >= allowed || waited {
            return concluded(step, this, tries, reached && elsewhere);
        }
        tokio::time::sleep(Duration::from_secs(wait)).await;
    }
}

/// Whether a try's answer is the one a retry waits for: the answer its `until` ends on,
/// or one larger than a recipe reads, which no later try is waited for after.
fn ended(retry: &Retry, this: &Read, values: &BTreeMap<String, String>) -> bool {
    match this {
        Read::Unreachable(_) => false,
        Read::Oversized(_) => true,
        Read::Answered(answer, captured) => {
            let mut seen = values.clone();
            seen.extend(captured.clone().unwrap_or_default());
            reading::ended(&retry.until, answer.status, &seen)
        }
    }
}

/// What a step's last try comes to.
fn concluded(step: &Step, last: Read, tries: u32, landed: bool) -> Made {
    let ran = |came: Came, status: Option<u16>, why: Option<String>| StepRan {
        step: step.id.clone(),
        to: step.call.to.clone(),
        came,
        status,
        tries,
        landed,
        why,
    };
    let (ran, captured) = match last {
        Read::Unreachable(unreachable) => (
            ran(
                Came::Unreachable,
                None,
                Some(format!("was not answered: {}", unreachable.reason)),
            ),
            BTreeMap::new(),
        ),
        Read::Oversized(status) => (
            ran(
                Came::Oversized,
                Some(status),
                Some(format!(
                    "was answered with more than {LARGEST_ANSWER} bytes"
                )),
            ),
            BTreeMap::new(),
        ),
        Read::Answered(answer, captured) => {
            let faults = step
                .expect
                .as_ref()
                .map(|expect| crate::plugin::judging::judge(expect, &answer))
                .unwrap_or_default();
            match (faults.is_empty(), captured) {
                (false, _) => (
                    ran(
                        Came::Unexpected,
                        Some(answer.status),
                        Some(faults.join("; ")),
                    ),
                    BTreeMap::new(),
                ),
                (true, Err(why)) => (
                    ran(Came::Uncaptured, Some(answer.status), Some(why)),
                    BTreeMap::new(),
                ),
                (true, Ok(captured)) => (ran(Came::Answered, Some(answer.status), None), captured),
            }
        }
    };
    Made {
        ran,
        captured,
        carried: BTreeSet::new(),
    }
}

#[cfg(test)]
mod tests;
