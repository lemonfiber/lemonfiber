//! The one address to hand somebody who lives here.
//!
//! [`crate::door`] decides which service that is from what the stack declares; this
//! asks the engine what became of it and assembles the answer every surface shows.
//! The reading of what is running is the same survey the status view is built from,
//! so the two cannot grade one service differently.

use crate::app::Ctx;
use crate::door::{address, facing, Address, Chosen, Facing, Refusal};
use crate::error::{Diagnose, Problem};
use crate::model::{Beside, FrontDoorReport, Standing};
use crate::platform::Environment;

/// What there is to hand somebody who lives here, and where it stands.
pub(crate) async fn front_door(ctx: &Ctx) -> Result<FrontDoorReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let containers = ctx
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    let profiles: Vec<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    let running = crate::docker::survey(&manifest, &profiles, &containers, ctx.settings.protocols);
    // Asked now rather than remembered: a machine renamed since the last look
    // answers as it is, which is the whole of how a changed address is noticed.
    let named = ctx.site.name().await;
    Ok(assembled(
        &manifest.services,
        &running,
        named.as_deref(),
        ctx.settings.household_host.as_deref(),
        ctx.settings.front_door.as_deref(),
        ctx.environment,
    ))
}

/// The answer itself, over what the stack declares and what became of it.
///
/// Apart from the reading above so that the stacks worth asking about — one that
/// publishes nothing to the household at all — can be put to it, which no stack this
/// repository carries is.
pub(crate) fn assembled(
    declared: &[lemonfiber_manifest::Service],
    running: &[crate::docker::Service],
    named: Option<&str>,
    recorded: Option<&str>,
    chose: Option<&str>,
    environment: Environment,
) -> FrontDoorReport {
    let (chosen, door) = crate::door::chosen(declared, chose);
    let Some((faces, service)) = door else {
        return FrontDoorReport {
            standing: Standing::Absent,
            service: None,
            address: None,
            facing: None,
            meaning: meaning(Standing::Absent, "", &chosen),
            chosen,
            beside: beside(declared, None, named, recorded, environment),
        };
    };

    let answering = running
        .iter()
        .find(|running| running.id == service.id)
        .is_some_and(|running| answering(running.state));
    let reached = reached(service, named, recorded, environment);
    let standing = standing(faces, answering, reached.is_some());
    FrontDoorReport {
        standing,
        service: Some(service.name.clone()),
        facing: Some(faces),
        meaning: meaning(standing, &service.name, &chosen),
        chosen,
        address: reached,
        beside: beside(
            declared,
            Some(service.id.as_str()),
            named,
            recorded,
            environment,
        ),
    }
}

/// Where the door is reached from another device in the house.
///
/// Nothing for a service the stack publishes no port for: an address with no port
/// on it is one a browser answers with a refusal, and there is nothing to guess
/// at — the manifest is where a port is declared.
fn reached(
    service: &lemonfiber_manifest::Service,
    named: Option<&str>,
    recorded: Option<&str>,
    environment: Environment,
) -> Option<Address> {
    address(named, recorded, environment, service.port?)
}

/// Whether a service in this state could answer somebody arriving at it.
///
/// A container that has not finished starting has not begun answering, which is
/// what makes it different from one that is up: a door reported as open the moment
/// its container exists is a door somebody is sent to before it opens.
const fn answering(state: crate::docker::State) -> bool {
    matches!(
        state,
        crate::docker::State::Healthy
            | crate::docker::State::Running
            | crate::docker::State::HostManaged
    )
}

/// What is said where this stack publishes nothing anybody could begin at.
const NOWHERE: &str = "There is no front door. Nothing this stack runs for the household is \
                       somewhere to begin, so there is no address to hand anybody — and the \
                       one thing worse than saying so would be handing over an address that \
                       leads somewhere they cannot use.";

