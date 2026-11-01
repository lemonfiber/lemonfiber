use std::path::Path;

use lemonfiber_contract::adapter::UPSTREAM_FILE;
use lemonfiber_fixtures::support::FixedRandom;
use serde_json::{json, Value};

use super::Filling;
use crate::plugin::program::Throwaway;

const MINTED: &str = "0707070707070707070707070707070707070707070707070707070707070707";

const CERTIFICATE: &str = "/config/lemonfiber/internal.p12";

const PIN: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

struct Filler {
    random: FixedRandom,
    held: Throwaway,
    certificate: Option<(String, String)>,
}

impl Filler {
    fn new(random: Option<u8>) -> Option<Self> {
        Some(Self {
            random: FixedRandom(random.map(|byte| vec![byte; 32])),
            held: Throwaway::of(None, &lemonfiber_fixtures::ports::Chance::cycling()).ok()?,
            certificate: random.map(|_| (CERTIFICATE.to_owned(), PIN.to_owned())),
        })
    }

    fn filling(&self) -> Filling<'_> {
        Filling {
            random: &self.random,
            held: &self.held,
            answers: [
                (
                    "identity.source@1/whoever-answers".to_owned(),
                    json!({"id": "m1", "token": "t0k3n"}),
                ),
                (
                    "identity.source@1/household-answers".to_owned(),
                    json!([{"id": 3}]),
                ),
                (
                    "identity.source@1/revoke-answers".to_owned(),
                    json!({"type": "unsupported", "detail": "it revokes nothing"}),
                ),
                (
                    "identity.source@1/claim-answers".to_owned(),
                    json!({"type": "unauthorised", "detail": "the upstream rejected the credential"}),
                ),
                (
                    "identity.source@1/sessions-answers".to_owned(),
                    json!({"oops": true}),
                ),
            ]
            .into_iter()
            .collect(),
            made: Vec::new(),
            hashed: Vec::new(),
            certificate: self.certificate.clone(),
        }
    }

    fn filled(&self, upstream: Option<&str>, asks: &Value) -> (Result<Value, String>, usize) {
        if let Some(text) = upstream {
            let _ = self.held.hold(UPSTREAM_FILE, text);
        }
        let mut filling = self.filling();
        let filled = filling.filled(asks).map(|filled| filled.value);
        (filled, filling.made.len())
    }
}

#[test]
fn every_placeholder_is_filled_wherever_it_stands_and_only_credentials_are_kept_as_made() {
    let Some(filler) = Filler::new(Some(7)) else {
        return;
    };
    let asks = json!({
        "name": "{upstream:username}",
        "signed": ["{identity.source@1/whoever-answers#/token}", 12, null],
        "key": "{minted}",
        "plain": "conformance-member",
        "path": "{certificate}",
        "pin": "{certificate:sha256}",
    });
    assert_eq!(
        filler.filled(Some("{\"username\": \"admin\"}"), &asks),
        (
            Ok(json!({
                "name": "admin",
                "signed": ["t0k3n", 12, null],
                "key": MINTED,
                "plain": "conformance-member",
                "path": CERTIFICATE,
                "pin": PIN,
            })),
            2
        )
    );
}

#[test]
fn a_placeholder_with_nothing_to_fill_it_names_itself_and_why() {
    let Some(filler) = Filler::new(None) else {
        return;
    };
    let why = |upstream: Option<&str>, placeholder: &str| {
        filler
            .filled(upstream, &json!({ "x": placeholder }))
            .0
            .err()
            .unwrap_or_default()
    };
    assert_eq!(
        why(None, "{upstream:password}"),
        "{upstream:password} cannot be filled: there is no upstream.json yet"
    );
    assert_eq!(
        why(Some("[]"), "{upstream:password}"),
        "{upstream:password} cannot be filled: upstream.json is not an object"
    );
    assert_eq!(
        why(Some("{}"), "{upstream:password}"),
        "{upstream:password} cannot be filled: upstream.json holds no password"
    );
    assert_eq!(
        why(None, "{identity.source@1/invite-answers#/id}"),
        "{identity.source@1/invite-answers#/id} cannot be filled: identity.source@1/invite-answers was not asked before it"
    );
    assert_eq!(
        why(None, "{identity.source@1/household-answers#/0/id}"),
        "{identity.source@1/household-answers#/0/id} cannot be filled: identity.source@1/household-answers answered no string at \"/0/id\""
    );
    assert_eq!(
        why(None, "{certificate}"),
        "{certificate} cannot be filled: no internal certificate was written for the upstream"
    );
    assert_eq!(
        why(None, "{certificate:sha256}"),
        "{certificate:sha256} cannot be filled: no internal certificate was written for the upstream"
    );
    assert_eq!(
        why(None, "{minted}"),
        format!(
            "{{minted}} cannot be filled: {}",
            crate::secret::NO_RANDOMNESS_FOR_PASSWORD
        )
    );
    assert_eq!(
        why(None, "{minted:upstream:password}"),
        format!(
            "{{minted:upstream:password}} cannot be filled: {}",
            crate::secret::NO_RANDOMNESS_FOR_PASSWORD
        )
    );
}

