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
mod tests {
    use super::{chosen, Chosen, Refusal, UNDECLARED, WITHHELD};
    use crate::door::fixtures::{asking, service, watching};
    use crate::door::Facing;
    use lemonfiber_manifest::{ApiKind, Bind};

    /// How the door was chosen, and which service it came out as.
    fn door(
        services: &[lemonfiber_manifest::Service],
        named: Option<&str>,
    ) -> (Chosen, Option<String>) {
        let (chosen, door) = chosen(services, named);
        (chosen, door.map(|(_, service)| service.id.clone()))
    }

    /// The refusal a name comes back with, as the two things it carries.
    fn refused(named: &str, because: &str) -> Chosen {
        Chosen::Refused(Refusal {
            named: named.to_owned(),
            because: because.to_owned(),
        })
    }

    #[test]
    fn naming_nothing_leaves_the_door_where_the_stack_puts_it() {
        let services = [watching(), asking()];
        assert_eq!(
            door(&services, None),
            (Chosen::Derived, Some("seerr".to_owned())),
            "the request surface, as it was before there was a setting"
        );
        // A blank is a mistake rather than an intent, and reads as no name at all.
        assert_eq!(
            door(&services, Some("   ")),
            (Chosen::Derived, Some("seerr".to_owned()))
        );
    }

    #[test]
    fn the_library_can_be_named_over_the_request_surface() {
        // The disagreement the setting exists for: this stack has somewhere to ask,
        // and this operator wants their household sent to what is already there.
        let services = [asking(), watching()];
        assert_eq!(
            door(&services, Some("jellyfin")),
            (
                Chosen::Named("jellyfin".to_owned()),
                Some("jellyfin".to_owned())
            )
        );
    }

    #[test]
    fn a_name_is_read_the_way_somebody_types_it() {
        let services = [asking(), watching()];
        assert_eq!(
            door(&services, Some(" Jellyfin ")),
            (
                Chosen::Named("jellyfin".to_owned()),
                Some("jellyfin".to_owned())
            ),
            "the id the stack declares, whatever case it was written in"
        );
    }

    #[test]
    fn a_service_the_household_tier_does_not_publish_is_refused_and_said() {
        // The refusal this setting exists to make. An admin service answers this
        // machine alone; a front door pointed at one would hand out the address the
        // binding exists to withhold.
        let services = [
            asking(),
            service("sonarr", Some(Bind::Loopback), Some(ApiKind::Servarr)),
        ];
        assert_eq!(
            door(&services, Some("sonarr")),
            (refused("sonarr", WITHHELD), Some("seerr".to_owned())),
            "refused, and the worked-out door still stands"
        );
        assert!(WITHHELD.contains("change what everybody else gets"));
    }

    #[test]
    fn a_service_this_stack_does_not_declare_at_all_is_refused_and_said() {
        let services = [asking()];
        assert_eq!(
            door(&services, Some("plex")),
            (refused("plex", UNDECLARED), Some("seerr".to_owned()))
        );
    }

    #[test]
    fn the_index_over_every_service_is_refused_in_the_registers_own_words() {
        // Naming it would present the household with a page listing every service
        // this stack runs, which is the one thing the register already says it is
        // not for — so the refusal borrows that sentence rather than writing a
        // second one to keep in step with it.
        let services = [asking(), service("homepage", Some(Bind::Lan), None)];
        assert_eq!(
            door(&services, Some("homepage")),
            (
                refused("homepage", Facing::Operators.because()),
                Some("seerr".to_owned())
            )
        );
    }

    #[test]
    fn a_shelf_is_refused_for_the_reason_it_is_not_a_way_in() {
        let services = [asking(), service("audiobookshelf", Some(Bind::Lan), None)];
        assert_eq!(
            door(&services, Some("audiobookshelf")).0,
            refused("audiobookshelf", Facing::Shelf.because())
        );
    }

    #[test]
    fn a_refused_name_over_a_stack_with_no_door_leaves_there_being_none() {
        // Two absences at once. The setting was wrong and the stack has nowhere to
        // send anybody either, and neither stands in for the other.
        let services = [service(
            "sonarr",
            Some(Bind::Loopback),
            Some(ApiKind::Servarr),
        )];
        assert_eq!(
            door(&services, Some("sonarr")),
            (refused("sonarr", WITHHELD), None)
        );
    }

    #[test]
    fn what_was_named_is_carried_back_as_the_operator_wrote_it() {
        // So the answer names the line they have to go and change, rather than a
        // tidied version of it they then cannot find in their file.
        assert_eq!(
            door(&[asking()], Some("  Not-A-Service  ")).0,
            refused("Not-A-Service", UNDECLARED)
        );
    }

    #[test]
    fn a_screen_with_one_line_is_told_the_same_thing_in_fewer_words() {
        assert_eq!(
            Chosen::Derived.said(),
            None,
            "nobody chose it, so nothing is said"
        );
        assert_eq!(
            Chosen::Named("jellyfin".to_owned()).said(),
            Some("named rather than worked out".to_owned())
        );
        assert_eq!(
            refused("sonarr", WITHHELD).said(),
            Some("`sonarr` was named as the front door and cannot be one".to_owned())
        );
    }

    #[test]
    fn how_a_door_was_chosen_reads_the_same_way_to_a_browser_as_to_a_person() {
        // The field a script reads, pinned: a refusal that only ever appeared inside
        // a sentence would be one every consumer but a reader missed.
        let derived = serde_json::to_value(Chosen::Derived).ok();
        assert_eq!(
            derived,
            Some(serde_json::json!({ "chosen": "derived" })),
            "{derived:?}"
        );
        let named = serde_json::to_value(Chosen::Named("jellyfin".to_owned())).ok();
        assert_eq!(
            named,
            Some(serde_json::json!({ "chosen": "named", "door": "jellyfin" })),
            "{named:?}"
        );
        let refused = serde_json::to_value(refused("sonarr", WITHHELD)).ok();
        assert_eq!(
            refused,
            Some(serde_json::json!({
                "chosen": "refused",
                "door": { "named": "sonarr", "because": WITHHELD },
            })),
            "{refused:?}"
        );
    }
}
