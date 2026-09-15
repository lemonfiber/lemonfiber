//! The core capability vocabulary — the named, contracted things a service can do.
//!
//! It exists so that wiring can ask for a capability rather than name a service, and
//! so that a plugin can claim one rather than invent a name for it. Every capability
//! the first two published plugins claim is namespaced with the plugin's own id,
//! because a plugin may not invent a core-looking name and until now there was no core
//! name to use instead — so both of them install a container, pass their proofs, and
//! wire nothing.
//!
//! Three things in this system are called capabilities and only one of them is this:
//! this is what a *service* can do, `[requires].capabilities` is what lemonfiber
//! offers the thing reading it, and `grants` in the stack manifest is what the kernel
//! lets a container do. The three sets are published separately and no name may appear
//! in two of them, because a name whose meaning depended on the field it sat in is the
//! failure the kernel set was renamed out of.
//!
//! **The vocabulary declares what must be shown; the claimant declares where to ask.**
//! A capability is one contract and its claimants are many services, each answering at
//! a path of its own, so the probes here carry the question, the statuses that answer
//! it and the kinds of body constraint required, and a claim binds each of them to a
//! method, a path and a recorded response.

mod carried;

use std::collections::BTreeMap;

use serde::Serialize;
use thiserror::Error;

use carried::{CARRIED, REMOVED};

/// Which of the three sets this is.
///
/// Present in the artefact so a file read out of context cannot be mistaken for
/// another one.
pub const VOCABULARY: &str = "service-capabilities";

/// The generation this vocabulary is at.
///
/// Monotonic. Advanced by a removal or by a contract narrowing; adding a capability
/// does not move it, because nothing already written stops being true.
pub const VOCABULARY_VERSION: u32 = 1;

/// A named, contracted thing a service can do, as this project authors it.
///
/// No `declared_by`: who declares a capability is a fact the stack manifest already
/// states, and restating it here would be a second copy of it that could disagree.
/// [`published`] reads it off the stack at generation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    /// The core name. Unique, and never reused.
    pub name: &'static str,
    /// One line, in the terms the bundled catalogue is written in.
    pub summary: &'static str,
    /// What a service claiming it undertakes to do — the prose a plugin author is
    /// held to.
    pub contract: &'static str,
    /// What must be demonstrated. At least one.
    pub probes: &'static [Probe],
}

/// One thing a claimant must demonstrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Probe {
    /// What a claim names to bind it.
    pub id: &'static str,
    /// What it establishes, in one line.
    pub title: &'static str,
    /// The question it asks, in prose. The claimant says where.
    pub asks: &'static str,
    /// Why this is the evidence worth asking for.
    pub why: &'static str,
    /// Who it is asked as.
    pub credential: Credential,
    /// What an answer has to be for it to have been shown.
    pub requires: Requirement,
}

/// What an answer has to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Requirement {
    /// The statuses that are an acceptable answer. The claimant says which of them
    /// this service gives.
    pub status: &'static [u16],
    /// The kinds of body constraint the answer must carry.
    ///
    /// Empty on a `guarded` probe, and that is not a weaker expectation: a refusal is
    /// the one answer no port proxy can produce. An emptied container answers a
    /// refused connection or a gateway error, and only an application with a protected
    /// surface answers that it is protected — so there the status is the body's job.
    pub body: &'static [Constraint],
}

/// Who a probe is asked as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Credential {
    /// Anyone may ask, so it runs against a live service holding nothing.
    None,
    /// The credential the operator holds for this service.
    ///
    /// Against a live service with nothing held it is **unproven** and never failed: a
    /// manifest cannot hold a credential until recipes arrive, and reporting an
    /// unanswerable question as a failure would say the service is broken when the
    /// runner is.
    Operator,
}

/// A kind of constraint an expectation can put on a body.
///
/// The same vocabulary a proof's expectation and a contributed check's use, so that
/// "a status alone is not evidence" is one rule rather than three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Constraint {
    /// The status alone.
    Status,
    /// Keys carrying exactly these values.
    Json,
    /// Keys that must be present, whatever they hold.
    JsonHasKeys,
    /// Keys that must hold a given kind of thing.
    JsonTypes,
    /// Keys that must hold at least a given number.
    JsonAtLeast,
    /// The answer read as an array, with at least a given number of entries.
    JsonArrayMin,
    /// The answer did not parse as JSON at all.
    JsonIsAbsent,
    /// The content type it is served as.
    ContentType,
    /// What the body must begin with.
    BodyStartsWith,
}

