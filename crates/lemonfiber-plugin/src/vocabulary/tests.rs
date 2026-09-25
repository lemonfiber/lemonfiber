use std::collections::BTreeSet;

use super::{
    carried, is_core_name, published, Constraint, Credential, Unpublishable, VOCABULARY_VERSION,
};

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
///
/// Asked of every capability in turn rather than of the first, and not only for
/// thoroughness: "the first" is an option, and an arm for a vocabulary carrying
/// nothing is a line no run can ever enter.
#[test]
fn every_service_declaring_a_capability_is_named_against_it() {
    for (at, held) in carried().iter().enumerate() {
        let text = format!(
            "{}{}",
            declaring_everything(),
            service("also", &[held.name])
        );
        let claimants = published(&services(&text))
            .ok()
            .and_then(|artefact| artefact.capabilities.into_iter().nth(at))
            .map(|capability| capability.declared_by);
        assert_eq!(
            claimants,
            Some(vec![format!("s{at}"), "also".to_owned()]),
            "{}",
            held.name
        );
    }
}

/// A capability nothing declares fails generation, and is named.
#[test]
fn a_capability_no_bundled_service_declares_refuses_to_be_published() {
    for held in carried() {
        let text = declaring_everything().replace(&format!("\"{}\"", held.name), "");
        let refusal = published(&services(&text)).err().unwrap_or_default();
        assert_eq!(refusal, vec![Unpublishable::Unclaimed(held.name)]);
        let said: String = refusal.iter().map(ToString::to_string).collect();
        assert!(
            said.contains(held.name),
            "the refusal names the capability: {said}"
        );
    }
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
    // The first name, reached without asking whether there is one: an arm for a
    // vocabulary that carries nothing is a line no run can enter.
    let mut text = declaring_everything();
    for held in carried().iter().take(1) {
        text = text.replace(&format!("\"{}\"", held.name), "");
    }
    let text = format!("{text}{}", service("odd", &["not.here"]));
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
///
/// Asked through the same function a manifest's names are asked through, rather
/// than by a second reading of the rule here: a shape these names satisfied and a
/// plugin's were refused for would be two rules wearing one name.
#[test]
fn every_core_name_is_an_area_and_a_verb_and_nothing_else() {
    let wrong: Vec<&str> = carried()
        .iter()
        .map(|held| held.name)
        .filter(|name| !is_core_name(name))
        .collect();
    assert!(wrong.is_empty(), "these are not `area.verb`: {wrong:?}");
}

/// The shapes that are not core names, each for its own reason.
///
/// A namespaced name is the one that matters — it is what a plugin's own capability
/// looks like, and reading one as a core name would have the vocabulary refuse it
/// as unknown instead of accepting it as inert.
#[test]
fn a_name_that_is_not_an_area_and_a_verb_is_not_a_core_name() {
    for odd in [
        "komga:opds",
        "media",
        "media.serve.now",
        "Media.Serve",
        "media.",
        ".serve",
        "media.serve-",
        "1media.serve",
        "media serve",
    ] {
        assert!(!is_core_name(odd), "{odd} reads as a core name");
    }
    assert!(is_core_name("media.serve"), "the ordinary shape");
    assert!(
        is_core_name("network.egress-guard"),
        "a hyphen inside a half"
    );
}

/// No name is in two of the three sets called capabilities, and none of the three
/// can be confused by somebody who does not know which document they are reading.
///
/// One is what a *service* can do, one is what lemonfiber offers the manifest
/// reading it, and one is a hole in a container's isolation. The last two were one
/// word until the stack manifest's field was renamed, and the rule that kept them
/// apart afterwards was that nobody would pick the same name twice, which is not a
/// rule. Shape decides it instead: a kernel grant is shouted and carries no dot, and
/// the other two are an area and a verb — so the third is held apart by membership,
/// which is what this asks.
#[test]
fn no_name_is_in_two_of_the_three_sets_called_capabilities() {
    let granted: BTreeSet<&str> = lemonfiber_manifest::ALLOWED_GRANTS
        .iter()
        .copied()
        .collect();
    assert!(!granted.is_empty(), "there are kernel grants to be unlike");
    let shared: Vec<&str> = carried()
        .iter()
        .map(|held| held.name)
        .filter(|name| granted.contains(name))
        .collect();
    assert!(shared.is_empty(), "these are in both sets: {shared:?}");

    let offered = crate::offering::offered();
    assert!(
        !offered.is_empty(),
        "there are offers to be unlike; an empty set would agree with anything"
    );
    let twice: Vec<&str> = carried()
        .iter()
        .map(|held| held.name)
        .chain(granted.iter().copied())
        .filter(|name| offered.contains(name))
        .collect();
    assert!(
        twice.is_empty(),
        "these are what lemonfiber offers and something else too: {twice:?}"
    );

    let readable: Vec<&str> = granted
        .iter()
        .copied()
        .filter(|grant| is_core_name(grant))
        .collect();
    assert!(
        readable.is_empty(),
        "these kernel grants read as capabilities: {readable:?}"
    );
    let shouted: Vec<&str> = carried()
        .iter()
        .map(|held| held.name)
        .filter(|name| *name == name.to_uppercase())
        .collect();
    assert!(
        shouted.is_empty(),
        "these capabilities read as kernel grants: {shouted:?}"
    );
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
