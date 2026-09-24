use super::{
    Bind, Contribution, Criticality, Expected, ExpectedKind, HealthKind, Manifest, Service,
};
use crate::Error;

/// A manifest declaring every block the contract carries.
///
/// Whole rather than minimal, because what these tests are for is that the types
/// mirror the contract field for field — and a fixture carrying only the required
/// half would leave the optional half describing nothing.
pub(crate) const WHOLE: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://github.com/gotson/komga"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
health      = { kind = "http", path = "/actuator/health", timeout_s = 90 }
criticality = "important"
media_types = ["comics"]
takes_data  = true
provides    = ["media.serve", "komga:kobo-sync"]
config_path = "/config"

[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }
fixture = "fixtures/media-serve-guarded.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 200, json_has_keys = ["content"], json_types = { content = "list" }, json_at_least = { totalElements = 1 }, json_array_min = 1, json_is_absent = false, content_type = "application/json", body_starts_with = "{" }
fixture = "fixtures/media-serve-catalogue.json"

[[wiring]]
hostname        = "comics"
dashboard_group = "Library"

[[proof]]
id      = "komga.serves"
title   = "Komga answers on its declared health path"
request = { method = "GET", path = "/actuator/health" }
expect  = { status = 200, json = { status = "UP", claimed = true, libraries = 3 } }
fixture = "fixtures/health.json"
why     = "The path the health probe asks for is one this image serves."

[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "Komga has an administrator, so nobody else can become one"
category  = "services"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200, json = { isClaimed = true } }
fixture   = "fixtures/api-v1-claim-claimed.json"
timeout_s = 10
service   = "komga"
why       = "An unclaimed Komga hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
id     = "komga:claim-it"
for    = "komga:claimed"
action = "Open Komga and create the administrator account"
detail = "POST /api/v1/claim with an email and a password creates it."
why    = "Until somebody does, the first caller on the household network becomes it."

[[recipe]]
id    = "adopt-existing-library"
title = "Point it at the comics the stack already files"
why   = "The stack already files comics, and a fresh Komga knows nothing about them."

[[recipe.step]]
id      = "sign-in"
call    = { method = "POST", to = "komga", path = "/api/v1/login", body = "{\"password\": \"read from the operator's store\"}" }
expect  = { status = 200 }
capture = [{ name = "token", from = "json.token", origin = "stack-service" }]

[[recipe.step]]
id      = "create"
call    = { method = "POST", to = "komga", path = "/api/v1/libraries", headers = { Authorization = "Bearer {{token}}" }, body = "{\"name\": \"Comics\"}" }
expect  = { status = 200 }
capture = [{ name = "library", from = "json.id", origin = "stack-service" }]

[[recipe.pair]]
value = "token"
to    = "komga"

[[secret]]
id  = "api-key"
of  = "komga"
why = "Read the library counts the dashboard panel shows"

[[override]]
id  = "homepage.services"
why = "Add its own entry to the bundled dashboard"

[requires]
capabilities = ["doctor.contribute", "recipe.run"]
"#;

/// Parsed, or absent.
///
/// Assertions go through combinators rather than destructuring, because a
/// `let … else` needs an arm for the case that cannot happen, and that arm is a
/// line no test can ever reach.
fn parse(text: &str) -> Option<Manifest> {
    Manifest::from_toml(text).ok()
}

#[test]
fn reads_every_block_the_contract_declares() {
    let counted = parse(WHOLE).map(|manifest| {
        (
            manifest.services.len(),
            manifest.claims.len(),
            manifest.proofs.len(),
            manifest.contributions.len(),
            manifest.recipes.len(),
            manifest.secrets.len(),
            manifest.overrides.len(),
        )
    });
    assert_eq!(counted, Some((1, 1, 1, 2, 1, 1, 1)));
}

#[test]
fn reads_who_the_plugin_is_and_where_it_came_from() {
    let read = parse(WHOLE).map(|manifest| {
        (
            manifest.plugin.id,
            manifest.plugin.name,
            manifest.plugin.version,
            manifest.plugin.description,
            manifest.plugin.without_it,
            manifest.plugin.upstream,
            manifest.plugin.license,
            manifest.plugin.forms,
        )
    });
    assert_eq!(
        read,
        Some((
            "komga".to_owned(),
            "Komga".to_owned(),
            "1.2.0".to_owned(),
            "Reads your comics on any browser".to_owned(),
            "Files on disk, no way to read them".to_owned(),
            "https://github.com/gotson/komga".to_owned(),
            "MIT".to_owned(),
            vec!["library".to_owned()],
        ))
    );
}

#[test]
fn reads_what_the_plugin_runs() {
    let read = parse(WHOLE)
        .and_then(|manifest| manifest.services.into_iter().next())
        .map(|service| {
            (
                service.id,
                service.image,
                service.digest.starts_with("sha256:"),
                service.tag,
                service.port,
                service.bind,
                service.criticality,
                service.media_types,
                service.takes_data,
                service.provides,
                service.config_path,
            )
        });
    assert_eq!(
        read,
        Some((
            "komga".to_owned(),
            "docker.io/gotson/komga".to_owned(),
            true,
            "1.11.0".to_owned(),
            Some(25600),
            Some(Bind::Lan),
            Criticality::Important,
            vec!["comics".to_owned()],
            true,
            vec!["media.serve".to_owned(), "komga:kobo-sync".to_owned()],
            Some("/config".to_owned()),
        ))
    );
}

#[test]
fn reads_the_health_probe_a_plugin_declares() {
    let probe = parse(WHOLE)
        .and_then(|manifest| manifest.services.into_iter().next())
        .and_then(|service| service.health)
        .map(|health| (health.kind, health.path, health.timeout_s));
    assert_eq!(
        probe,
        Some((
            HealthKind::Http,
            Some("/actuator/health".to_owned()),
            Some(90)
        ))
    );
}

/// A claim names a capability and binds each probe to a request of its own.
///
/// Read back whole rather than counted, because the count is the half that would
/// still pass if every field arrived empty.
#[test]
fn a_claim_binds_each_probe_to_a_request_and_a_recording() {
    let bound = parse(WHOLE)
        .and_then(|manifest| manifest.claims.into_iter().next())
        .map(|claim| {
            let bindings: Vec<(String, String, String, Option<u16>, String)> = claim
                .probes
                .into_iter()
                .map(|probe| {
                    (
                        probe.id,
                        probe.request.method,
                        probe.request.path,
                        probe.expect.status,
                        probe.fixture,
                    )
                })
                .collect();
            (claim.capability, bindings)
        });
    assert_eq!(
        bound,
        Some((
            "media.serve".to_owned(),
            vec![
                (
                    "guarded".to_owned(),
                    "GET".to_owned(),
                    "/api/v1/series".to_owned(),
                    Some(401),
                    "fixtures/media-serve-guarded.json".to_owned(),
                ),
                (
                    "catalogue".to_owned(),
                    "GET".to_owned(),
                    "/api/v1/series".to_owned(),
                    Some(200),
                    "fixtures/media-serve-catalogue.json".to_owned(),
                ),
            ]
        ))
    );
}

/// Every kind of body constraint the shared vocabulary has, read off one binding.
///
/// Together rather than one test each, because what is being checked is that the
/// vocabulary is *whole* — a kind the type is missing reads exactly like a kind
/// the manifest did not declare.
#[test]
fn every_kind_of_body_constraint_is_read() {
    let read = parse(WHOLE)
        .and_then(|manifest| manifest.claims.into_iter().next())
        .and_then(|claim| claim.probes.into_iter().nth(1))
        .map(|probe| probe.expect);
    let expected = read.map(|expect| {
        (
            expect.json_has_keys,
            expect
                .json_types
                .and_then(|types| types.get("content").copied()),
            expect
                .json_at_least
                .and_then(|least| least.get("totalElements").copied()),
            expect.json_array_min,
            expect.json_is_absent,
            expect.content_type,
            expect.body_starts_with,
        )
    });
    assert_eq!(
        expected,
        Some((
            Some(vec!["content".to_owned()]),
            Some(ExpectedKind::List),
            Some(1),
            Some(1),
            Some(false),
            Some("application/json".to_owned()),
            Some("{".to_owned()),
        ))
    );
}

/// The three things an exact value can be, read off one proof.
#[test]
fn an_exact_value_is_a_word_a_flag_or_a_number() {
    let held = parse(WHOLE)
        .and_then(|manifest| manifest.proofs.into_iter().next())
        .and_then(|proof| proof.expect.json)
        .map(|json| {
            (
                json.get("status").cloned(),
                json.get("claimed").cloned(),
                json.get("libraries").cloned(),
            )
        });
    assert_eq!(
        held,
        Some((
            Some(Expected::Word("UP".to_owned())),
            Some(Expected::Flag(true)),
            Some(Expected::Number(3)),
        ))
    );
}

#[test]
fn reads_what_a_proof_asks_and_why_it_is_worth_asking() {
    let read = parse(WHOLE)
        .and_then(|manifest| manifest.proofs.into_iter().next())
        .map(|proof| {
            (
                proof.id,
                proof.title,
                proof.request.method,
                proof.fixture,
                proof.why.contains("health probe"),
            )
        });
    assert_eq!(
        read,
        Some((
            "komga.serves".to_owned(),
            "Komga answers on its declared health path".to_owned(),
            "GET".to_owned(),
            Some("fixtures/health.json".to_owned()),
            true,
        ))
    );
}

/// Which fields one row of a contribution filled in.
///
/// Said as a list of names rather than as a tuple of options, because what the two
/// rows differ in is *which* of the union they use — and a reader comparing two
/// columns of `None` against two columns of `Some` is doing that translation by eye.
fn filled(row: &Contribution) -> Vec<&'static str> {
    [
        ("title", row.title.is_some()),
        ("category", row.category.is_some()),
        ("request", row.request.is_some()),
        ("expect", row.expect.is_some()),
        ("why", row.why.is_some()),
        ("fixture", row.fixture.is_some()),
        ("timeout_s", row.timeout_s.is_some()),
        ("service", row.service.is_some()),
        ("for", row.about.is_some()),
        ("action", row.action.is_some()),
        ("detail", row.detail.is_some()),
    ]
    .into_iter()
    .filter_map(|(field, given)| given.then_some(field))
    .collect()
}

