use std::collections::BTreeMap;

use base64::Engine as _;

use lemonfiber_contract::conformance::MINTED_HASH;

use super::{Scrub, REDACTED};
use crate::plugin::recorded::Answer;
use crate::plugin::recording::alike::alike;
use crate::ports::serving::Served;

const KEY: &str = "Ab/c+d=e f\"g";

fn served() -> Served {
    Served {
        container: "4f1c2d3e5a6b7c8d9e0f".to_owned(),
        network: "lemonfiber-record-1-1-0".to_owned(),
        hostname: "media-host".to_owned(),
        addresses: vec!["172.18.0.2".to_owned()],
        machine: vec![
            "orbstack".to_owned(),
            "7.0.14-orbstack-00380-ga7e0a2dc9535".to_owned(),
            "OrbStack".to_owned(),
        ],
        port: Some(1),
        contract: None,
    }
}

#[test]
fn a_credential_is_scrubbed_in_every_encoding_an_answer_could_echo_it_in_either_case() {
    let scrub = Scrub::of(&[KEY.to_owned()], None);
    let bytes = KEY.as_bytes();
    for spelling in [
        KEY.to_owned(),
        KEY.to_lowercase(),
        KEY.to_uppercase(),
        crate::endpoint::query_encoded(KEY),
        crate::endpoint::query_encoded(KEY).to_lowercase(),
        form_urlencoded::byte_serialize(bytes).collect::<String>(),
        base64::engine::general_purpose::STANDARD.encode(bytes),
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes),
        base64::engine::general_purpose::URL_SAFE.encode(bytes),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
        "Ab/c+d=e f\\\"g".to_owned(),
    ] {
        let said = scrub.text(&format!("before {spelling} after"));
        assert_eq!(said, format!("before {REDACTED} after"), "{spelling}");
        assert!(!scrub.holds(&said));
        assert!(scrub.holds(&spelling), "{spelling}");
    }
}

#[test]
fn the_containers_id_short_id_hostname_and_addresses_are_scrubbed() {
    let scrub = Scrub::of(&[], Some(&served()));
    assert_eq!(
        scrub.text("4f1c2d3e5a6b7c8d9e0f 4f1c2d3e5a6b media-host 172.18.0.2 other"),
        format!("{REDACTED} {REDACTED} {REDACTED} {REDACTED} other")
    );
    assert!(!scrub.holds("lemonfiber-record-1-1-0"));
}

#[test]
fn the_recording_machines_name_kernel_and_system_are_scrubbed_and_a_count_of_it_kept() {
    let scrub = Scrub::of(&[], Some(&served()));
    let answer = Answer {
        status: 200,
        headers: BTreeMap::new(),
        json: Some(serde_json::json!({
            "operating_system": "Linux-7.0.14-orbstack-00380-ga7e0a2dc9535-aarch64-with",
            "host": "orbstack",
            "platform": "OrbStack",
            "cpu_cores": 8
        })),
        body_starts_with: None,
    };
    assert_eq!(
        scrub.answer(answer).json,
        Some(serde_json::json!({
            "operating_system": format!("Linux-{REDACTED}-aarch64-with"),
            "host": REDACTED,
            "platform": REDACTED,
            "cpu_cores": 8
        }))
    );
}

#[test]
fn a_value_too_short_to_name_is_never_scrubbed_for_every_place_it_appears() {
    let scrub = Scrub::of(&["ab".to_owned(), String::new()], None);
    assert_eq!(scrub, Scrub::default());
    assert_eq!(scrub.text("abc"), "abc");
}

#[test]
fn an_answers_headers_keys_strings_and_text_are_scrubbed_and_its_numbers_kept() {
    let scrub = Scrub::of(&["s3cret-key".to_owned()], None);
    let answer = Answer {
        status: 200,
        headers: BTreeMap::from([("content-type".to_owned(), "text/s3cret-key".to_owned())]),
        json: Some(serde_json::json!({
            "s3cret-key": ["a s3cret-key", {"n": 7}],
            "plain": true
        })),
        body_starts_with: Some("S3CRET-KEY".to_owned()),
    };
    let scrubbed = scrub.answer(answer);
    assert_eq!(scrubbed.status, 200);
    assert_eq!(
        scrubbed.headers.get("content-type").map(String::as_str),
        Some("text/<redacted>")
    );
    assert_eq!(
        scrubbed.json,
        Some(serde_json::json!({
            "<redacted>": ["a <redacted>", {"n": 7}],
            "plain": true
        }))
    );
    assert_eq!(scrubbed.body_starts_with.as_deref(), Some(REDACTED));
}

#[test]
fn the_longest_spelling_goes_first_so_a_shorter_one_inside_it_leaves_nothing_behind() {
    let scrub = Scrub::of(&["abcd".to_owned(), "abcdefgh".to_owned()], None);
    assert_eq!(scrub.text("abcdefgh"), REDACTED);
}

fn sha256(value: &str) -> String {
    crate::secret::render(ring::digest::digest(&ring::digest::SHA256, value.as_bytes()).as_ref())
}

fn admitted(credential: &str) -> Answer {
    Answer {
        status: 200,
        headers: BTreeMap::new(),
        json: Some(serde_json::json!([{ "route": "tv", "credential": sha256(credential) }])),
        body_starts_with: None,
    }
}

#[test]
fn the_hash_of_a_minted_credential_is_its_placeholder_on_every_run_and_any_other_hash_differs() {
    let taken = |minted: &str, answered: &str| {
        Scrub::of(&[minted.to_owned()], None)
            .and_hashed(&[sha256(minted)])
            .answer(admitted(answered))
            .json
            .unwrap_or_default()
    };
    let recorded = taken("first-run-credential", "first-run-credential");
    assert_eq!(
        recorded,
        serde_json::json!([{ "route": "tv", "credential": MINTED_HASH }])
    );
    let again = taken("second-run-credential", "second-run-credential");
    assert!(alike(&recorded, &again));
    let other = taken("second-run-credential", "another-credential");
    assert_eq!(
        other.pointer("/0/credential"),
        Some(&serde_json::json!(sha256("another-credential")))
    );
    assert!(!alike(&recorded, &other));
}