impl Constraint {
    /// The key an expectation writes it as.
    ///
    /// The same word the artefact serialises, held to it by a test below — a listing
    /// that spelled a constraint any other way would be showing an author a key they
    /// could not type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Json => "json",
            Self::JsonHasKeys => "json_has_keys",
            Self::JsonTypes => "json_types",
            Self::JsonAtLeast => "json_at_least",
            Self::JsonArrayMin => "json_array_min",
            Self::JsonIsAbsent => "json_is_absent",
            Self::ContentType => "content_type",
            Self::BodyStartsWith => "body_starts_with",
        }
    }
}

/// A name a published generation carried and this one does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Removed {
    /// The name, so a manifest claiming it is told it *went* rather than that no such
    /// capability exists.
    pub name: &'static str,
    /// The generation it went in.
    pub removed_in: u32,
}

/// The vocabulary as it is published, with each capability's claimants filled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Published {
    /// Which of the three sets this is.
    pub vocabulary: &'static str,
    /// The generation.
    pub vocabulary_version: u32,
    /// Every capability this generation carries.
    pub capabilities: Vec<Declared>,
    /// Every name a published generation carried and this one does not.
    pub removed: &'static [Removed],
}

/// One capability, and the bundled services declaring it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Declared {
    /// The core name.
    pub name: &'static str,
    /// One line.
    pub summary: &'static str,
    /// What a service claiming it undertakes to do.
    pub contract: &'static str,
    /// Every bundled service declaring it, read off the stack manifest.
    pub declared_by: Vec<String>,
    /// What must be demonstrated.
    pub probes: &'static [Probe],
}

/// Why a vocabulary could not be published.
///
/// Both refusals are about the stack and the vocabulary disagreeing, and each is a
/// fault in a different one of them — so the generation fails rather than writing a
/// file that is wrong in a way nobody would see until a plugin stopped wiring.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Unpublishable {
    /// No bundled service declares it.
    ///
    /// An untested contract is worse than an absent one, because a plugin author will
    /// trust it — so a capability nothing declares refuses to be published rather than
    /// waiting for a reviewer to notice.
    #[error("{0} is declared by no bundled service, so its contract has never been shown")]
    Unclaimed(&'static str),
    /// A bundled service declares a name this vocabulary does not carry.
    #[error("service {service} declares {name}, which this vocabulary does not carry")]
    Uncarried {
        /// The service that declared it.
        service: String,
        /// The name it declared.
        name: String,
    },
}

/// Every capability this generation carries, before a stack says who declares them.
#[must_use]
pub fn carried() -> &'static [Capability] {
    CARRIED
}

/// The vocabulary as it is published against a given set of bundled services.
///
/// `declared_by` is read out of the stack manifest rather than written beside each
/// capability, so a bundled service that stops declaring one changes what lemonfiber
/// publishes — and a change nobody meant shows up as a diff in a generated artefact
/// rather than as a plugin that stopped wiring six months later.
///
/// # Errors
///
/// Every disagreement between the two, in one pass: a capability no bundled service
/// declares, and a name a bundled service declares that this vocabulary does not
/// carry. Both are named.
pub fn published(
    services: &[lemonfiber_manifest::Service],
) -> Result<Published, Vec<Unpublishable>> {
    let mut claimants: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut wrong = Vec::new();

    for service in services {
        for name in &service.provides {
            match CARRIED.iter().find(|held| held.name == name) {
                Some(held) => claimants
                    .entry(held.name)
                    .or_default()
                    .push(service.id.clone()),
                None => wrong.push(Unpublishable::Uncarried {
                    service: service.id.clone(),
                    name: name.clone(),
                }),
            }
        }
    }

    let capabilities: Vec<Declared> = CARRIED
        .iter()
        .map(|held| Declared {
            name: held.name,
            summary: held.summary,
            contract: held.contract,
            declared_by: claimants.remove(held.name).unwrap_or_default(),
            probes: held.probes,
        })
        .collect();

    wrong.extend(
        capabilities
            .iter()
            .filter(|declared| declared.declared_by.is_empty())
            .map(|declared| Unpublishable::Unclaimed(declared.name)),
    );

    if wrong.is_empty() {
        Ok(Published {
            vocabulary: VOCABULARY,
            vocabulary_version: VOCABULARY_VERSION,
            capabilities,
            removed: REMOVED,
        })
    } else {
        Err(wrong)
    }
}

