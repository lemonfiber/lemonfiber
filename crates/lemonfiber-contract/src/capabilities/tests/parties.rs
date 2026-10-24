use lemonfiber_ports::serving::StandIn;
use serde_json::Value;

use crate::capabilities::{curator_answering, parties, request::identity_answering};

fn said(stand_in: &StandIn, method: &str, path: &str) -> Option<Value> {
    stand_in
        .answers
        .iter()
        .find(|said| said.method == method && said.path == path)
        .and_then(|said| serde_json::from_str(said.body.as_deref()?).ok())
}

fn every_case_asks() -> String {
    crate::capabilities::all()
        .iter()
        .flat_map(crate::conformance::cases)
        .map(|case| case.asks.to_string())
        .collect()
}

#[test]
fn every_party_answers_on_a_port_of_its_own_at_a_host_the_cases_name() {
    let all = parties(&["tv".to_owned()]);
    let asks = every_case_asks();
    let mut ports: Vec<u16> = all.iter().map(|party| party.answering.port).collect();
    ports.sort_unstable();
    ports.dedup();

    assert_eq!(
        all.iter().map(|party| party.named).collect::<Vec<_>>(),
        vec!["download client", "identity source", "curator"]
    );
    assert_eq!(ports.len(), all.len());
    assert!(ports.iter().all(|port| *port > 1024), "{ports:?}");
    for party in &all {
        assert!(
            asks.contains(party.host),
            "{} is named by no case",
            party.host
        );
    }
}

#[test]
fn the_identity_source_signs_in_the_owner_and_holds_the_member_the_cases_link() {
    let identity = identity_answering();

    let signed = said(&identity, "POST", "/Users/AuthenticateByName");
    let users = said(&identity, "GET", "/Users");
    let keys = said(&identity, "GET", "/Auth/Keys");

    assert_eq!(identity.port, 58096);
    assert!(signed
        .as_ref()
        .and_then(|signed| signed.pointer("/User/Policy/IsAdministrator"))
        .is_some_and(|administrator| administrator == true));
    assert_eq!(
        signed
            .as_ref()
            .and_then(|signed| signed.pointer("/User/Name")),
        Some(&Value::from("conformance"))
    );
    let ids: Vec<&str> = users
        .as_ref()
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|user| user.get("Id")?.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![
            "0a11ce0000000000000000000000000a",
            "c0f04a7e5e1d4b2a9c3f0e8d7b6a5c4d"
        ]
    );
    assert_eq!(
        keys.as_ref()
            .and_then(|keys| keys.pointer("/Items/0/AppName")),
        Some(&Value::from("{query:App}"))
    );
    assert!(identity
        .answers
        .iter()
        .any(|said| said.method == "POST" && said.path == "/Auth/Keys" && said.status == 204));
    assert!(said(&identity, "GET", "/System/Info/Public").is_some());
    assert!(said(&identity, "GET", "/System/Info").is_some());
}

#[test]
fn the_curator_answers_what_an_upstream_tests_a_curator_with_and_says_its_release() {
    let curator = curator_answering();

    assert_eq!(curator.port, 58989);
    assert_eq!(
        curator.headers,
        vec![("X-Application-Version".to_owned(), "4.0.15.2941".to_owned())]
    );
    assert_eq!(
        said(&curator, "GET", "/api/v3/system/status")
            .and_then(|status| status.get("version").cloned()),
        Some(Value::from("4.0.15.2941"))
    );
    assert_eq!(
        said(&curator, "GET", "/api/v3/qualityprofile"),
        Some(serde_json::json!([{ "id": 1, "name": "conformance" }]))
    );
    assert!(said(&curator, "GET", "/api/v3/rootfolder")
        .and_then(|folders| folders.pointer("/0/path").cloned())
        .is_some_and(|path| path == "/data/conformance"));
    let schemas: Vec<String> = said(&curator, "GET", "/api/v3/indexer/schema")
        .and_then(|schemas| schemas.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|schema| schema.get("implementation")?.as_str().map(str::to_owned))
        .collect();
    assert_eq!(schemas, vec!["Newznab", "Torznab"]);
    assert_eq!(
        said(&curator, "GET", "/api/v3/tag"),
        Some(serde_json::json!([]))
    );
    assert_eq!(
        said(&curator, "GET", "/api/v3/indexer"),
        Some(serde_json::json!([]))
    );
    assert_eq!(
        said(&curator, "POST", "/api/v3/indexer/test"),
        Some(serde_json::json!({}))
    );
}

fn fields_of(curator: &StandIn, implementation: &str) -> Vec<Value> {
    said(curator, "GET", "/api/v3/indexer/schema")
        .and_then(|schemas| schemas.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .find(|schema| schema.get("implementation") == Some(&Value::from(implementation)))
        .and_then(|schema| schema.get("fields")?.as_array().cloned())
        .unwrap_or_default()
}

fn field<'a>(fields: &'a [Value], name: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|field| field.get("name") == Some(&Value::from(name)))
}

#[test]
fn every_indexer_schema_the_curator_answers_holds_each_field_a_curator_of_series_does() {
    use serde_json::json;
    let curator = curator_answering();
    let newznab = fields_of(&curator, "Newznab");
    let torznab = fields_of(&curator, "Torznab");

    for fields in [&newznab, &torznab] {
        assert_eq!(
            field(fields, "animeCategories"),
            Some(&json!({ "order": 4, "name": "animeCategories", "value": [] }))
        );
        assert_eq!(
            field(fields, "animeStandardFormatSearch").and_then(|field| field.get("value")),
            Some(&Value::from(false))
        );
        assert!(field(fields, "additionalParameters").is_some());
        let mut orders: Vec<u64> = fields
            .iter()
            .filter_map(|field| field.get("order")?.as_u64())
            .collect();
        orders.dedup();
        assert_eq!(orders, (0..fields.len() as u64).collect::<Vec<_>>());
    }
    assert_eq!(newznab.len(), 7);
    assert_eq!(
        field(&torznab, "minimumSeeders").and_then(|field| field.get("value")),
        Some(&Value::from(1))
    );
    for seeding in [
        "seedCriteria.seedRatio",
        "seedCriteria.seedTime",
        "seedCriteria.seasonPackSeedTime",
    ] {
        assert!(field(&torznab, seeding).is_some(), "{seeding}");
        assert!(field(&newznab, seeding).is_none(), "{seeding}");
    }
    assert_eq!(
        field(&torznab, "rejectBlocklistedTorrentHashesWhileGrabbing")
            .and_then(|field| field.get("value")),
        Some(&Value::from(false))
    );
    assert_eq!(torznab.len(), 12);
}