/// The status and the refusal a case is recorded as answering without being asked.
fn unasked(filling: &Filling<'_>, asks: &Value, speaks: &[String]) -> Option<(u16, Value)> {
    filling.unasked(asks, speaks).map(|response| {
        (
            response.status,
            serde_json::from_str(&response.body).unwrap_or(Value::Null),
        )
    })
}

#[test]
fn a_case_reading_an_unspoken_contract_or_an_unsupported_answer_is_recorded_unsupported() {
    let Some(filler) = Filler::new(None) else {
        return;
    };
    let filling = filler.filling();
    let speaks = ["identity.source@1".to_owned()];
    assert_eq!(
        unasked(
            &filling,
            &json!({"by": ["plain", "{media.serve@1/mint-answers#/key}"]}),
            &speaks
        ),
        Some((
            500,
            json!({"type": "unsupported", "detail": "it speaks no media.serve@1, which media.serve@1/mint-answers is a case of"})
        ))
    );
    assert_eq!(
        unasked(
            &filling,
            &json!({"x": {"id": "{identity.source@1/revoke-answers#/id}"}}),
            &speaks
        ),
        Some((
            500,
            json!({"type": "unsupported", "detail": "identity.source@1/revoke-answers was answered unsupported"})
        ))
    );
    assert_eq!(
        unasked(
            &filling,
            &json!({"signed": ["{identity.source@1/whoever-answers#/token}", "{upstream:username}", 12]}),
            &speaks
        ),
        None
    );
}

#[test]
fn a_case_reading_a_refused_answer_is_recorded_with_that_refusal_at_its_status() {
    let Some(filler) = Filler::new(None) else {
        return;
    };
    let filling = filler.filling();
    let speaks = ["identity.source@1".to_owned()];
    assert_eq!(
        unasked(
            &filling,
            &json!({"id": "{identity.source@1/claim-answers#/id}"}),
            &speaks
        ),
        Some((
            502,
            json!({"type": "unauthorised", "detail": "identity.source@1/claim-answers was refused: the upstream rejected the credential"})
        ))
    );
}

#[test]
fn a_case_reading_an_answer_no_refusal_explains_is_asked_and_cannot_be_filled() {
    let Some(filler) = Filler::new(None) else {
        return;
    };
    let mut filling = filler.filling();
    let asks = json!({"member": "{identity.source@1/sessions-answers#/0}"});
    assert_eq!(
        unasked(&filling, &asks, &["identity.source@1".to_owned()]),
        None
    );
    assert_eq!(
        filling.filled(&asks).err(),
        Some(
            "{identity.source@1/sessions-answers#/0} cannot be filled: identity.source@1/sessions-answers answered no string at \"/0\""
                .to_owned()
        )
    );
}

#[test]
fn a_minted_credential_is_written_into_what_the_adapter_holds_beside_its_fields() {
    let Some(filler) = Filler::new(Some(7)) else {
        return;
    };
    let _ = filler.held.hold(UPSTREAM_FILE, "{\"username\": \"admin\"}");
    let filling = filler.filling();
    assert_eq!(filling.keep_minted(&[]), Ok(()));
    assert_eq!(
        filling.keep_minted(&[("password".to_owned(), "s3cret".to_owned())]),
        Ok(())
    );
    let held = filler
        .held
        .wrote(Path::new(UPSTREAM_FILE))
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    assert_eq!(
        held,
        Some(json!({"username": "admin", "password": "s3cret"}))
    );
}

#[test]
fn each_minted_credential_is_kept_with_its_lower_case_sha256() {
    let Some(filler) = Filler::new(Some(7)) else {
        return;
    };
    let mut filling = filler.filling();
    assert!(filling
        .filled(&json!(["{minted}", "{minted:upstream:key}"]))
        .is_ok());
    let hashed = crate::secret::render(
        ring::digest::digest(&ring::digest::SHA256, MINTED.as_bytes()).as_ref(),
    );
    assert_eq!(filling.hashed, vec![hashed.clone(), hashed]);
}