#[cfg(test)]
mod tests {
    use super::{carried, published, Constraint, Credential, Unpublishable, VOCABULARY_VERSION};

    /// A stack declaring every capability this generation carries, one service each.
    ///
    /// Built from the vocabulary rather than written out, because a fixture listing
    /// the names by hand would be a second copy of the set — and the test that a
    /// capability nothing declares fails generation would then start failing for the
    /// fixture's reason instead of the rule's.
    fn declaring_everything() -> String {
        let services: String = carried()
            .iter()
            .enumerate()
            .map(|(at, held)| service(&format!("s{at}"), &[held.name]))
            .collect();
        format!(
            "schema_version = 1\nstack_version = \"1.0.0\"\nmin_cli_version = \"0.4.0\"\n\
             [[profile]]\nid = \"p\"\nname = \"P\"\ndescription = \"P\"\n{services}"
        )
    }

    /// One service declaring the given names.
    fn service(id: &str, provides: &[&str]) -> String {
        let named: Vec<String> = provides.iter().map(|name| format!("\"{name}\"")).collect();
        format!(
            "\n[[service]]\nid = \"{id}\"\nname = \"{id}\"\nprofile = \"p\"\n\
             image = \"example.invalid/{id}\"\ntag = \"1.0.0\"\ncriticality = \"core\"\n\
             license = \"MIT\"\nupstream = \"https://example.invalid\"\n\
             last_release = \"2026-01-01\"\ndescribes = \"d\"\nwithout_it = \"w\"\n\
             provides = [{}]\n",
            named.join(", ")
        )
    }

    /// The services a stack description declares, or none where it does not parse.
    fn services(text: &str) -> Vec<lemonfiber_manifest::Service> {
        lemonfiber_manifest::Manifest::from_toml(text)
            .map(|manifest| manifest.services)
            .unwrap_or_default()
    }

    #[test]
    fn a_stack_declaring_everything_publishes_every_capability_with_its_claimants() {
        let declared = services(&declaring_everything());
        let artefact = published(&declared).ok();
        let counted = artefact.as_ref().map(|published| {
            (
                published.vocabulary,
                published.vocabulary_version,
                published.capabilities.len(),
                published
                    .capabilities
                    .iter()
                    .all(|capability| capability.declared_by.len() == 1),
            )
        });
        assert_eq!(
            counted,
            Some((
                "service-capabilities",
                VOCABULARY_VERSION,
                carried().len(),
                true
            ))
        );
    }

    /// Two services declaring one capability both appear against it.
    ///
    /// In the order the stack declares them rather than sorted, because the stack's
    /// order is the one an operator reading the manifest already has.
    #[test]
    fn every_service_declaring_a_capability_is_named_against_it() {
        let Some(first) = carried().first() else {
            unreachable!("the vocabulary carries capabilities")
        };
        let text = format!(
            "{}{}",
            declaring_everything(),
            service("also", &[first.name])
        );
        let claimants = published(&services(&text))
            .ok()
            .and_then(|artefact| artefact.capabilities.into_iter().next())
            .map(|capability| capability.declared_by);
        assert_eq!(claimants, Some(vec!["s0".to_owned(), "also".to_owned()]));
    }

    /// A capability nothing declares fails generation, and is named.
    #[test]
    fn a_capability_no_bundled_service_declares_refuses_to_be_published() {
        let Some(first) = carried().first() else {
            unreachable!("the vocabulary carries capabilities")
        };
        let text = declaring_everything().replace(&format!("\"{}\"", first.name), "");
        let refusal = published(&services(&text)).err().unwrap_or_default();
        assert_eq!(refusal, vec![Unpublishable::Unclaimed(first.name)]);
        assert!(
            refusal
                .first()
                .is_some_and(|said| said.to_string().contains(first.name)),
            "the refusal names the capability: {refusal:?}"
        );
    }

    /// A name the vocabulary does not carry fails generation, and names both.
    #[test]
    fn a_service_declaring_a_name_the_vocabulary_lacks_refuses_to_be_published() {
        let text = format!(
            "{}{}",
            declaring_everything(),
            service("odd", &["not.here"])
        );
        let said = published(&services(&text))
            .err()
            .unwrap_or_default()
            .first()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(said.contains("odd"), "names the service: {said}");
        assert!(said.contains("not.here"), "names the name: {said}");
    }

