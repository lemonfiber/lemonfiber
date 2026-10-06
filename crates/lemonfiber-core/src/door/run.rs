//! The one address to hand somebody who lives here.
//!
//! [`crate::door`] decides which service that is from what the stack declares; this
//! asks the engine what became of it and assembles the answer every surface shows.
//! The reading of what is running is the same survey the status view is built from,
//! so the two cannot grade one service differently.

use crate::app::Ctx;
use crate::door::{address, proxied, Address, Candidate, Chosen, Facing, Place, Reached, Refusal};
use crate::error::{Diagnose, Problem};
use crate::model::{Beside, FrontDoorReport, Standing};

/// What the engine says is running: the stack's services as the status survey grades
/// them, and every container the stack does not declare — a plugin's among them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Running<'a> {
    /// The stack's own services.
    pub(crate) surveyed: &'a [crate::docker::Service],
    /// Everything else running under the project.
    pub(crate) undeclared: &'a [crate::docker::Undeclared],
}

impl Running<'_> {
    /// How the service running as `id` stands, where anything runs as it.
    fn state(&self, id: &str) -> Option<crate::docker::State> {
        self.surveyed
            .iter()
            .find(|one| one.id == id)
            .map(|one| one.state)
            .or_else(|| {
                self.undeclared
                    .iter()
                    .find(|one| one.id == id)
                    .map(|one| one.state)
            })
    }
}

/// What there is to hand somebody who lives here, and where it stands.
pub(crate) async fn front_door(ctx: &Ctx) -> Result<FrontDoorReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let containers = ctx
        .seams
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    let profiles: Vec<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    let halted = crate::app::engine::halted::load(ctx);
    let surveyed = crate::docker::survey(
        &manifest,
        &profiles,
        &containers,
        &halted,
        ctx.settings.protocols,
    );
    let undeclared = crate::docker::undeclared(&manifest, &containers, &halted);
    // A record of what is installed that will not read leaves the stack's own services
    // to answer for the door: what is shown is made from what could be read.
    let register =
        crate::app::plugins::read(ctx).unwrap_or_else(|_| crate::plugin::Register::empty());
    // Asked now rather than remembered: a machine renamed since the last look
    // answers as it is, which is the whole of how a changed address is noticed.
    let named = ctx.site.name().await;
    Ok(assembled(
        &crate::door::candidates(&manifest.services, register.installed()),
        Running {
            surveyed: &surveyed,
            undeclared: &undeclared,
        },
        &place(ctx, named.as_deref()),
        ctx.settings.front_door.as_deref(),
    ))
}

/// Where this machine is, as the settings and its own name say, for every address the
/// household is handed.
pub(crate) fn place<'a>(ctx: &'a Ctx, named: Option<&'a str>) -> Place<'a> {
    Place {
        named,
        recorded: ctx.settings.household_host.as_deref(),
        domain: ctx.settings.household_domain.as_deref(),
        environment: ctx.environment,
    }
}

/// The answer itself, over what the stack declares and what became of it.
///
/// Apart from the reading above so that the stacks worth asking about — one that
/// publishes nothing to the household at all — can be put to it, which no stack this
/// repository carries is.
pub(crate) fn assembled(
    candidates: &[Candidate<'_>],
    running: Running<'_>,
    place: &Place<'_>,
    chose: Option<&str>,
) -> FrontDoorReport {
    let (chosen, door) = crate::door::chosen(candidates, chose);
    let Some((faces, door)) = door else {
        return FrontDoorReport {
            standing: Standing::Absent,
            service: None,
            address: None,
            facing: None,
            meaning: meaning(Standing::Absent, "", UNADDRESSED, &chosen),
            chosen,
            beside: beside(candidates, None, place),
        };
    };

    let answering = arrives(door, candidates, running);
    let reached = reached(door, place);
    let standing = standing(faces, answering, reached.is_some());
    FrontDoorReport {
        standing,
        service: Some(door.name.to_owned()),
        facing: Some(faces),
        meaning: meaning(standing, door.name, unaddressed(door), &chosen),
        chosen,
        address: reached,
        beside: beside(candidates, Some(door.id), place),
    }
}

/// Whether somebody sent to `door` would be answered: it is running, and where the
/// household reaches it only through the stack's proxy, so is the proxy. A plugin's
/// service answering behind a proxy that is not running is a door nobody arrives at.
fn arrives(door: &Candidate<'_>, candidates: &[Candidate<'_>], running: Running<'_>) -> bool {
    let up = |candidate: &Candidate<'_>| running.state(candidate.id).is_some_and(answering);
    up(door)
        && match door.reached {
            Reached::Port(_) => true,
            Reached::Proxied(_) => candidates
                .iter()
                .filter(|candidate| candidate.facing == Some(Facing::Carriage))
                .any(up),
        }
}

