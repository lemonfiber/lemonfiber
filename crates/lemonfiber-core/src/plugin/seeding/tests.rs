use std::collections::BTreeMap;
use std::path::Path;

use base64::Engine as _;
use lemonfiber_fixtures::ports::Chance;

use lemonfiber_manifest::container::Minted;

use super::{
    mint, minted_as, shape_of, shapes, starting, upstream, written, Shipped, MINTED_BYTES,
};
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
        super::ROUNDS,
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

#[test]
fn a_services_starting_files_hold_no_more_placeholders_in_all_than_it_may() {
    let text = "{{secret:api-key}}\n".repeat(9);
    let shipped = |service: &str, path: &str| Shipped {
        service: service.to_owned(),
        path: path.to_owned(),
        bytes: text.clone().into_bytes(),
    };
    let said = super::crowded(&[
        shipped("komga", "a.ini"),
        shipped("komga", "b.ini"),
        shipped("adapter", "c.ini"),
    ]);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said.iter().all(|one| one.location == "config/komga"
        && one.message.contains("holds 18 placeholders, past the 16")));
}

#[test]
fn the_hashed_form_is_refused_without_random_bytes_to_salt_it() {
    let refused = written(
        b"WebUI\\Password_PBKDF2=\"{{secret:api-key|pbkdf2}}\"",
        &held,
        &Chance::exactly(None),
    );
    assert_eq!(refused, Err(super::UNMINTED.to_owned()));
}

#[test]
fn a_replaced_value_is_written_where_the_old_one_stands_and_nothing_else_moves() {
    let template = b"[misc]\napi_key = {{secret:api-key}}\nport = 8080\n";
    let current = b"[misc]\nhost = 0.0.0.0\napi_key = was\nport = 8080\n# written by the service\n";
    let replaced = super::replaced(template, current, "api-key", "fedcba", &Chance::cycling());
    assert_eq!(
        replaced,
        Ok(Some(
            b"[misc]\nhost = 0.0.0.0\napi_key = fedcba\nport = 8080\n# written by the service\n"
                .to_vec()
        ))
    );
    assert_eq!(
        super::replaced(template, current, "other", "fedcba", &Chance::cycling()),
        Ok(None)
    );
}

#[test]
fn a_replacement_whose_line_is_gone_twice_there_or_unanchored_is_refused() {
    let random = Chance::cycling();
    let template = b"api_key = {{secret:api-key}}\n";
    for (template, current, says) in [
        (
            &template[..],
            &b"apikey = 0123\n"[..],
            "is not where the service keeps it now",
        ),
        (
            &template[..],
            &b"api_key = 01\napi_key = 23\n"[..],
            "is not where the service keeps it now",
        ),
        (
            &b"{{secret:api-key}}\n"[..],
            &b"0123\n"[..],
            "stands alone on its line",
        ),
        (&template[..], &[0xff, 0xfe][..], "it is not text"),
        (
            &[0xff, 0xfe][..],
            &b"api_key = 0123\n"[..],
            "it is not text",
        ),
        (
            &b"api_key = {{secret:api-key\n"[..],
            &b"api_key = 0123\n"[..],
            "never closes",
        ),
    ] {
        let refused = super::replaced(template, current, "api-key", "fedcba", &random);
        assert!(
            refused.as_ref().is_err_and(|why| why.contains(says)),
            "{says}: {refused:?}"
        );
    }
}

#[test]
fn a_replaced_hash_is_derived_afresh_for_the_new_value() {
    let hashing = |random: &Chance| {
        super::replaced(
            b"WebUI\\Password_PBKDF2=\"{{secret:api-key|pbkdf2}}\"\n",
            b"WebUI\\Port=8080\nWebUI\\Password_PBKDF2=\"@ByteArray(old:old)\"\n",
            "api-key",
            "fedcba",
            random,
        )
    };
    assert_eq!(
        hashing(&Chance::exactly(None)),
        Err(super::UNMINTED.to_owned())
    );
    let replaced = hashing(&Chance::exactly(Some(vec![9_u8; 16])))
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default();
    assert!(replaced.starts_with("WebUI\\Port=8080\nWebUI\\Password_PBKDF2=\"@ByteArray("));
    assert!(!replaced.contains("old:old"), "{replaced}");
}

/// Whether `value` holds 32 characters, every one of them a letter, a digit or `-_.~`, and
/// at least one of each class.
fn a_password(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|one| one.is_ascii_alphanumeric() || b"-_.~".contains(&one))
        && value.bytes().any(|one| one.is_ascii_uppercase())
        && value.bytes().any(|one| one.is_ascii_lowercase())
        && value.bytes().any(|one| one.is_ascii_digit())
        && value.bytes().any(|one| b"-_.~".contains(&one))
}

#[test]
fn a_minted_password_holds_every_class_however_alike_its_random_bytes() {
    for random in [
        Chance::cycling(),
        Chance::exactly(Some(vec![0x00; 80])),
        Chance::exactly(Some(vec![0xff; 80])),
        Chance::exactly(Some(vec![0xcd; 80])),
    ] {
        let value = minted_as(&random, Minted::Password).unwrap_or_default();
        assert!(a_password(&value));
    }
}

#[test]
fn a_minted_password_is_nothing_without_enough_random_bytes() {
    assert_eq!(minted_as(&Chance::exactly(None), Minted::Password), None);
    assert_eq!(
        minted_as(&Chance::exactly(Some(vec![0x10; 79])), Minted::Password),
        None
    );
}

#[test]
fn a_key_is_minted_as_before_and_an_unminted_secret_is_minted_nothing() {
    let random = Chance::exactly(Some(vec![0xab; MINTED_BYTES]));
    assert_eq!(minted_as(&random, Minted::Key), mint(&random));
    assert_eq!(minted_as(&random, Minted::Not), None);
}

#[test]
fn the_shapes_a_manifest_declares_are_read_beside_it_and_a_key_is_the_default() {
    let dir = lemonfiber_fixtures::scratch::Scratch::new("minted-shapes");
    let _ = std::fs::write(
        dir.join(lemonfiber_bundle::MANIFEST),
        crate::plugin::program::tests::declaring_a_password(),
    );

    let read = shapes(&dir);

    assert_eq!(read.get("admin-password"), Some(&Minted::Password));
    assert_eq!(shape_of(&read, "admin-password"), Minted::Password);
    assert_eq!(shape_of(&read, "never-declared"), Minted::Key);
    assert!(shapes(Path::new("/lemonfiber/no/such/plugin")).is_empty());
}