/// A contributed check and the remedy that answers it are one block, twice.
///
/// The row belongs to the point rather than to this type, so the two shapes share
/// a table and are told apart by `at` — which is what keeps the required set
/// published in one place instead of restated in two.
#[test]
fn a_contribution_carries_the_row_its_point_declares() {
    let rows: Vec<Contribution> = parse(WHOLE)
        .map(|manifest| manifest.contributions)
        .unwrap_or_default();
    let named: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| (row.at.as_str(), row.id.as_str()))
        .collect();
    assert_eq!(
        named,
        vec![
            ("doctor.check", "komga:claimed"),
            ("doctor.remedy", "komga:claim-it"),
        ]
    );

    let check = rows.first().map(filled);
    assert_eq!(
        check,
        Some(vec![
            "title",
            "category",
            "request",
            "expect",
            "why",
            "fixture",
            "timeout_s",
            "service",
        ])
    );

    let remedy = rows.get(1).map(filled);
    assert_eq!(remedy, Some(vec!["why", "for", "action", "detail"]));
    assert_eq!(
        rows.get(1).and_then(|row| row.about.clone()),
        Some("komga:claimed".to_owned())
    );
}

#[test]
fn a_recipe_declares_its_calls_its_captures_and_where_each_value_may_go() {
    let read = parse(WHOLE)
        .and_then(|manifest| manifest.recipes.into_iter().next())
        .map(|recipe| {
            let step = recipe.steps.into_iter().find(|step| step.id == "create");
            let pair = recipe.pairs.into_iter().next();
            (
                recipe.id,
                recipe.title,
                recipe.why.is_empty(),
                step.map(|step| {
                    let capture = step.capture.into_iter().next();
                    (
                        step.id,
                        step.call.method,
                        step.call.to,
                        step.call.path,
                        step.call
                            .headers
                            .and_then(|carried| carried.get("Authorization").cloned()),
                        step.call.body.is_some(),
                        step.expect.and_then(|expect| expect.status),
                        capture.map(|one| (one.name, one.from, one.origin)),
                    )
                }),
                pair.map(|pair| (pair.value, pair.to)),
            )
        });
    assert_eq!(
        read,
        Some((
            "adopt-existing-library".to_owned(),
            "Point it at the comics the stack already files".to_owned(),
            false,
            Some((
                "create".to_owned(),
                "POST".to_owned(),
                "komga".to_owned(),
                "/api/v1/libraries".to_owned(),
                Some("Bearer {{token}}".to_owned()),
                true,
                Some(200),
                Some((
                    "library".to_owned(),
                    "json.id".to_owned(),
                    "stack-service".to_owned()
                )),
            )),
            Some(("token".to_owned(), "komga".to_owned())),
        ))
    );
}

