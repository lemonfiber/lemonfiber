use lemonfiber_ports::service::Failure;

use super::{answered, asked, Kind, Refusal};

/// Each way a port fails crosses as its own refusal and is read back as the same way.
#[test]
fn a_failure_crosses_and_is_read_back_as_itself() {
    let failures = [
        Failure::Unavailable {
            service: "upstream".to_owned(),
        },
        Failure::Unauthorised {
            service: "upstream".to_owned(),
        },
        Failure::Refused {
            service: "upstream".to_owned(),
            detail: "odd".to_owned(),
        },
        Failure::Unsupported {
            service: "upstream".to_owned(),
            detail: "too new".to_owned(),
        },
    ];
    for failure in failures {
        let read = Refusal::from(&failure).failure("adapter");
        assert_eq!(
            std::mem::discriminant(&read),
            std::mem::discriminant(&failure)
        );
    }
}

/// A call nobody declared and a body that does not read are refused, and read as refused.
#[test]
fn what_only_a_contract_can_refuse_is_read_as_refused() {
    for kind in [Kind::UnknownOperation, Kind::NotAsked] {
        assert!(matches!(
            Refusal::new(kind, "x").failure("adapter"),
            Failure::Refused { .. }
        ));
    }
    assert_eq!(
        Refusal::unknown_operation("nope").kind,
        Kind::UnknownOperation
    );
}

#[test]
fn every_refusal_has_a_status_of_its_own_kind() {
    let statuses = [
        (Kind::Unavailable, 503),
        (Kind::Unauthorised, 502),
        (Kind::Refused, 500),
        (Kind::Unsupported, 500),
        (Kind::UnknownOperation, 404),
        (Kind::NotAsked, 400),
    ];
    for (kind, status) in statuses {
        assert_eq!(kind.status(), status);
    }
}

#[test]
fn a_body_that_is_not_the_request_is_refused_as_not_asked() {
    let read: Result<u32, Refusal> = asked(b"{\"x\":1}");
    assert_eq!(read.err().map(|refusal| refusal.kind), Some(Kind::NotAsked));
    assert_eq!(asked::<u32>(b"7").ok(), Some(7));
}

#[test]
fn an_answer_is_its_json_and_a_failure_is_its_refusal() {
    assert_eq!(answered(Ok(3_u8)).ok(), Some(b"3".to_vec()));
    let failed: Result<u8, Failure> = Err(Failure::Unavailable {
        service: "upstream".to_owned(),
    });
    assert_eq!(
        answered(failed).err().map(|refusal| refusal.kind),
        Some(Kind::Unavailable)
    );
}

/// A refusal with a field the contract does not have is not one.
#[test]
fn a_refusal_reads_only_as_the_contract_writes_it() {
    let strict: Result<Refusal, _> =
        serde_json::from_str(r#"{"type":"refused","detail":"x","more":1}"#);
    assert!(strict.is_err());
    let written = serde_json::to_string(&Refusal::new(Kind::NotAsked, "x")).unwrap_or_default();
    assert_eq!(written, r#"{"type":"not-asked","detail":"x"}"#);
}

/// What cannot be written down.
struct Unwritable;

impl serde::Serialize for Unwritable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("no"))
    }
}

/// An answer that cannot be written is refused rather than sent half-written.
#[test]
fn an_answer_that_cannot_be_written_is_refused() {
    assert_eq!(
        answered(Ok(Unwritable)).err().map(|refusal| refusal.kind),
        Some(Kind::Refused)
    );
}
