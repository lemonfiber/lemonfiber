use super::contributing::{present, Held};
use super::violations;
use crate::extension::POINTS;
use crate::vocabulary::carried;
use crate::vocabulary::Constraint;
use crate::vocabulary::Removed;
use crate::Expect;
use crate::Manifest;

/// The identities the doctor's register holds in these tests.
///
/// Two whole names and one family, which is what the real register is a longer
/// version of — the family is the half a set of exact names would let through.
const OCCUPIED: &[&str] = &["storage.space", "environment.engine", "credentials."];

/// A manifest claiming `media.serve` the way the plugin that claims it does.
///
/// Written out rather than derived from the vocabulary, deliberately. A fixture
/// built from the probe requirements would satisfy them by construction and would
/// go on passing through any change to them; this one is a claimant's side of the
/// contract, and it fails if the two stop agreeing — which is the thing worth
/// being told.
const CLAIMANT: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.1.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.26.3"
port        = 25600
bind        = "lan"
criticality = "enhancing"
provides    = ["media.serve", "komga:opds"]

[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }
fixture = "fixtures/guarded.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 200, json_has_keys = ["content", "totalElements"] }
fixture = "fixtures/catalogue.json"

[requires]
capabilities = ["doctor.contribute"]

[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "Komga has an administrator"
category  = "credentials"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200, json = { isClaimed = true } }
fixture   = "fixtures/claim.json"
timeout_s = 10
why       = "An unclaimed Komga hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
id     = "komga:claim-it"
for    = "komga:claimed"
action = "Open Komga and create the administrator account"
why    = "Until somebody does, the first caller on the household network becomes it."
"#;

