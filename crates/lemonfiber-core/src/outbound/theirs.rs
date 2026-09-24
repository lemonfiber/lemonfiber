//! The requests the stack's own services make, attributed to them.
//!
//! An indexer query is Prowlarr asking an indexer. A poster is Radarr asking a
//! metadata provider. A peer connection is qBittorrent being a torrent client.
//! Counting any of those as lemonfiber's would overstate what this product does —
//! and leaving them out would understate what running the stack does, which is the
//! thing an operator is actually deciding about.
//!
//! **The stack is what says so.** A service declares where it reaches and what it
//! asks for in its own manifest entry, and that is what the inventory carries. So
//! adding a service to a Compose project is a change to that project and to nothing
//! here, and the prose describing nineteen services versions with the nineteen
//! services rather than with this binary.
//!
//! It was not always so, and what it replaced is worth stating, because the thing it
//! was defending is real. This file used to carry the prose itself — a table keyed by
//! service id, held against the stack by a set-equality test in both directions, so a
//! service arriving in the stack turned this crate red until somebody edited Rust.
//! That is the one thing adding a service to a Compose project must never cost. But
//! an inventory of what leaves a machine is only honest if a service cannot arrive in
//! it unlisted, and that is what the test bought.
//!
//! Both, now, and from the stack alone. A service the stack describes is described. A
//! service nothing describes is carried into the inventory saying lemonfiber has no
//! record of what it reaches, which is the truth and is neither a guess nor a silent
//! omission. And the shipped stack is held to describing all of its own: the test
//! below fails the build rather than the operator, because the pairing of this binary
//! with the stack it embeds is decided here and not by somebody's machine.
//!
//! What a second copy here would cost is not hypothetical. The table outlived its
//! last reader by one release and had already drifted from the manifest in the one
//! entry nobody happened to reread — which is what a duplicate does when only one of
//! the two is the one anything consults.

use super::Elsewhere;
use lemonfiber_manifest::Service;

/// Where an unrecorded service is said to reach, which is the one thing that can
/// honestly be said about it.
///
/// A sentence rather than an empty string, because empty already means *nothing
/// leaves this machine* in this report and the two are opposite claims.
const UNKNOWN: &str = "not known to lemonfiber";

/// What is said about a destination a plugin's recipes declare.
const CARRIED: &str = "A destination this plugin's recipes declare they may call, or carry \
                       a value it captured to. Declared in its manifest and held to it: a \
                       recipe reaching anywhere else is refused before it is installed.";

/// What is said about a service this build ships no record for.
const NO_RECORD: &str = "This service is not one lemonfiber knows, so nothing here can say \
                         where it reaches or what it asks for. It is listed because leaving \
                         it out would make this inventory read as complete while it was \
                         short. Its own documentation, and the Compose file that declares \
                         it, are what answer this.";

/// What the services in this stack reach, in the order the stack declares them.
///
/// Every service declared, whether or not it describes itself. One that does not is
/// carried anyway and reported as undescribed: never dropped, and never rendered as
/// reaching nothing.
pub(super) fn elsewhere(services: &[Service]) -> Vec<Elsewhere> {
    services
        .iter()
        .map(|service| from_the_stack(service).unwrap_or_else(|| no_record_of(service)))
        .collect()
}

/// What a service says about itself, where its own manifest entry says anything.
///
/// Both halves or neither, which the manifest's own validation holds it to: half an
/// answer here would attribute a purpose to a service the same report says goes
/// nowhere, so it is treated as the silence it is. This is the only source, because
/// it is the one that travels with the stack — a fork describes its own services, and
/// a service added to the stack needs no release of this binary to be described.
fn from_the_stack(service: &Service) -> Option<Elsewhere> {
    let destination = service.reaches.clone()?;
    let purpose = service.asks_for.clone()?;
    Some(Elsewhere {
        service: service.id.clone(),
        destination,
        purpose,
        recorded: true,
        origin: crate::origin::Origin::Bundled,
    })
}

/// The admission that nothing describes this service.
///
/// Reached by a service an operator's own stack declares and says nothing about. For
/// the stack this binary embeds it is reached by nothing at all, which is a property
/// of that stack rather than of this function, and the test below is what keeps it
/// one.
fn no_record_of(service: &Service) -> Elsewhere {
    Elsewhere {
        service: service.id.clone(),
        destination: UNKNOWN.to_owned(),
        purpose: NO_RECORD.to_owned(),
        recorded: false,
        origin: crate::origin::Origin::Bundled,
    }
}

