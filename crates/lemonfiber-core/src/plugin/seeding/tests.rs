use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::path::Path;

use base64::Engine as _;
use lemonfiber_fixtures::ports::Chance;

use super::{mint, starting, upstream, written, Shipped, MINTED_BYTES};
use crate::plugin::Lands;

/// The value the store holds as `api-key`.
const HELD: &str = "0123456789abcdef0123456789abcdef";

/// What the store holds: `api-key` alone.
fn held(secret: &str) -> Option<String> {
    (secret == "api-key").then(|| HELD.to_owned())
}

#[test]
fn a_minted_secret_is_32_lowercase_hexadecimal_digits_and_none_without_random_bytes() {
    let minted = mint(&Chance::exactly(Some(vec![0xab; MINTED_BYTES])));
    assert_eq!(minted.as_deref(), Some("abababababababababababababababab"));
    assert_eq!(mint(&Chance::exactly(None)), None);
}

#[test]
fn a_starting_file_is_written_with_the_value_where_it_names_a_secret() {
    let written = written(
        b"[misc]\napi_key = {{secret:api-key}}\n",
        &held,
        &Chance::cycling(),
    );
    assert_eq!(
        written.as_deref(),
        Ok(format!("[misc]\napi_key = {HELD}\n").as_bytes())
    );
}

#[test]
fn a_file_naming_no_secret_is_written_as_shipped_whatever_it_holds() {
    let binary = [0xff, 0x00, 0x7b, 0x7b];
    assert_eq!(
        written(&binary, &held, &Chance::cycling()),
        Ok(binary.to_vec())
    );
}

#[test]
fn the_hashed_form_is_a_salted_pbkdf2_of_the_value_that_verifies() {
    let salt = vec![7_u8; 16];
    let written = written(
        b"WebUI\\Password_PBKDF2=\"{{secret:api-key|pbkdf2}}\"",
        &held,
        &Chance::exactly(Some(salt.clone())),
    )
    .ok()
    .and_then(|bytes| String::from_utf8(bytes).ok())
    .unwrap_or_default();
    let inner = written
        .split_once("@ByteArray(")
        .and_then(|(_, rest)| rest.split_once(")\""))
        .map(|(inner, _)| inner)
        .unwrap_or_default();
    let (salted, hash) = inner.split_once(':').unwrap_or_default();
    let encoded = base64::engine::general_purpose::STANDARD;
    assert_eq!(encoded.decode(salted).ok(), Some(salt.clone()));
    let hash = encoded.decode(hash).unwrap_or_default();
    let verified = ring::pbkdf2::verify(
        ring::pbkdf2::PBKDF2_HMAC_SHA512,
        NonZeroU32::new(100_000).unwrap_or(NonZeroU32::MIN),
        &salt,
        HELD.as_bytes(),
        &hash,
    );
    assert!(verified.is_ok(), "{written}");
    assert!(!written.contains("0123456789abcdef"));
}

#[test]
fn a_secret_nothing_holds_is_named_by_its_placeholder_and_never_by_a_value() {
    let refused = written(b"key = {{secret:other}}", &held, &Chance::cycling());
    assert_eq!(
        refused,
        Err("a placeholder names the secret other, which nothing holds".to_owned())
    );
}

#[test]
fn an_adapter_is_handed_each_field_with_its_secrets_value() {
    let fields = BTreeMap::from([("api_key".to_owned(), "api-key".to_owned())]);
    assert_eq!(
        upstream(&fields, &held),
        Ok(format!(r#"{{"api_key":"{HELD}"}}"#))
    );
    let missing = BTreeMap::from([("api_key".to_owned(), "other".to_owned())]);
    assert!(
        upstream(&missing, &held).is_err_and(|why| why.contains("api_key names the secret other"))
    );
}

#[test]
fn a_starting_file_is_planned_only_where_the_service_holds_none_yet() {
    let shipped = |path: &str, bytes: &[u8]| Shipped {
        service: "sabnzbd".to_owned(),
        path: path.to_owned(),
        bytes: bytes.to_vec(),
    };
    let stack = Path::new("/opt/stack");
    let planned = starting(
        vec![
            shipped("sabnzbd.ini", b"api_key = {{secret:api-key}}"),
            shipped("theme.css", b"body {}"),
            shipped("kept.ini", b"kept"),
        ],
        stack,
        &|at| at.ends_with("kept.ini"),
    );
    let read: Vec<(String, bool)> = planned
        .iter()
        .map(|one| {
            (
                one.path.display().to_string(),
                matches!(one.lands, Lands::Starting { seeded: true, .. }),
            )
        })
        .collect();
    assert_eq!(
        read,
        vec![
            ("/opt/stack/config/sabnzbd/sabnzbd.ini".to_owned(), true),
            ("/opt/stack/config/sabnzbd/theme.css".to_owned(), false),
        ]
    );
}