/// Where a service is reached from another device in the house.
///
/// At this machine on the port the stack publishes, for the stack's own — nothing for
/// one it publishes no port for, since an address with no port on it is one a browser
/// answers with a refusal and the manifest is where a port is declared. Through the
/// stack's proxy at its label in front of the configured domain, for a plugin's — the
/// one way the household reaches it, and nothing where no domain is configured.
pub(crate) fn reached(candidate: &Candidate<'_>, place: &Place<'_>) -> Option<Address> {
    match candidate.reached {
        Reached::Port(port) => address(place.named, place.recorded, place.environment, port?),
        Reached::Proxied(label) => proxied(label, place.domain),
    }
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
/// The address is part of the answer rather than a caveat appended to it.
/// `established` means running *and* reachable, so a door answering on a machine
/// with no address to arrive at is not it: a consumer that reads the state rather
/// than the sentence would be told the door was fine.
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

/// What is said where nothing here can work out where `door` is reached: the fix for
/// the way it is published.
const fn unaddressed(door: &Candidate<'_>) -> &'static str {
    match door.reached {
        Reached::Port(_) => UNADDRESSED,
        Reached::Proxied(_) => UNPROXIED,
    }
}

/// What is said where the door is a plugin's service and no domain is configured for
/// the proxy that publishes it.
///
/// The household reaches it only through the stack's proxy, at a name in front of the
/// operator's domain; with none written, or one kept for examples, there is no name
/// another device is promised to resolve, and none is invented.
const UNPROXIED: &str = " It is published to the household only through the stack's proxy, \
                         at a name in front of a domain, and no domain is configured for it. \
                         Set `DOMAIN` to a domain whose addresses point at this machine and \
                         the address the proxy publishes it at will be the one given here.";

/// What this comes to, in the words an operator would say it in.
///
/// The address is no longer a caveat bolted to whatever else was said: a door
/// nobody can reach has a standing of its own, and that standing's own sentence
/// carries what to do about it. How the door was chosen is the one thing still said
/// after the standing, because it is about the operator's file rather than about
/// their stack.
fn meaning(standing: Standing, name: &str, unaddressed: &str, chosen: &Chosen) -> String {
    let mut said = said(standing, name, unaddressed);
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
fn said(standing: Standing, name: &str, unaddressed: &str) -> String {
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
        Standing::Stranded => format!("{name} is the front door and it is answering.{unaddressed}"),
        Standing::Absent => NOWHERE.to_owned(),
    }
}

/// Everything else the household can reach, and why none of it is the door.
///
/// The stack's in the order the manifest declares them, then each plugin's, so the same
/// stack answers the same way twice. The door itself is left out: it is named above,
/// and naming it here as well would be one fact stated in two places that can disagree.
fn beside(candidates: &[Candidate<'_>], door: Option<&str>, place: &Place<'_>) -> Vec<Beside> {
    candidates
        .iter()
        .filter(|candidate| Some(candidate.id) != door)
        .filter_map(|candidate| {
            let facing = candidate.facing?;
            Some(Beside {
                service: candidate.name.to_owned(),
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
                    .then(|| reached(candidate, place))
                    .flatten(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
