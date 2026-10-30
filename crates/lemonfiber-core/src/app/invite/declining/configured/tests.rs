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

/// The certificates the media server may present are written beside the key, and answered
/// back with it; a pin that is not a SHA-256 is refused and nothing is written.
#[tokio::test]
async fn the_pins_are_written_beside_the_key_and_answered_back() {
    let scratch = Scratch::new("decline-configured-pins");
    let service = decline(&scratch);
    let pins = vec!["ab".repeat(32), "cd".repeat(32)];

    assert!(service.hold("0123456789abcdef", &pins).await.is_ok());
    assert_eq!(
        std::fs::read_to_string(path(scratch.path(), File::Pins)).ok(),
        Some(format!("{}\n{}\n", "ab".repeat(32), "cd".repeat(32)))
    );
    assert_eq!(
        service.held().await.ok().flatten().map(|held| held.pins),
        Some(pins)
    );

    let refused = Scratch::new("decline-configured-pins-refused");
    let service = decline(&refused);
    assert!(service
        .hold("0123456789abcdef", &["not-a-pin".to_owned()])
        .await
        .is_err());
    assert!(!path(refused.path(), File::Key).exists());
    assert!(!path(refused.path(), File::Pins).exists());
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

/// What the service recorded is answered as its contract says it, every way it took an
/// invitation back included, and where it serves the decline page.
#[tokio::test]
async fn its_records_and_its_page_are_answered_as_the_contract_says() {
    use lemonfiber_sidecar::decline::{Lapse, Lapses, Left, Outcome, Refusal, Refusals};

    use crate::ports::service::{LeftBecause, TakenBack};

    let scratch = Scratch::new("decline-configured-records");
    let decline = decline(&scratch);
    assert_eq!(decline.refusals().await.ok(), Some(Vec::new()));
    assert_eq!(decline.lapses().await.ok(), Some(Vec::new()));

    let refusals = Refusals::default().with(Refusal {
        token: TokenHash::of("ana"),
        account: "9".to_owned(),
        at: 3,
    });
    let outcomes = [
        (Outcome::Removed, TakenBack::Removed),
        (Outcome::SwitchedOff, TakenBack::SwitchedOff),
        (Outcome::Left(Left::Claimed), LeftBecause::Claimed.into()),
        (
            Outcome::Left(Left::Administrator),
            LeftBecause::Administrator.into(),
        ),
        (Outcome::Left(Left::Disabled), LeftBecause::Disabled.into()),
        (Outcome::Left(Left::Gone), LeftBecause::Gone.into()),
        (
            Outcome::Left(Left::Reoffered),
            LeftBecause::Reoffered.into(),
        ),
        (
            Outcome::Left(Left::Unmatched),
            LeftBecause::Unmatched.into(),
        ),
    ];
    let written = outcomes
        .iter()
        .zip(0_u8..)
        .fold(Lapses::default(), |all, ((outcome, _), n)| {
            all.with(Lapse {
                token: TokenHash::of(&format!("bo-{n}")),
                account: "10".to_owned(),
                name: "bo".to_owned(),
                issued: 1,
                at: 2,
                outcome: *outcome,
            })
        });
    let at = |file| path(scratch.path(), file);
    let _ = std::fs::create_dir_all(at(File::Refusals).parent().unwrap_or(scratch.path()));
    let _ = std::fs::write(at(File::Refusals), refusals.written());
    let _ = std::fs::write(at(File::Lapses), written.written());

    let declined = decline.refusals().await.unwrap_or_default();
    let taken = decline.lapses().await.unwrap_or_default();

    assert_eq!(declined.len(), 1);
    assert_eq!(declined.first().map(|one| one.at), Some(3));
    assert_eq!(
        taken.iter().map(|one| one.outcome).collect::<Vec<_>>(),
        outcomes.iter().map(|(_, taken)| *taken).collect::<Vec<_>>()
    );
    assert_eq!(
        decline.page().await.ok().as_deref(),
        Some("/decline/{token}")
    );

    let _ = std::fs::write(at(File::Lapses), "not a record");
    assert!(decline.lapses().await.is_err());
    let _ = std::fs::write(at(File::Refusals), "not a record");
    assert!(decline.refusals().await.is_err());
}

/// A record that is a link is never followed.
#[cfg(unix)]
#[tokio::test]
async fn a_record_that_is_a_link_is_refused() {
    let scratch = Scratch::new("decline-configured-linked");
    let decline = decline(&scratch);
    let record = path(scratch.path(), File::Refusals);
    let elsewhere = scratch.path().join("elsewhere.json");
    let _ = std::fs::create_dir_all(record.parent().unwrap_or(scratch.path()));
    let _ = std::fs::write(&elsewhere, "{}");
    let _ = std::os::unix::fs::symlink(&elsewhere, &record);

    assert!(decline.refusals().await.is_err());
}
