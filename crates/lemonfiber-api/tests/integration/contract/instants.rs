//! Every instant an answer carries crosses as a whole number of seconds.
//!
//! Read off the documents the samples write, one family of envelope at a time, so a
//! field that went back to text in any of them fails here by its kind and its place.

use serde_json::Value;

use super::samples::samples;

/// Each kind, with the places in its document an instant is carried.
const CARRIED: &[(&str, &[&str])] = &[
    ("version", &["/data/attestation_status"]),
    (
        "news-items",
        &["/data/requests/0/asked", "/data/problems/0/onset"],
    ),
    ("handoff", &["/data/issued", "/data/sessions/0/last_seen"]),
    ("pairing", &["/data/until", "/data/material/expires"]),
    ("key-claim", &["/data/lapses"]),
    (
        "keys",
        &[
            "/data/keys/0/minted",
            "/data/keys/0/used",
            "/data/keys/1/revoked",
        ],
    ),
    ("device-key", &["/data/registered"]),
    (
        "device-keys",
        &[
            "/data/keys/0/registered",
            "/data/keys/0/used",
            "/data/keys/1/revoked",
        ],
    ),
    ("push-endpoint", &["/data/registered"]),
    ("push-endpoints", &["/data/endpoints/0/registered"]),
];

#[test]
fn every_instant_an_envelope_carries_is_whole_seconds() {
    let written: Vec<(String, Value)> = samples()
        .into_iter()
        .map(|outcome| {
            let envelope = outcome.envelope();
            let kind = envelope.kind.as_str().to_owned();
            (kind, serde_json::to_value(envelope).unwrap_or_default())
        })
        .collect();
    for (kind, places) in CARRIED {
        let document = written
            .iter()
            .find(|(named, _)| named == kind)
            .map(|(_, document)| document);
        assert!(document.is_some(), "no sample writes a {kind} document");
        for place in *places {
            let carried = document.and_then(|document| document.pointer(place));
            assert!(
                carried.is_some_and(Value::is_u64),
                "{kind} carries {place} as {carried:?}, not as whole seconds"
            );
        }
    }
}