#[test]
fn reads_what_it_will_hold_what_it_will_change_and_what_it_needs() {
    let read = parse(WHOLE).map(|manifest| {
        (
            manifest
                .secrets
                .into_iter()
                .next()
                .map(|secret| (secret.id, secret.of, secret.why.is_empty())),
            manifest
                .overrides
                .into_iter()
                .next()
                .map(|changed| (changed.id, changed.why.is_empty())),
            manifest.requires.map(|requires| requires.capabilities),
        )
    });
    assert_eq!(
        read,
        Some((
            Some(("api-key".to_owned(), "komga".to_owned(), false)),
            Some(("homepage.services".to_owned(), false)),
            Some(vec![
                "doctor.contribute".to_owned(),
                "recipe.run".to_owned()
            ]),
        ))
    );
}

#[test]
fn reads_how_the_stack_is_told_to_reach_it() {
    let read = parse(WHOLE)
        .map(|manifest| manifest.wirings)
        .and_then(|wirings| wirings.into_iter().next())
        .map(|wiring| (wiring.service, wiring.hostname, wiring.dashboard_group));
    assert_eq!(
        read,
        Some((None, Some("comics".to_owned()), Some("Library".to_owned())))
    );
}

/// A wiring naming its service, which is how a plugin with two says which is which.
#[test]
fn a_wiring_says_which_service_it_is_about() {
    let read = parse(&WHOLE.replace("[[wiring]]", "[[wiring]]\nservice = \"komga\""))
        .map(|manifest| manifest.wirings)
        .and_then(|wirings| wirings.into_iter().next())
        .map(|wiring| wiring.service);
    assert_eq!(read, Some(Some("komga".to_owned())));
}

