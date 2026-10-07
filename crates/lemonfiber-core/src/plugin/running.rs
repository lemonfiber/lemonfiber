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

use lemonfiber_plugin::vocabulary::Constraint;
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
    /// Every credential lemonfiber holds for a service of the stack's, as the service's
    /// id and the value, which a value holding one is held to however a recipe came by
    /// it.
    pub credentials: &'a [(String, String)],
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
    let mut bounds = Bounds::of(recipe, running.approved, running.credentials);
    let mut captured = BTreeMap::new();
    let mut answered: BTreeMap<String, u16> = BTreeMap::new();
    let mut steps = Vec::new();
    let mut why = None;
    for step in &recipe.steps {
        if why.is_some() {
            steps.push(said(step, Came::NotReached, None));
            continue;
        }
        // Asked before the guard is: a step left out because its guard read a credential
        // would say what the credential holds by what the recipe went on to do.
        let made = if let Some(withheld) = undecided(step, &values, &bounds) {
            withheld
        } else {
            if let Some(guard) = &step.when {
                if !reading::holds(guard, &answered, &values) {
                    steps.push(said(step, Came::Skipped, None));
                    continue;
                }
            }
            made(running, step, &values, &bounds).await
        };
        if made.ran.came == Came::Answered {
            answered.extend(made.ran.status.map(|status| (step.id.clone(), status)));
            let by = running
                .reaching
                .ports
                .contains_key(&step.call.to)
                .then_some(step.call.to.as_str());
            bounds.captured(&made.carried, by, made.captured.keys());
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
    /// Every value its call carried or its guard read, by name, where it was made.
    carried: BTreeMap<String, String>,
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
                ..tried(running.http, step, &request, values, bounds, elsewhere).await
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
) -> Result<(Request, BTreeMap<String, String>), Made> {
    let whither = running
        .reaching
        .ports
        .get(&step.call.to)
        .map_or(Whither::Outside, |port| Whither::Stack(*port));
    let unmade = |came: Came, why: String| Made {
        ran: said(step, came, Some(why)),
        captured: BTreeMap::new(),
        carried: BTreeMap::new(),
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
    let mut carried = built.carried;
    carried.extend(guarding(step, values));
    Ok((request, carried))
}

/// Every value this step's guard and its retry's end read, by name, where the run
/// holds it.
fn guarding(step: &Step, values: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    [
        step.when.as_ref(),
        step.retry.as_ref().map(|retry| &retry.until),
    ]
    .into_iter()
    .flatten()
    .filter_map(|condition| condition.value.as_deref())
    .filter_map(|name| values.get_key_value(name))
    .map(|(name, value)| (name.clone(), value.clone()))
    .collect()
}

/// The step said as withheld where its guard or its retry's end reads a value held to
/// a service other than the one it calls, or nothing where it may be decided on.
fn undecided(step: &Step, values: &BTreeMap<String, String>, bounds: &Bounds<'_>) -> Option<Made> {
    let why = guarding(step, values)
        .iter()
        .find_map(|(name, value)| bounds.decided(name, value, &step.call.to))?;
    Some(Made {
        ran: said(
            step,
            Came::Withheld,
            Some(format!("was not sent: it {why}")),
        ),
        captured: BTreeMap::new(),
        carried: BTreeMap::new(),
    })
}

/// One way an answer was not the one a step expects, said as the constraint and the
/// place, never as what the answer held there: an answer may carry a credential, and
/// what a step says reaches the problem an act ends with.
///
/// A status is said as it is, since a status is a number the protocol defines.
fn unheld(fault: &crate::plugin::judging::Fault) -> String {
    match (fault.constraint, &fault.place) {
        (Constraint::Status, _) => fault.said.clone(),
        (constraint, Some(place)) => {
            format!("{place} is not what {} declares", constraint.as_str())
        }
        (constraint, None) => format!("the answer is not what {} declares", constraint.as_str()),
    }
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
    bounds: &Bounds<'_>,
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
        if let Some(why) = unwaited(step, &this, bounds) {
            return Made {
                ran: said(
                    step,
                    Came::Withheld,
                    Some(format!("stopped waiting: it {why}")),
                ),
                captured: BTreeMap::new(),
                carried: BTreeMap::new(),
            };
        }
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

/// Why a retry's end may not be decided on, where it reads a value this try captured
/// that is held to a service other than the one the step calls.
///
/// The guard check before the step reads only what the run held then; a retry's end
/// can also read what each try captures, so the same check is made again on that.
fn unwaited(step: &Step, this: &Read, bounds: &Bounds<'_>) -> Option<String> {
    let name = step.retry.as_ref()?.until.value.as_deref()?;
    let Read::Answered(_, Ok(captured)) = this else {
        return None;
    };
    let value = captured.get(name)?;
    bounds.decided(name, value, &step.call.to)
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
                .map(|expect| crate::plugin::judging::faults(expect, &answer))
                .unwrap_or_default()
                .iter()
                .map(unheld)
                .collect::<Vec<_>>();
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
        carried: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests;