/// What installed plugins bring to this account: each of their services, and every
/// destination outside the stack their recipes declare.
///
/// A plugin's service says nothing about where it reaches — the manifest format has no
/// field for it — so it is listed as one lemonfiber has no record of, attributed to its
/// plugin, for the reason an undescribed bundled service is listed: leaving it out
/// would make the account read as complete while it was short. A destination a recipe
/// declares is recorded, because the plugin declared it and was held to it, and it is
/// listed only where it is outside the stack: a recipe reaching one of the stack's own
/// services is not something leaving the machine.
pub(super) fn brought(stack: &[Service], installed: &[crate::plugin::Installed]) -> Vec<Elsewhere> {
    let inside = |to: &str| stack.iter().any(|service| service.id == to);
    installed
        .iter()
        .flat_map(|one| {
            let origin = crate::origin::Origin::Plugin {
                named: one.plugin.clone(),
            };
            let services = one.services.iter().map({
                let origin = origin.clone();
                move |placed| Elsewhere {
                    service: placed.service.clone(),
                    destination: UNKNOWN.to_owned(),
                    purpose: NO_RECORD.to_owned(),
                    recorded: false,
                    origin: origin.clone(),
                }
            });
            let hosts = one
                .declared
                .reaches
                .iter()
                .filter(|to| !inside(to))
                .map(move |to| Elsewhere {
                    service: one.plugin.clone(),
                    destination: to.clone(),
                    purpose: CARRIED.to_owned(),
                    recorded: true,
                    origin: origin.clone(),
                });
            services.chain(hosts).collect::<Vec<Elsewhere>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::elsewhere;

    fn declared() -> Vec<lemonfiber_manifest::Service> {
        crate::test_support::stack()
            .manifest()
            .map(|manifest| manifest.services)
            .unwrap_or_default()
    }

    /// Every service the shipped stack declares says where it reaches.
    ///
    /// This is what is left of the old set-equality, and it no longer costs what that
    /// one cost: a service added to the stack answers for itself in its own manifest
    /// entry, which is a change to the stack and not to this binary. What it refuses
    /// is a service that says nothing — because an inventory of what leaves a machine
    /// is only honest if a service cannot arrive in it unlisted, and the shipped stack
    /// is the one stack whose pairing with this binary is decided here rather than by
    /// an operator.
    #[test]
    fn every_service_the_shipped_stack_declares_says_where_it_reaches() {
        let services = declared();
        let counted = services.len();
        // Named rather than left to the emptiness check below, which a stack that
        // failed to load satisfies twice over: nothing declared and nothing wrong are
        // the same answer. Every worktree here shares one build cache, so a test
        // binary can read another tree's copy of this file.
        assert!(
            counted > 10,
            "the stack declares {counted} services, which means this is reading the wrong manifest"
        );

        let silent: Vec<String> = elsewhere(&services)
            .into_iter()
            .filter(|entry| !entry.recorded)
            .map(|entry| entry.service)
            .collect();
        assert!(
            silent.is_empty(),
            "these are in the stack this build ships and nothing says what they reach. \
             Say it in the stack's own manifest entry — `reaches` and `asks_for`, which \
             travel with the stack: {silent:?}"
        );
    }

    /// And says something when it does.
    ///
    /// Checked beside the one above because the two fail the same way and only one of
    /// them looks like a failure. A service recorded with nothing to say renders as an
    /// entry with an empty purpose, which reads as a description somebody wrote and
    /// found unremarkable rather than as one nobody wrote at all.
    #[test]
    fn every_service_the_shipped_stack_declares_says_what_it_asks_for() {
        let hollow: Vec<String> = elsewhere(&declared())
            .into_iter()
            .filter(|entry| entry.purpose.split_whitespace().count() < 6)
            .map(|entry| entry.service)
            .collect();
        assert!(
            hollow.is_empty(),
            "these are recorded and the record does not say what they ask for: {hollow:?}"
        );
    }

    /// The manifest wins where it speaks, which is what makes adding a service to a
    /// Compose project cost nothing here.
    #[test]
    fn a_service_that_describes_itself_is_taken_at_its_word() {
        let theirs: Vec<_> = declared()
            .into_iter()
            .take(1)
            .map(|mut service| {
                service.id = "somebodys-own-service".to_owned();
                service.reaches = Some("a place of their own".to_owned());
                service.asks_for = Some("Whatever they built it to ask for.".to_owned());
                service
            })
            .collect();

        let found = elsewhere(&theirs);
        let entry = found.first();
        assert!(
            entry.is_some_and(|one| one.destination == "a place of their own"),
            "{found:?}"
        );
        assert!(entry.is_some_and(|one| one.recorded), "{found:?}");
    }

    /// An empty destination is an answer, and the whole of what this inventory is for
    /// turns on it reading as one. "Nothing leaves this machine" is the strongest
    /// thing a privacy inventory can say about a service, and a stack that says it
    /// must not come back indistinguishable from a stack that said nothing at all.
    #[test]
    fn a_stack_saying_a_service_reaches_nothing_is_recorded_as_having_said_so() {
        let theirs: Vec<_> = declared()
            .into_iter()
            .take(1)
            .map(|mut service| {
                service.id = "somebodys-own-service".to_owned();
                service.reaches = Some(String::new());
                service.asks_for = Some("Nothing leaves this machine.".to_owned());
                service
            })
            .collect();

        let found = elsewhere(&theirs);
        let entry = found.first();
        assert!(
            entry.is_some_and(|one| one.destination.is_empty()),
            "{found:?}"
        );
        assert!(entry.is_some_and(|one| one.recorded), "{found:?}");
    }

    /// Half an answer is no answer, and what is left when it is taken away is the
    /// admission that nothing was said.
    ///
    /// A stack that says where a service reaches and not what it asks for there has
    /// described a destination with no purpose attached — and a purpose is the half
    /// an operator actually reads, because "it talks to a metadata provider" and
    /// "it sends your library to a metadata provider" are the same destination and
    /// different decisions. The manifest's own validation holds a stack to both
    /// halves or neither, so this is the shape that arrives when something has gone
    /// wrong upstream of it; taking the half offered would attribute a purpose this
    /// service never claimed, or leave one blank in a report whose blanks already
    /// mean *nothing leaves this machine*.
    #[test]
    fn half_an_answer_from_the_stack_is_no_answer_at_all() {
        let theirs: Vec<_> = declared()
            .into_iter()
            .filter(|one| one.id == "prowlarr")
            .take(1)
            .map(|mut prowlarr| {
                prowlarr.reaches = Some("somewhere this binary never heard of".to_owned());
                prowlarr.asks_for = None;
                prowlarr
            })
            .collect();

        let carried = elsewhere(&theirs);
        let entry = carried.first();
        assert!(entry.is_some_and(|one| !one.recorded), "{carried:?}");
        assert!(
            entry.is_some_and(|one| !one.destination.contains("somewhere this binary")),
            "half an answer must not reach the report as a whole one: {carried:?}"
        );
    }

    /// And a service this binary once carried prose about is still the stack's to
    /// describe: the point of the field is that a fork can correct what shipped with
    /// it, and prowlarr is the case where something used to answer instead.
    #[test]
    fn a_service_this_binary_once_described_is_still_the_stacks_to_describe() {
        let theirs: Vec<_> = declared()
            .into_iter()
            .filter(|service| service.id == "prowlarr")
            .take(1)
            .map(|mut prowlarr| {
                prowlarr.reaches = Some("somewhere this binary never heard of".to_owned());
                prowlarr.asks_for = Some("Something this binary never heard of either.".to_owned());
                prowlarr
            })
            .collect();

        let found = elsewhere(&theirs);
        assert!(
            found
                .first()
                .is_some_and(|one| one.destination == "somewhere this binary never heard of"),
            "{found:?}"
        );
    }

    #[test]
    fn the_list_follows_the_stack_it_is_given() {
        let one = declared().into_iter().take(1).collect::<Vec<_>>();
        let found = elsewhere(&one);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found
                .iter()
                .any(|entry| entry.service == "prowlarr" && entry.destination.contains("indexers")),
            "{found:?}"
        );
    }

    /// The case the whole rearrangement is for: a service nobody has written anything
    /// about is carried into the inventory saying so, rather than dropped from it.
    #[test]
    fn a_service_nothing_is_written_down_about_is_reported_rather_than_left_out() {
        let unknown: Vec<lemonfiber_manifest::Service> = declared()
            .into_iter()
            .take(1)
            .map(|mut service| {
                service.id = "something-new".to_owned();
                // And says nothing for itself. A stack may now declare where a
                // service reaches, and a renamed entry keeping those two lines is a
                // service this build has been told about rather than one it knows
                // nothing of — which is the opposite of what these cases are.
                service.reaches = None;
                service.asks_for = None;
                service
            })
            .collect();
        assert_eq!(unknown.len(), 1, "the stack declares services to rename");

        let found = elsewhere(&unknown);
        let entry = found.first();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            entry.is_some_and(|one| one.service == "something-new"),
            "{found:?}"
        );
        assert!(entry.is_some_and(|one| !one.recorded), "{found:?}");
    }

    /// And it is not reported as reaching nothing, which is the one wrong answer here.
    ///
    /// An empty destination is what `unpackerr` and `caddy` say, and it means no
    /// request leaves this machine. Saying that about a service nobody has looked at
    /// would be this product making a privacy claim out of its own ignorance.
    #[test]
    fn an_unknown_service_is_never_reported_as_reaching_nothing() {
        let unknown: Vec<lemonfiber_manifest::Service> = declared()
            .into_iter()
            .take(1)
            .map(|mut service| {
                service.id = "something-new".to_owned();
                // And says nothing for itself. A stack may now declare where a
                // service reaches, and a renamed entry keeping those two lines is a
                // service this build has been told about rather than one it knows
                // nothing of — which is the opposite of what these cases are.
                service.reaches = None;
                service.asks_for = None;
                service
            })
            .collect();

        let found = elsewhere(&unknown);
        let entry = found.first();
        assert!(
            entry.is_some_and(|one| !one.destination.is_empty()),
            "an empty destination reads as nothing leaving this machine: {found:?}"
        );
        assert!(
            entry.is_some_and(|one| one.purpose.contains("not one lemonfiber knows")),
            "{found:?}"
        );
    }

    /// Adding a service to the stack is a Compose change and a manifest change and
    /// nothing else. Asserted here because the rule it replaces was asserted here,
    /// and a rule taken out without one to stand in its place comes back.
    #[test]
    fn a_service_added_to_the_stack_is_reported_without_a_change_here() {
        let mut services = declared();
        let copied: Vec<_> = services
            .first()
            .cloned()
            .into_iter()
            .map(|mut service| {
                service.id = "somebodys-own-service".to_owned();
                service
            })
            .collect();
        let counted = services.len();
        services.extend(copied);
        // Said rather than left to the count below, which an empty stack satisfies
        // twice over: nothing declared and nothing reported are equal.
        assert_eq!(
            services.len(),
            counted + 1,
            "the stack declares a service to copy"
        );

        let found = elsewhere(&services);
        assert_eq!(
            found.len(),
            services.len(),
            "every declared service reaches the inventory: {found:?}"
        );
    }
}