    /// Both faults in one pass, because a stack is likelier to carry several.
    #[test]
    fn both_disagreements_are_reported_together() {
        let Some(first) = carried().first() else {
            unreachable!("the vocabulary carries capabilities")
        };
        let text = format!(
            "{}{}",
            declaring_everything().replace(&format!("\"{}\"", first.name), ""),
            service("odd", &["not.here"])
        );
        let refusal = published(&services(&text)).err().unwrap_or_default();
        assert_eq!(refusal.len(), 2, "both of them: {refusal:?}");
    }

    /// Every capability carries at least one probe, and every probe says what it is.
    ///
    /// A capability with nothing to demonstrate is an assertion, which is the posture
    /// this whole vocabulary exists to refuse.
    #[test]
    fn every_capability_carries_probes_that_say_what_must_be_shown() {
        let thin: Vec<&str> = carried()
            .iter()
            .filter(|held| {
                held.probes.is_empty()
                    || held.summary.is_empty()
                    || held.contract.is_empty()
                    || held.probes.iter().any(|probe| {
                        probe.id.is_empty()
                            || probe.title.is_empty()
                            || probe.asks.is_empty()
                            || probe.why.is_empty()
                            || probe.requires.status.is_empty()
                    })
            })
            .map(|held| held.name)
            .collect();
        assert!(thin.is_empty(), "these say less than a contract: {thin:?}");
    }

    /// A core name is `area.verb`, which is what lets a plugin's own be told apart
    /// from one of these by reading the name.
    #[test]
    fn every_core_name_is_an_area_and_a_verb_and_nothing_else() {
        let wrong: Vec<&str> = carried()
            .iter()
            .map(|held| held.name)
            .filter(|name| {
                name.matches('.').count() != 1
                    || name.contains(':')
                    || name != &name.to_lowercase()
                    || name.split('.').any(str::is_empty)
            })
            .collect();
        assert!(wrong.is_empty(), "these are not `area.verb`: {wrong:?}");
    }

    /// A probe asked with nothing held that required a body would be asking a question
    /// no anonymous caller can answer — and a refusal is the one thing it can.
    #[test]
    fn a_probe_requiring_no_body_constraint_is_a_refusal() {
        let odd: Vec<&str> = carried()
            .iter()
            .flat_map(|held| held.probes)
            .filter(|probe| probe.requires.body.is_empty())
            .filter(|probe| probe.requires.status.iter().any(|status| *status < 400))
            .map(|probe| probe.id)
            .collect();
        assert!(
            odd.is_empty(),
            "these constrain only a status and are not refusals: {odd:?}"
        );
    }

    /// The two ways a probe can be asked, and the nine kinds of constraint, each
    /// spelled as the artefact spells it.
    #[test]
    fn the_published_words_are_the_ones_the_contract_uses() {
        let credentials: Vec<String> = [Credential::None, Credential::Operator]
            .iter()
            .filter_map(|credential| serde_json::to_string(credential).ok())
            .collect();
        assert_eq!(credentials, vec!["\"none\"", "\"operator\""]);

        let constraints: Vec<String> = [
            Constraint::Status,
            Constraint::Json,
            Constraint::JsonHasKeys,
            Constraint::JsonTypes,
            Constraint::JsonAtLeast,
            Constraint::JsonArrayMin,
            Constraint::JsonIsAbsent,
            Constraint::ContentType,
            Constraint::BodyStartsWith,
        ]
        .iter()
        .filter_map(|constraint| serde_json::to_string(constraint).ok())
        .collect();
        // The word a listing shows and the word the artefact carries are the same
        // word, which is the half a serialisation test cannot see on its own.
        let spelled: Vec<String> = [
            Constraint::Status,
            Constraint::Json,
            Constraint::JsonHasKeys,
            Constraint::JsonTypes,
            Constraint::JsonAtLeast,
            Constraint::JsonArrayMin,
            Constraint::JsonIsAbsent,
            Constraint::ContentType,
            Constraint::BodyStartsWith,
        ]
        .iter()
        .map(|constraint| format!("\"{}\"", constraint.as_str()))
        .collect();
        assert_eq!(constraints, spelled);
        assert_eq!(
            constraints,
            vec![
                "\"status\"",
                "\"json\"",
                "\"json_has_keys\"",
                "\"json_types\"",
                "\"json_at_least\"",
                "\"json_array_min\"",
                "\"json_is_absent\"",
                "\"content_type\"",
                "\"body_starts_with\"",
            ]
        );
    }
}