/// What the rules say about a manifest, as one string per violation.
fn against(text: &str) -> Vec<String> {
    // A fixture this build cannot read is a mistake in the test rather than a
    // refusal, and it has to stop the case rather than come back as an empty list
    // that every "says nothing" assertion below would pass on.
    let read = Manifest::from_toml(text);
    assert!(read.is_ok(), "the fixture does not read: {read:?}");
    read.map(|manifest| violations(&manifest, OCCUPIED))
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// The same, with one line of the claimant replaced — which is how every negative
/// case below is built, so each is exactly one departure from a manifest that holds.
fn changed(from: &str, to: &str) -> Vec<String> {
    assert!(
        CLAIMANT.contains(from),
        "the fixture no longer says: {from}"
    );
    against(&CLAIMANT.replace(from, to))
}

/// Whether every word is somewhere in one of the refusals.
fn says(said: &[String], words: &[&str]) -> bool {
    said.iter()
        .any(|one| words.iter().all(|word| one.contains(word)))
}

#[test]
fn a_claimant_that_binds_what_the_vocabulary_declares_is_refused_nothing() {
    assert_eq!(against(CLAIMANT), Vec::<String>::new());
}

#[test]
fn a_core_name_the_vocabulary_does_not_carry_is_named_with_the_ones_it_does() {
    let said = changed("\"media.serve\", \"komga:opds\"", "\"media.stream\"");
    assert!(
        says(
            &said,
            &["media.stream", "names no capability", "media.serve"]
        ),
        "got: {said:?}"
    );
}

/// A name a published generation carried and this one does not is told which
/// generation it went in, rather than that no such capability exists.
///
/// Asked of the rule directly, because this generation has removed nothing: the
/// published list is empty and will be until a capability is withdrawn, so a test
/// driven through the manifest would be asserting about a branch nothing can reach
/// — and the first removal would be the first time anybody found out whether it
/// worked.
#[test]
fn a_capability_a_later_generation_removed_is_told_when_it_went() {
    let gone = Removed {
        name: "media.stream",
        removed_in: 4,
    };
    let mut found = Vec::new();
    super::core("media.stream", "service s", &[gone], &mut found);
    let said = found.first().map(ToString::to_string).unwrap_or_default();
    assert!(said.contains("media.stream"), "names it: {said}");
    assert!(said.contains("generation 4"), "and when it went: {said}");

    let mut unknown = Vec::new();
    super::core("media.stream", "service s", &[], &mut unknown);
    let never = unknown.first().map(ToString::to_string).unwrap_or_default();
    assert!(
        never.contains("names no capability") && !never.contains("generation"),
        "a name that never existed is a different fact: {never}"
    );
}

/// A second service of the same plugin, declaring what the first one does.
const ALONGSIDE: &str = r#"
[[service]]
id          = "komga-sync"
name        = "Komga's reading history"
image       = "example.invalid/komga-sync"
digest      = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
tag         = "1.0.0"
criticality = "enhancing"
provides    = ["media.serve"]

"#;

/// Something asks for a core capability by name and exactly one service answers it,
/// so two services of one plugin declaring it is not a choice an operator could make.
///
/// Decided here rather than contested on their machine. A plugin is one install and
/// both halves arrive together, so there is no moment at which anybody is asked
/// which of the two to wire — and nothing downstream would say which one was.
#[test]
fn a_core_capability_two_of_one_plugins_services_declare_is_refused_naming_the_first() {
    let said = against(&CLAIMANT.replace("[[claim]]", &format!("{ALONGSIDE}[[claim]]")));
    assert!(
        says(
            &said,
            &[
                "media.serve is a core capability and komga already declares it",
                "two answers to one question",
            ]
        ),
        "got: {said:?}"
    );
}

/// Two claims for one capability are two answers to one question, and nothing here
/// would say which of them was read.
#[test]
fn a_capability_claimed_twice_is_refused() {
    let twice = CLAIMANT.replace(
        "[requires]",
        "[[claim]]\ncapability = \"media.serve\"\n\n[requires]",
    );
    let said = against(&twice);
    assert!(
        says(&said, &["media.serve is claimed twice"]),
        "got: {said:?}"
    );
}

/// The same, one level down: two bindings for one probe.
#[test]
fn a_probe_bound_twice_is_refused() {
    let twice = CLAIMANT.replace(
        "[[claim.probe]]\nid      = \"catalogue\"",
        "[[claim.probe]]\nid      = \"guarded\"\nrequest = { method = \"GET\", path = \"/x\" }\n\
             expect  = { status = 401 }\nfixture = \"fixtures/x.json\"\n\n\
             [[claim.probe]]\nid      = \"catalogue\"",
    );
    let said = against(&twice);
    assert!(says(&said, &["guarded is bound twice"]), "got: {said:?}");
}

/// A binding that says nothing about the status has not said the probe was answered
/// — and the refusal names what an answer would have been.
#[test]
fn a_binding_that_declares_no_status_is_refused_naming_what_answers_the_probe() {
    let said = changed(
        "expect  = { status = 401 }",
        "expect  = { json_has_keys = [] }",
    );
    assert!(
        says(&said, &["expects no status", "401, 403"]),
        "got: {said:?}"
    );
}

/// Two rows holding one identity is a register with two answers under one name.
#[test]
fn a_contributed_identity_declared_twice_is_refused() {
    let again = r#"
[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "The same identity, a second time"
category  = "services"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200, json = { isClaimed = true } }
fixture   = "fixtures/claim.json"
why       = "Two rows, one name."
"#;
    let said = against(&format!("{CLAIMANT}{again}"));
    assert!(
        says(&said, &["komga:claimed is declared twice"]),
        "got: {said:?}"
    );
}

#[test]
fn a_capability_namespaced_with_somebody_else_s_id_is_refused() {
    let said = changed("\"komga:opds\"", "\"plex:opds\"");
    assert!(
        says(&said, &["plex:opds", "namespaced with this plugin's id"]),
        "got: {said:?}"
    );
}

#[test]
fn a_name_that_is_neither_shape_is_refused_as_neither() {
    let said = changed("\"komga:opds\"", "\"OPDS\"");
    assert!(
        says(&said, &["OPDS", "neither a core name"]),
        "got: {said:?}"
    );
}

/// A core name asserted and not demonstrated is the posture the vocabulary exists
/// to refuse, so it is named on its own rather than left to the probe rules.
#[test]
fn a_core_name_with_no_claim_block_is_refused_as_an_assertion() {
    let (before, rest) = CLAIMANT.split_once("[[claim]]").unwrap_or_default();
    let (_, after) = rest.split_once("[requires]").unwrap_or_default();
    let said = against(&format!("{before}[requires]{after}"));
    assert!(
        says(&said, &["media.serve", "demonstrated, not asserted"]),
        "got: {said:?}"
    );
}

#[test]
fn a_claim_for_a_capability_no_service_declares_names_both_halves() {
    let said = changed("\"media.serve\", \"komga:opds\"", "\"komga:opds\"");
    assert!(
        says(&said, &["media.serve", "in no service's `provides`"]),
        "got: {said:?}"
    );
}

/// A namespaced capability has no published contract, so a claim block for one is
/// evidence for a thing nobody wrote down.
#[test]
fn a_claim_block_for_a_namespaced_capability_is_refused() {
    let said = changed(
        "capability = \"media.serve\"",
        "capability = \"komga:opds\"",
    );
    assert!(
        says(&said, &["komga:opds", "no published contract"]),
        "got: {said:?}"
    );
}

#[test]
fn a_probe_the_capability_declares_and_the_claim_does_not_bind_is_named() {
    let said = changed("id      = \"guarded\"", "id      = \"catalogue\"");
    assert!(
        says(&said, &["binds no probe guarded", "it declares"]),
        "got: {said:?}"
    );
}

#[test]
fn a_probe_the_capability_does_not_declare_is_named_with_the_ones_it_does() {
    let said = changed("id      = \"guarded\"", "id      = \"invented\"");
    assert!(
        says(&said, &["invented", "not a probe", "guarded"]),
        "got: {said:?}"
    );
}

#[test]
fn an_expectation_the_probe_does_not_answer_with_is_refused_naming_what_it_does() {
    let said = changed("expect  = { status = 401 }", "expect  = { status = 200 }");
    assert!(
        says(&said, &["expects status 200", "401, 403"]),
        "got: {said:?}"
    );
}

#[test]
fn a_binding_that_says_nothing_about_a_body_the_probe_needs_is_refused() {
    let said = changed(
        "expect  = { status = 200, json_has_keys = [\"content\", \"totalElements\"] }",
        "expect  = { status = 200 }",
    );
    assert!(
        says(&said, &["constrains no body", "json_has_keys"]),
        "got: {said:?}"
    );
}

/// A key that reads as a constraint and asserts nothing is not evidence.
#[test]
fn a_body_constraint_that_evaluates_to_nothing_does_not_answer_for_one() {
    let said = changed(
        "expect  = { status = 200, json_has_keys = [\"content\", \"totalElements\"] }",
        "expect  = { status = 200, json_is_absent = false }",
    );
    assert!(says(&said, &["constrains no body"]), "got: {said:?}");
}

#[test]
fn a_probe_asking_for_a_count_above_zero_is_refused_on_both_keys() {
    let list = changed(
        "json_has_keys = [\"content\", \"totalElements\"]",
        "json_array_min = 1",
    );
    assert!(
        says(&list, &["json_array_min", "gates an install"]),
        "got: {list:?}"
    );
    let counted = changed(
        "json_has_keys = [\"content\", \"totalElements\"]",
        "json_at_least = { totalElements = 1 }",
    );
    assert!(
        says(&counted, &["totalElements at least 1", "gates an install"]),
        "got: {counted:?}"
    );
}

/// Every kind of body constraint is one this can recognise in an expectation.
///
/// Asked of each in turn rather than through the vocabulary, because the capabilities
/// this generation carries ask for five of the nine — and the other four would be a
/// rule that reads as written and decides nothing the day a capability asks for one.
#[test]
fn every_kind_of_body_constraint_is_one_an_expectation_can_carry() {
    let each = [
        (Constraint::Status, "status = 200"),
        (Constraint::Json, "json = { a = 1 }"),
        (Constraint::JsonHasKeys, "json_has_keys = [\"a\"]"),
        (Constraint::JsonTypes, "json_types = { a = \"int\" }"),
        (Constraint::JsonAtLeast, "json_at_least = { a = 0 }"),
        (Constraint::JsonArrayMin, "json_array_min = 0"),
        (Constraint::JsonIsAbsent, "json_is_absent = true"),
        (
            Constraint::ContentType,
            "content_type = \"application/json\"",
        ),
        (Constraint::BodyStartsWith, "body_starts_with = \"<\""),
    ];
    let empty = toml::from_str::<Expect>("");
    for (constraint, declared) in each {
        assert!(
            toml::from_str::<Expect>(declared)
                .is_ok_and(|expect| super::carries(&expect, constraint)),
            "{declared} does not read as {constraint:?}"
        );
        assert!(
            empty
                .as_ref()
                .is_ok_and(|nothing| !super::carries(nothing, constraint)),
            "an expectation that says nothing reads as {constraint:?}"
        );
    }
}

/// The form that is present and asserts nothing, which is the gap this test
/// had: it held the populated form and the wholly-absent one, and every
/// constraint that can be written empty sat between them.
///
/// `json_array_min = 0` is not here, and that is the one real difference:
/// the shape check behind it still requires the body to parse as an array.
#[test]
fn a_constraint_written_empty_constrains_nothing_and_does_not_count() {
    let each = [
        (Constraint::Json, "json = {}"),
        (Constraint::JsonHasKeys, "json_has_keys = []"),
        (Constraint::JsonTypes, "json_types = {}"),
        (Constraint::JsonAtLeast, "json_at_least = {}"),
        (Constraint::ContentType, "content_type = \"\""),
        (Constraint::BodyStartsWith, "body_starts_with = \"\""),
    ];
    for (constraint, declared) in each {
        assert!(
            toml::from_str::<Expect>(declared)
                .is_ok_and(|expect| !super::carries(&expect, constraint)),
            "{declared} reads as {constraint:?}, so a claim could be demonstrated \
                 on the evidence that something answered at all"
        );
    }
}

/// A count of zero says the answer reads as a list, or carries a number at a key,
/// which is about shape. Both keys, because the rule is written twice and a pass
/// demonstrated on one of them says nothing about the other.
#[test]
fn a_count_of_zero_is_a_shape_and_is_allowed() {
    let list = changed(
        "json_has_keys = [\"content\", \"totalElements\"]",
        "json_array_min = 0",
    );
    assert_eq!(list, Vec::<String>::new());
    let counted = changed(
        "json_has_keys = [\"content\", \"totalElements\"]",
        "json_at_least = { totalElements = 0 }",
    );
    assert_eq!(counted, Vec::<String>::new());
}

/// A plugin that adds nothing to lemonfiber's own registers is asked nothing about
/// them — including whether it asked for the capability that taking one requires.
#[test]
fn a_plugin_that_contributes_nothing_is_asked_nothing_about_contributions() {
    let (before, _) = CLAIMANT.split_once("[[contribution]]").unwrap_or_default();
    assert_eq!(
        against(&before.replace("[\"doctor.contribute\"]", "[]")),
        Vec::<String>::new()
    );
}

#[test]
fn a_contribution_at_a_point_this_build_does_not_publish_names_the_ones_it_does() {
    let said = changed(
        "at        = \"doctor.check\"",
        "at        = \"doctor.rewrite\"",
    );
    assert!(
        says(
            &said,
            &["doctor.rewrite", "no extension point", "doctor.check"]
        ),
        "got: {said:?}"
    );
}

#[test]
fn a_contributed_identity_that_is_not_the_plugin_s_own_is_refused() {
    let said = changed(
        "id        = \"komga:claimed\"",
        "id        = \"storage.space\"",
    );
    assert!(
        says(
            &said,
            &["storage.space", "namespaced with this plugin's id"]
        ),
        "got: {said:?}"
    );
}

/// The belt to the namespacing braces: an identity a bundled row holds is refused
/// on its own account, so the rule does not rest on two conventions staying apart.
#[test]
fn an_identity_a_bundled_row_holds_is_refused_naming_it() {
    let said = changed(
        "id        = \"komga:claimed\"",
        "id        = \"environment.engine\"",
    );
    assert!(
        says(&said, &["environment.engine", "bundled row already holds"]),
        "got: {said:?}"
    );
}

/// A bundled name ending in a dot is a family, and everything under it is taken.
#[test]
fn an_identity_inside_a_bundled_family_is_refused_naming_the_family() {
    let said = changed(
        "for    = \"komga:claimed\"",
        "for    = \"credentials.indexer\"",
    );
    assert!(
        says(&said, &["credentials.indexer", "credentials."]),
        "got: {said:?}"
    );
}

#[test]
fn a_remedy_for_a_check_this_manifest_does_not_declare_is_refused() {
    let said = changed(
        "for    = \"komga:claimed\"",
        "for    = \"komga:something-else\"",
    );
    assert!(
        says(
            &said,
            &[
                "komga:something-else",
                "names no check this manifest declares"
            ]
        ),
        "got: {said:?}"
    );
}

#[test]
fn a_check_with_no_remedy_is_refused() {
    let said =
        against(&CLAIMANT.replace("at     = \"doctor.remedy\"", "at     = \"doctor.check\""));
    assert!(says(&said, &["carries no remedy"]), "got: {said:?}");
}

#[test]
fn a_row_missing_what_its_point_requires_is_refused_naming_the_field() {
    let said = against(&CLAIMANT.replace("category  = \"credentials\"\n", ""));
    assert!(
        says(&said, &["carries no category", "doctor.check requires"]),
        "got: {said:?}"
    );
}

#[test]
fn a_row_carrying_a_field_its_point_does_not_declare_is_refused_with_what_it_takes() {
    let said = changed(
        "action = \"Open Komga and create the administrator account\"",
        "action = \"Open Komga\"\ncategory = \"services\"",
    );
    assert!(
        says(
            &said,
            &["category is outside what doctor.remedy declares", "action"]
        ),
        "got: {said:?}"
    );
}

#[test]
fn a_family_outside_the_closed_set_its_point_declares_is_refused() {
    let said = changed("category  = \"credentials\"", "category  = \"comics\"");
    assert!(
        says(&said, &["comics is not one category recognises"]),
        "got: {said:?}"
    );
}

#[test]
fn a_check_bounded_beyond_what_its_point_permits_is_refused_naming_the_bounds() {
    let said = changed("timeout_s = 10", "timeout_s = 90");
    assert!(
        says(&said, &["90 is outside the bounds", "1 to 30"]),
        "got: {said:?}"
    );
}

/// Contributing is a capability a manifest asks for by name, so a build that does
/// not take rows there refuses the manifest rather than reading one and dropping it.
#[test]
fn a_contribution_whose_capability_the_manifest_never_asked_for_is_refused_by_name() {
    let said = changed(
        "capabilities = [\"doctor.contribute\"]",
        "capabilities = []",
    );
    assert!(
        says(&said, &["doctor.contribute", "asks for"]),
        "got: {said:?}"
    );
}

/// The table of fields is the one list in this module, and a list goes stale.
///
/// Every field the published points say a row may carry has to be readable back
/// off a contribution, or a row could carry it into a register with nothing
/// deciding whether that point declares it.
#[test]
fn every_field_a_published_point_declares_is_one_this_can_read_back() {
    let whole = r#"
at        = "doctor.check"
id        = "p:one"
title     = "t"
category  = "services"
request   = { method = "GET", path = "/" }
expect    = { status = 200 }
why       = "w"
fixture   = "f.json"
fires_on  = "g.json"
timeout_s = 10
service   = "s"
for       = "p:two"
action    = "a"
detail    = "d"
"#;
    let readable = toml::from_str::<crate::Contribution>(whole)
        .map(|entry| present(&entry))
        .unwrap_or_default();
    let declared: std::collections::BTreeSet<&str> = POINTS
        .iter()
        .flat_map(|point| point.row.required.iter().chain(point.row.optional))
        .copied()
        .collect();
    let read: std::collections::BTreeSet<&str> = readable.keys().copied().collect();
    assert_eq!(
        read, declared,
        "the fields a row may carry and the fields this reads back have parted"
    );
}

/// A bound is on a number and a closed set is on a word, and reading one as the
/// other would quietly stop deciding.
#[test]
fn a_bounded_field_reads_back_as_a_number_and_a_closed_one_as_a_word() {
    let readable = toml::from_str::<crate::Contribution>(
        "at = \"doctor.check\"\nid = \"p:one\"\ncategory = \"services\"\ntimeout_s = 7\n",
    )
    .map(|entry| present(&entry))
    .unwrap_or_default();
    assert_eq!(readable.get("timeout_s"), Some(&Held::Number(7)));
    assert_eq!(
        readable.get("category"),
        Some(&Held::Word("services".to_owned()))
    );
}

/// Every capability the vocabulary carries can be claimed by something.
///
/// A contract whose probes no binding could satisfy — two probes wanting the same
/// id, a body requirement naming a constraint an expectation cannot express — would
/// be a name published and unclaimable, and nobody would find out until an author
/// tried.
#[test]
fn every_published_capability_is_one_a_binding_could_satisfy() {
    let unclaimable: Vec<&str> = carried()
        .iter()
        .filter(|held| {
            let ids: std::collections::BTreeSet<&str> =
                held.probes.iter().map(|probe| probe.id).collect();
            ids.len() != held.probes.len()
                || held
                    .probes
                    .iter()
                    .any(|probe| probe.requires.status.is_empty())
        })
        .map(|held| held.name)
        .collect();
    assert!(
        unclaimable.is_empty(),
        "nothing could claim: {unclaimable:?}"
    );
}
