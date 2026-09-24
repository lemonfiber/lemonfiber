//! The door the operator named, and the ones a setting may not name.
//!
//! The call this feature makes — that a household begins where they can ask for
//! something — is a call somebody may reasonably disagree with about their own
//! stack, so it is a setting. What a setting cannot be is a way around the tiers.
//! The services that can reconfigure this stack answer this machine and nowhere
//! else, and a front door pointed at one would hand the household the very address
//! that arrangement exists to withhold. So a recorded name is read as a request
//! rather than as an instruction: obeyed where it names somewhere this stack
//! already publishes to the household *and* already holds to be a place to begin,
//! and refused in words where it does not.
//!
//! A refusal leaves the worked-out door standing rather than answering that there
//! is none. What was wrong is the setting, and the stack is not: a household that
//! could be sent somewhere is still one that can be, and reporting no front door
//! over a misspelt line would take a true answer away and send the operator looking
//! at their stack instead of at their file. What a refusal must not do is go
//! quietly — an operator shown a door they did not name, with no reason beside it,
//! has been told their setting worked. So it is carried on the answer as a state of
//! its own rather than as a sentence under one, the way every other thing here that
//! a browser or a script has to be able to read is.

use serde::Serialize;

use lemonfiber_manifest::Service;

use super::{begins_at, facing, Facing};

/// What is said where nothing this stack declares goes by the name that was given.
const UNDECLARED: &str = "this stack declares no service by that name, so there is nothing \
                          behind it to send anybody to";

/// What is said where the name reaches a service the household tier does not publish.
const WITHHELD: &str = "it answers this machine and nowhere else, which is where the services \
                        that can change what this stack does are kept — somebody in the house \
                        who arrived there could change what everybody else gets, so it is not \
                        an address to hand out";

/// What naming a front door costs, said wherever one is named.
///
/// The one thing lemonfiber stops keeping right. Every other answer here is worked
/// out afresh from what the stack declares, so a stack that changes is answered
/// about as it is now; a named door is answered about as it was decided, and the
/// day this stack grows somewhere to ask for things nothing will notice that the
/// decision has been overtaken — because from here it has not been overtaken, it
/// has been kept.
///
/// Said at the moment the operator sets it, which is the only moment they are
/// deciding, and again in the answer, which is where somebody who did not set it
/// finds out why the door is the one it is.
pub const KEPT: &str = "A front door that is named rather than worked out stays what it names \
                        whatever this stack becomes: gain somewhere to ask for things later, and \
                        the household will still be sent to what the setting says.";

/// How the front door came to be the one it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", tag = "chosen", content = "door")]
pub enum Chosen {
    /// Worked out from what the stack declares, which is what a stack whose
    /// operator has named nothing answers.
    Derived,
    /// Named by the operator, by the id the stack declares it under, and it is the
    /// door.
    Named(String),
    /// Named by the operator and refused. The worked-out door stands, and this
    /// carries what was named and why it is not it.
    Refused(Refusal),
}

/// A named front door that is not one, and why it is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Refusal {
    /// What the operator recorded, as they wrote it.
    pub named: String,
    /// Why this stack will not send a household there.
    pub because: String,
}

impl Chosen {
    /// How this door was chosen, in one phrase, or nothing where nobody chose it.
    ///
    /// For the surfaces with a line rather than a paragraph — a panel on a screen is
    /// two lines and cannot carry the answer's own sentence. Said here so those
    /// surfaces do not each write a shorter version of it and come to disagree with
    /// the long one about what was refused.
    #[must_use]
    pub fn said(&self) -> Option<String> {
        match self {
            Self::Derived => None,
            Self::Named(_) => Some("named rather than worked out".to_owned()),
            Self::Refused(refusal) => Some(format!(
                "`{}` was named as the front door and cannot be one",
                refusal.named
            )),
        }
    }
}

/// Which service the household begins at, and how it came to be that one.
///
/// `named` is what the operator recorded. Nothing there leaves the answer to
/// [`begins_at`], which is the whole of what decided this before there was a
/// setting to consult.
#[must_use]
pub fn chosen<'a>(
    services: &'a [Service],
    named: Option<&str>,
) -> (Chosen, Option<(Facing, &'a Service)>) {
    let derived = begins_at(services);
    let Some(named) = named.map(str::trim).filter(|named| !named.is_empty()) else {
        return (Chosen::Derived, derived);
    };
    match offered(services, named) {
        Ok((facing, service)) => (Chosen::Named(service.id.clone()), Some((facing, service))),
        Err(because) => (
            Chosen::Refused(Refusal {
                named: named.to_owned(),
                because,
            }),
            derived,
        ),
    }
}

/// The service a recorded name reaches, or why the household is not sent to it.
///
/// Matched without regard to case, because this is a line in a file somebody types:
/// a stack declaring `jellyfin` and an operator writing `Jellyfin` mean the same
/// service, and refusing the second would be refusing a name that is right.
///
/// The last refusal borrows the register's own words rather than writing a second
/// set. Why the index over every service is not a way in has an answer already, and
/// two answers to one question is one of them going stale.
fn offered<'a>(services: &'a [Service], named: &str) -> Result<(Facing, &'a Service), String> {
    let Some(service) = services
        .iter()
        .find(|service| service.id.eq_ignore_ascii_case(named))
    else {
        return Err(UNDECLARED.to_owned());
    };
    let Some(facing) = facing(service) else {
        return Err(WITHHELD.to_owned());
    };
    if facing.begins() {
        return Ok((facing, service));
    }
    Err(facing.because().to_owned())
}

#[cfg(test)]
mod tests;