#[cfg(test)]
mod brought {
    use super::{brought, CARRIED, NO_RECORD};
    use crate::origin::Origin;
    use crate::plugin::Register;

    /// One installed plugin with one service, whose recipes reach one host outside the
    /// stack and one of the stack's own services.
    const INSTALLED: &str = r#"{
      "installed": [
        {
          "plugin": "comics",
          "version": "1.2.0",
          "services": [
            {
              "service": "komga",
              "image": "docker.io/gotson/komga",
              "digest": "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
              "tag": "1.11.0",
              "config_path": "/config",
              "takes_data": true
            }
          ],
          "declared": { "reaches": ["metadata.example", "sonarr"] }
        }
      ]
    }"#;

    fn stack() -> Vec<lemonfiber_manifest::Service> {
        crate::test_support::stack()
            .manifest()
            .map(|manifest| manifest.services)
            .unwrap_or_default()
    }

    /// What the plugin in [`INSTALLED`] brings to the shipped stack's account.
    fn listed() -> Vec<super::Elsewhere> {
        Register::parse(INSTALLED)
            .map(|register| brought(&stack(), register.installed()))
            .unwrap_or_default()
    }

    fn comics() -> Origin {
        Origin::Plugin {
            named: "comics".to_owned(),
        }
    }

    #[test]
    fn a_plugins_service_is_listed_as_unrecorded_and_as_the_plugins() {
        let listed = listed();

        let service = listed.iter().find(|one| one.service == "komga");
        assert_eq!(
            service.map(|one| (one.recorded, one.purpose.as_str(), one.origin.clone())),
            Some((false, NO_RECORD, comics())),
            "{listed:?}"
        );
    }

    #[test]
    fn a_host_its_recipes_declare_is_listed_as_the_plugins_and_one_inside_the_stack_is_not() {
        let listed = listed();

        let hosts: Vec<(&str, &str, bool, Origin)> = listed
            .iter()
            .filter(|one| one.purpose == CARRIED)
            .map(|one| {
                (
                    one.service.as_str(),
                    one.destination.as_str(),
                    one.recorded,
                    one.origin.clone(),
                )
            })
            .collect();
        assert_eq!(
            hosts,
            vec![("comics", "metadata.example", true, comics())],
            "{listed:?}"
        );
    }

    #[test]
    fn nothing_installed_brings_nothing() {
        assert!(brought(&stack(), &[]).is_empty());
    }
}
