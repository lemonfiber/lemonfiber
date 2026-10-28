use std::sync::Arc;

use lemonfiber_adapters::filesystem::Disk;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_sidecar::decline::{File, Table, TokenHash};

use super::Configured;
use crate::app::invite::declining::path;
use crate::ports::service::{Claimed, Declinable, Declining, Invitations, KeyHeld};

/// The stack's own decline service under a fresh project.
fn decline(scratch: &Scratch) -> Configured {
    Configured::at(scratch.path(), Arc::new(Disk))
}

/// A key held is written where the service reads it, and answered back by its SHA-256
/// alone.
#[tokio::test]
async fn a_key_held_is_written_and_answered_by_its_hash() {
    let scratch = Scratch::new("decline-configured-held");
    let service = decline(&scratch);

    assert_eq!(service.held().await.ok(), Some(None));
    assert!(service.hold("0123456789abcdef", &[]).await.is_ok());
    assert_eq!(
        std::fs::read_to_string(path(scratch.path(), File::Key)).ok(),
        Some("0123456789abcdef\n".to_owned())
    );
    assert_eq!(
        service.held().await.ok(),
        Some(Some(KeyHeld {
            key: TokenHash::of("0123456789abcdef").as_str().to_owned(),
            pins: Vec::new(),
        }))
    );
}

/// A key of more than one word is refused and nothing is written.
#[tokio::test]
async fn a_key_of_two_words_is_refused_and_nothing_written() {
    let scratch = Scratch::new("decline-configured-two-words");
    let service = decline(&scratch);

    assert!(service.hold("two words", &[]).await.is_err());
    assert!(!path(scratch.path(), File::Key).exists());
}

/// The invitations applied are the table the service reads, token hashes kept as they
/// were handed over.
#[tokio::test]
async fn the_invitations_applied_are_the_table_the_service_reads() {
    let scratch = Scratch::new("decline-configured-table");
    let service = decline(&scratch);
    let token = TokenHash::of("ana-token");

    let applied = service
        .apply_invitations(&Invitations {
            claimed: Claimed::OwnWrites,
            invitations: vec![Declinable {
                token: token.as_str().to_owned(),
                account: "9".to_owned(),
                name: "ana".to_owned(),
                issued: 1,
                lapses: 2,
            }],
        })
        .await;

    assert!(applied.is_ok(), "{applied:?}");
    let table = std::fs::read_to_string(path(scratch.path(), File::Table))
        .ok()
        .and_then(|text| Table::read(&text).ok());
    let found = table.as_ref().and_then(|table| table.find(&token));
    assert_eq!(
        found.map(|one| (
            one.account.as_str(),
            one.name.as_str(),
            one.issued,
            one.lapses
        )),
        Some(("9", "ana", 1, 2))
    );
    assert_eq!(
        table.map(|table| table.claimed),
        Some(lemonfiber_sidecar::decline::Claimed::OwnWrites)
    );
}

/// Each way the core says a claim is told is the way the table the service reads tells it.
#[test]
fn each_way_a_claim_is_told_is_the_tables() {
    use lemonfiber_sidecar::decline::Claimed as Telling;

    for (said, told) in [
        (Claimed::HasPassword, Telling::HasPassword),
        (Claimed::OwnWrites, Telling::OwnWrites),
        (Claimed::Unknown, Telling::Unknown),
    ] {
        let invitations = Invitations {
            claimed: said,
            invitations: Vec::new(),
        };
        assert_eq!(super::table(&invitations).claimed, told);
    }
}