/// The optional half is optional, and reads as absent rather than as a fault.
#[test]
fn a_plugin_that_declares_only_what_it_must_still_reads() {
    let bare = r#"
schema_version = 1

[plugin]
id          = "tiny"
name        = "Tiny"
version     = "0.1.0"
description = "Does one thing"
without_it  = "That thing is not done"
upstream    = "https://example.invalid/tiny"
license     = "MIT"
forms       = ["library"]
"#;
    let read = parse(bare).map(|manifest| {
        (
            manifest.services.len(),
            !manifest.wirings.is_empty(),
            manifest.requires.is_some(),
            manifest.recipes.len(),
        )
    });
    assert_eq!(read, Some((0, false, false, 0)));
}

/// A service that answers its application shell for every unimplemented path.
///
/// The shape two of the published plugins already carry, and the reason
/// `json_is_absent` is a flag rather than a list of keys: what has to be said is
/// that the body was not a document at all. A status alone proves nothing against
/// such a service, because the shell comes back `200` whether the API behind it
/// exists or not.
#[test]
fn a_proof_can_say_the_answer_was_not_json_at_all() {
    let text = WHOLE.replace(
            r#"expect  = { status = 200, json = { status = "UP", claimed = true, libraries = 3 } }"#,
            r#"expect  = { status = 200, content_type = "text/html", json_is_absent = true, body_starts_with = "<!DOCTYPE html>" }"#,
        );
    let read = parse(&text)
        .and_then(|manifest| manifest.proofs.into_iter().next())
        .map(|proof| {
            (
                proof.expect.json_is_absent,
                proof.expect.content_type,
                proof.expect.body_starts_with,
                proof.expect.json.is_none(),
            )
        });
    assert_eq!(
        read,
        Some((
            Some(true),
            Some("text/html".to_owned()),
            Some("<!DOCTYPE html>".to_owned()),
            true,
        ))
    );
}

#[test]
fn refuses_a_schema_generation_it_cannot_read() {
    let text = WHOLE.replace("schema_version = 1", "schema_version = 99");
    let refusal = Manifest::from_toml(&text).err().map(|err| err.to_string());
    assert_eq!(
        refusal.as_deref(),
        Some("the plugin manifest declares schema version 99, and this build reads [1]"),
        "the refusal names the version found and the versions supported"
    );
}

#[test]
fn a_newer_generation_is_named_as_such_even_when_it_carries_unknown_fields() {
    let text = format!(
        "{}\nfield_from_the_future = true\n",
        WHOLE.replace("schema_version = 1", "schema_version = 99")
    );
    assert!(matches!(
        Manifest::from_toml(&text),
        Err(Error::UnsupportedSchema { found: 99, .. })
    ));
}

/// A field nobody declared is refused rather than skipped.
///
/// The declaration is the entire basis for saying what a plugin may do, so a
/// tolerated unknown is a plugin whose stated behaviour is narrower than its
/// actual one.
#[test]
fn refuses_a_field_it_does_not_know() {
    let text = WHOLE.replace("takes_data  = true", "takes_data = true\nprivileged = true");
    let refusal = Manifest::from_toml(&text)
        .err()
        .map(|refused| refused.to_string())
        .unwrap_or_default();
    assert!(
        refusal.contains("service komga.privileged"),
        "names the field and the entry it was declared on: {refusal}"
    );
    assert!(
        refusal.contains("config_path"),
        "and lists what may be declared instead: {refusal}"
    );
}

