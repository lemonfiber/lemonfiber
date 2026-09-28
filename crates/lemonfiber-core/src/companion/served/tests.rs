use lemonfiber_fixtures::scratch::Scratch;

use super::{last, record, Served, FILE};

/// What a run wrote is what pairing reads back.
#[test]
fn how_the_surface_was_served_is_read_back_as_written() {
    let at = Scratch::new("companion-served");
    assert_eq!(last(&at), None);
    let served = Served {
        port: 8443,
        encrypted: true,
        network: true,
    };
    assert_eq!(record(&at, served), Ok(()));
    assert_eq!(last(&at), Some(served));
}

/// Something that is not this record reads as no record, rather than as a port.
#[test]
fn a_record_that_is_not_one_reads_as_none() {
    let at = Scratch::new("companion-served-unreadable");
    let _ = std::fs::write(at.join(FILE), r#"{"port": 8443}"#);
    assert_eq!(last(&at), None);
}

/// A record that cannot be written says why.
#[test]
fn a_record_that_cannot_be_written_says_why() {
    let at = Scratch::new("companion-served-unwritable");
    let blocked = at.join("blocked");
    let _ = std::fs::write(&blocked, "a file where the directory would go");
    let written = record(
        &blocked,
        Served {
            port: 1,
            encrypted: false,
            network: false,
        },
    );
    assert!(written.is_err(), "{written:?}");
}