/// Where the door stands, from what it is, whether it is answering, and whether
/// anything here can say where it would be reached.
///
/// The address is part of the answer rather than a caveat appended to it. This
/// used to report `established` — which the feature defines as running *and*
/// reachable — for a door that was answering on a machine with no address to
/// arrive at, and say the rest in prose. Every consumer that reads the state
/// rather than the sentence was told the door was fine.
const fn standing(faces: Facing, answering: bool, addressed: bool) -> Standing {
    if !answering {
        return Standing::Unreachable;
    }
    if !addressed {
        return Standing::Stranded;
    }
    match faces {
        Facing::Watching => Standing::LibraryOnly,
        _ => Standing::Established,
    }
}

/// What is said where there is a door and nothing here can work out its address.
///
/// The state a fresh install on a machine whose own name is not published is in:
/// the stack ships its household links pointed at this machine and nowhere else,
/// which is the right default for a machine nobody has told where it is and the
/// wrong address to hand anybody. So it is not handed over — it is said, with the
/// one thing that fixes it.
const UNADDRESSED: &str = " Nothing here can work out an address for this machine that another                            device would reach: it does not publish its own name, and the                            address the household's links point at is still the one that means                            this machine and nowhere else. Set `HOMEPAGE_VAR_LAN_HOST` to this                            machine's address on your network and it will be the address given                            here.";

/// What this comes to, in the words an operator would say it in.
///
/// The address is no longer a caveat bolted to whatever else was said: a door
/// nobody can reach has a standing of its own, and that standing's own sentence
/// carries what to do about it. How the door was chosen is the one thing still said
/// after the standing, because it is about the operator's file rather than about
/// their stack.
fn meaning(standing: Standing, name: &str, chosen: &Chosen) -> String {
    let mut said = said(standing, name);
    let after = match chosen {
        Chosen::Derived => return said,
        Chosen::Named(_) => crate::door::KEPT.to_owned(),
        Chosen::Refused(refusal) => refused(refusal),
    };
    said.push(' ');
    said.push_str(&after);
    said
}

/// What is said about a named door this stack will not send a household to.
///
/// The name is quoted back as it was written so the operator can find the line, and
/// the reason is the one the refusal came with rather than a second account of it.
fn refused(refusal: &Refusal) -> String {
    format!(
        "`{}` is named as the front door and it cannot be one: {}. What is said above is \
         what this stack's own shape settles on, and it stands until the setting names \
         something that can be a door.",
        refusal.named, refusal.because
    )
}

/// What the standing itself comes to, before anything is said about the address.
fn said(standing: Standing, name: &str) -> String {
    match standing {
        Standing::Established => format!(
            "Send them to {name}. It is where they ask for what they want, and it links \
             onward to where they watch it."
        ),
        Standing::LibraryOnly => format!(
            "Send them to {name}. This stack has nowhere to ask for anything, so what is \
             already there is what there is to watch."
        ),
        Standing::Unreachable => format!(
            "{name} is the front door and it is not answering, so there is nowhere to send \
             anybody yet. Nothing else here is a stand-in for it."
        ),
        Standing::Stranded => format!("{name} is the front door and it is answering.{UNADDRESSED}"),
        Standing::Absent => NOWHERE.to_owned(),
    }
}

/// Everything else the household can reach, and why none of it is the door.
///
/// In the order the manifest declares them, so the same stack answers the same way
/// twice. The door itself is left out: it is named above, and naming it here as well
/// would be one fact stated in two places that can disagree.
fn beside(
    services: &[lemonfiber_manifest::Service],
    door: Option<&str>,
    named: Option<&str>,
    recorded: Option<&str>,
    environment: Environment,
) -> Vec<Beside> {
    services
        .iter()
        .filter(|service| Some(service.id.as_str()) != door)
        .filter_map(|service| {
            let facing = facing(service)?;
            Some(Beside {
                service: service.name.clone(),
                facing,
                because: facing.because().to_owned(),
                // The same reading the door's own address gets, for the same machine
                // at the same moment. A second way of working out where a service is
                // would be a second answer able to disagree with the first.
                //
                // Only where somebody in the house may be handed the way there.
                // Naming a service and handing over the address of one are different
                // acts: the index over every service is named here in order to be
                // refused by name, and an address on it would be the refusal
                // undone.
                address: facing
                    .handed_over()
                    .then(|| reached(service, named, recorded, environment))
                    .flatten(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