/// The names it does not know are reported together, not one per run.
#[test]
fn names_every_unrecognised_declaration_in_one_pass() {
    let text = WHOLE
        .replace(r#"bind        = "lan""#, r#"bind        = "wan""#)
        .replace(
            r#"criticality = "important""#,
            r#"criticality = "critical""#,
        );
    let refusal = Manifest::from_toml(&text)
        .err()
        .map(|refused| refused.to_string())
        .unwrap_or_default();
    assert!(refusal.contains("`wan`"), "names the first: {refusal}");
    assert!(
        refusal.contains("`critical`"),
        "names the second too: {refusal}"
    );
}

#[test]
fn a_file_that_is_not_a_manifest_at_all_is_a_syntax_error() {
    assert!(matches!(
        Manifest::from_toml("= not toml"),
        Err(Error::Syntax(_))
    ));
}

/// The whole fixture, for the two defaults below.
fn whole() -> Option<Manifest> {
    Manifest::from_toml(WHOLE).ok()
}

#[test]
fn a_service_says_where_its_configuration_directory_goes_whether_or_not_it_declared_one() {
    let manifest = whole();
    let service = manifest.as_ref().and_then(|one| one.services.first());
    assert_eq!(service.map(Service::configuration), Some("/config"));

    let mut bare = service.cloned();
    if let Some(bare) = bare.as_mut() {
        bare.config_path = None;
    }
    assert_eq!(
        bare.as_ref().map(Service::configuration),
        Some(super::CONFIGURATION)
    );
}

/// The label and the group the manifest declared, read as one answer so no
/// caller has to take the two defaults for itself.
#[test]
fn where_a_services_own_entry_goes_is_what_the_manifest_declared() {
    let manifest = whole();
    let entry = manifest
        .as_ref()
        .and_then(|one| one.services.first().map(|service| one.entry(service)));
    assert_eq!(entry.map(|entry| entry.hostname), Some("comics"));
    assert_eq!(entry.and_then(|entry| entry.group), Some("Library"));
}

/// A second service of the same plugin, which is what makes the stanzas ambiguous.
const ALONGSIDE: &str = r#"[[service]]
id          = "komga-sync"
name        = "Komga's reading history"
image       = "docker.io/gotson/komga-sync"
digest      = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
tag         = "1.0.0"
criticality = "enhancing"
"#;

/// One stanza for each of them, each saying which it is about.
const WIRED: &str = r#"[[wiring]]
service         = "komga"
hostname        = "comics"
dashboard_group = "Library"

[[wiring]]
service         = "komga-sync"
hostname        = "history"
"#;

/// A stanza that names a service is the one read for that service.
///
/// The field exists precisely so a plugin declaring two services cannot leave which
/// one the household reaches to be inferred — and every fixture here wrote the
/// unnamed stanza, which is the form a plugin with a single service may use. So the
/// half of the reading that tells one service's stanza from another's had never
/// decided anything, and a manifest with two would have taken whichever came first.
#[test]
fn a_wiring_naming_a_service_is_read_for_that_one_and_not_for_the_other() {
    let two = WHOLE.replace(
        "[[wiring]]\nhostname        = \"comics\"\ndashboard_group = \"Library\"\n",
        &format!("{ALONGSIDE}\n{WIRED}"),
    );
    let read = parse(&two).map(|manifest| {
        manifest
            .services
            .iter()
            .map(|service| manifest.entry(service))
            .map(|entry| (entry.hostname.to_owned(), entry.group.map(str::to_owned)))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        read,
        Some(vec![
            ("comics".to_owned(), Some("Library".to_owned())),
            ("history".to_owned(), None),
        ])
    );
}

/// A label is a fact about one container, so the fallback is the service's own
/// id: taking the plugin's would give two of its services one address. The
/// group has no fallback this crate can state, so nothing declared stays
/// nothing declared rather than being guessed at.
#[test]
fn a_manifest_declaring_no_wiring_leaves_the_service_its_own_id_and_no_group() {
    let mut manifest = whole();
    if let Some(manifest) = manifest.as_mut() {
        manifest.wirings.clear();
    }
    let entry = manifest
        .as_ref()
        .and_then(|one| one.services.first().map(|service| one.entry(service)));
    assert_eq!(entry.map(|entry| entry.hostname), Some("komga"));
    assert_eq!(entry.map(|entry| entry.group), Some(None));
}
