use std::collections::BTreeSet;

use super::{
    carried, is_core_name, published, publishing, Capability, Constraint, Credential, Probe,
    ProbeKind, Requirement, Unpublishable, VOCABULARY_VERSION,
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
                        || probe.requires.endings().is_empty()
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
    assert!(wrong.is_empty(), "these are not area.verb: {wrong:?}");
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
    let granted: BTreeSet<&str> = lemonfiber_manifest::container::Shape::ALL
        .into_iter()
        .flat_map(|shape| shape.grants().iter().copied())
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
/// no anonymous caller can answer, and a refusal or a redirect to HTTPS is the one
/// thing it can: neither is something a port proxy produces.
#[test]
fn a_probe_requiring_no_body_constraint_is_a_refusal_or_a_redirect() {
    let odd: Vec<&str> = carried()
        .iter()
        .flat_map(|held| held.probes)
        .filter(|probe| probe.kind() == ProbeKind::Request && probe.requires.body().is_empty())
        .filter(|probe| {
            probe
                .requires
                .endings()
                .iter()
                .any(|status| *status < 400 && !REDIRECTS.contains(status))
        })
        .map(|probe| probe.id)
        .collect();
    assert!(
        odd.is_empty(),
        "these constrain only a status and are neither refusals nor redirects: {odd:?}"
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

/// The statuses a redirect to HTTPS is answered with.
const REDIRECTS: [i64; 2] = [301, 308];

/// A probe that runs the claimant image's own program and is answered by its exit and
/// the start of what it prints.
const RUN: Probe = Probe {
    id: "runs",
    title: "Its own program runs and says what it is",
    asks: "a run of the image's own program asking which release it is",
    why: "Nothing reaches it over the network, so its own program is what can be asked.",
    credential: Credential::None,
    requires: Requirement::Run {
        exit: &[0],
        body: &[Constraint::StdoutStartsWith],
    },
};

/// A capability whose fillers listen on nothing, probed by a run.
const QUIET: &[Capability] = &[Capability {
    name: "quiet.work",
    summary: "Works on its own schedule.",
    contract: "Listens on nothing.",
    probes: &[RUN],
    listens: false,
}];

/// The same probe on a capability whose fillers listen for requests.
const LISTENING: &[Capability] = &[Capability {
    name: "loud.work",
    summary: "Answers requests.",
    contract: "Listens for requests.",
    probes: &[RUN],
    listens: true,
}];

/// A stack whose one service declares `name`.
fn declaring(name: &str) -> Vec<lemonfiber_manifest::Service> {
    services(&format!(
        "schema_version = 1\nstack_version = \"1.0.0\"\nmin_cli_version = \"0.4.0\"\n\
         [[profile]]\nid = \"p\"\nname = \"P\"\ndescription = \"P\"\n{}",
        service("one", &[name])
    ))
}

/// A run probe where a capability's fillers listen for requests fails generation, naming
/// the capability and the probe.
#[test]
fn a_run_probe_where_the_fillers_listen_refuses_to_be_published() {
    let refusal = publishing(LISTENING, &declaring("loud.work"))
        .err()
        .unwrap_or_default();
    assert_eq!(
        refusal,
        vec![Unpublishable::RunOfAListener {
            capability: "loud.work",
            probe: "runs",
        }]
    );
    let said: String = refusal.iter().map(ToString::to_string).collect();
    assert!(
        said.contains("loud.work") && said.contains("runs"),
        "names both: {said}"
    );
}

/// A run probe where the fillers listen on nothing is published, and the artefact says
/// it is a run, answered by an exit status and the start of what it prints.
#[test]
fn a_run_probe_where_the_fillers_listen_on_nothing_is_published_as_a_run() {
    let published = publishing(QUIET, &declaring("quiet.work"));
    let probe = published
        .ok()
        .and_then(|published| serde_json::to_value(published.capabilities).ok())
        .and_then(|capabilities| capabilities.pointer("/0/probes/0").cloned());
    assert_eq!(
        probe,
        Some(serde_json::json!({
            "id": "runs",
            "kind": "run",
            "title": RUN.title,
            "asks": RUN.asks,
            "why": RUN.why,
            "credential": "none",
            "requires": { "exit": [0], "body": ["stdout_starts_with"] },
        }))
    );
}

/// A request probe says so in the artefact, and lists the statuses that answer it.
#[test]
fn a_request_probe_is_published_as_a_request_with_its_statuses() {
    let probe = carried()
        .iter()
        .flat_map(|held| held.probes)
        .find(|probe| probe.kind() == ProbeKind::Request)
        .and_then(|probe| serde_json::to_value(probe).ok());
    let kind = probe.as_ref().and_then(|probe| probe.get("kind").cloned());
    let statuses = probe.and_then(|probe| probe.pointer("/requires/status").cloned());
    assert_eq!(kind, Some(serde_json::json!("request")));
    assert!(
        statuses.is_some_and(|statuses| statuses.is_array()),
        "lists its statuses"
    );
}

/// Every probe this generation declares a run belongs to a capability whose fillers
/// listen on nothing, which is the rule generation refuses the artefact by.
#[test]
fn every_carried_run_probe_belongs_to_a_capability_whose_fillers_listen_on_nothing() {
    let odd: Vec<&str> = carried()
        .iter()
        .filter(|held| held.listens)
        .filter(|held| {
            held.probes
                .iter()
                .any(|probe| probe.kind() == ProbeKind::Run)
        })
        .map(|held| held.name)
        .collect();
    assert!(
        odd.is_empty(),
        "these listen and are probed by a run: {odd:?}"
    );
}

/// The two ways a probe is put, each spelled as the artefact and a binding spell it.
#[test]
fn each_kind_of_probe_is_spelled_as_a_binding_declares_it() {
    for kind in [ProbeKind::Request, ProbeKind::Run] {
        assert_eq!(
            serde_json::to_string(&kind).ok(),
            Some(format!("\"{}\"", kind.as_str()))
        );
    }
}

/// Quality sync is the one capability whose fillers listen on nothing, and so the one
/// whose probe may be a run.
#[test]
fn quality_sync_is_the_one_capability_whose_fillers_listen_on_nothing() {
    let quiet: Vec<&str> = carried()
        .iter()
        .filter(|held| !held.listens)
        .map(|held| held.name)
        .collect();
    assert_eq!(quiet, vec!["quality.sync"]);
}
