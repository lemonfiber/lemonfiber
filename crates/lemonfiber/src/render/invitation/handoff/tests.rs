use super::handoff;
use lemonfiber_core::model::{HandedClient, HandedSession, Handoff, HandoffRemedy, HandoffState};

const ADDRESS: &str = "http://192.168.1.20:8096";

fn issued() -> Handoff {
    Handoff {
        name: "ana".to_owned(),
        state: HandoffState::Ready,
        reason: None,
        remedy: None,
        address: Some(ADDRESS.to_owned()),
        caution: None,
        issued: Some("2026-09-29T10:00:00Z".to_owned()),
        quick_connect: true,
        steps: vec!["Open the app.".to_owned(), "Sign in as ana.".to_owned()],
        clients: vec![HandedClient {
            device: "Android phone or tablet".to_owned(),
            client: "the official Jellyfin app".to_owned(),
            open_source: true,
            code: ADDRESS.to_owned(),
            deep_link: false,
        }],
        sessions: Vec::new(),
        rehearsed: false,
    }
}

/// A code ready to scan is drawn, with the address, the steps in order and the apps.
#[test]
fn a_ready_code_is_drawn_with_the_steps_to_take() {
    let said = handoff(&issued()).text();

    assert!(said.contains("ana's code is ready"), "{said}");
    assert!(said.contains(ADDRESS), "{said}");
    assert!(said.contains('\u{2588}'), "no code was drawn: {said}");
    assert!(
        said.contains("1. Open the app.") && said.contains("2. Sign in as ana."),
        "{said}"
    );
    assert!(said.contains("Android phone or tablet"), "{said}");
}

/// Waiting on the device is said as waiting, and the code is still there to scan.
#[test]
fn a_code_not_yet_used_is_waiting_rather_than_failed() {
    let said = handoff(&Handoff {
        state: HandoffState::Pending,
        reason: Some("the next step is theirs".to_owned()),
        ..issued()
    })
    .text();

    assert!(
        said.contains("has not signed in on a new device yet"),
        "{said}"
    );
    assert!(said.contains("the next step is theirs"), "{said}");
    assert!(!said.contains("could not be handed over"), "{said}");
    assert!(said.contains('\u{2588}'), "{said}");
}

/// Once a device is signed in, it is named as the media server lists it.
#[test]
fn a_device_signed_in_is_named_and_no_code_is_drawn() {
    let said = handoff(&Handoff {
        state: HandoffState::Connected,
        sessions: vec![HandedSession {
            device: "Pixel 8".to_owned(),
            client: "Jellyfin Android".to_owned(),
            last_seen: Some("2026-09-29T10:05:00Z".to_owned()),
        }],
        ..issued()
    })
    .text();

    assert!(said.contains("signed in on one device"), "{said}");
    assert!(
        said.contains("Pixel 8 — Jellyfin Android, last heard from 2026-09-29T10:05:00Z"),
        "{said}"
    );
    assert!(!said.contains('\u{2588}'), "{said}");
}

/// Two devices are counted as two.
#[test]
fn more_than_one_device_is_counted() {
    let session = HandedSession {
        device: "Pixel 8".to_owned(),
        client: "Jellyfin Android".to_owned(),
        last_seen: None,
    };
    let said = handoff(&Handoff {
        state: HandoffState::Connected,
        sessions: vec![session.clone(), session],
        ..issued()
    })
    .text();

    assert!(said.contains("signed in on 2 devices"), "{said}");
    assert_eq!(
        said.matches("Pixel 8 — Jellyfin Android").count(),
        2,
        "{said}"
    );
}

/// A hand-off that failed says why and draws nothing to scan.
#[test]
fn a_failed_hand_off_says_why_and_draws_nothing() {
    let said = handoff(&Handoff {
        state: HandoffState::Failed,
        reason: Some("That is a question of reaching the server.".to_owned()),
        address: None,
        ..issued()
    })
    .text();

    assert!(said.contains("could not be handed over"), "{said}");
    assert!(said.contains("reaching the server"), "{said}");
    assert!(!said.contains('\u{2588}'), "{said}");
}

/// Nobody with an account is told to be invited first.
#[test]
fn nobody_with_an_account_is_said_to_have_none() {
    let said = handoff(&Handoff {
        state: HandoffState::Unprovisioned,
        reason: Some("Invite them first.".to_owned()),
        ..issued()
    })
    .text();

    assert!(said.contains("ana has no account yet"), "{said}");
    assert!(said.contains("Invite them first."), "{said}");
    assert!(!said.contains('\u{2588}'), "{said}");
}

/// A closed app is marked as one wherever it is named.
#[test]
fn a_closed_app_is_marked_as_one() {
    let mut report = issued();
    report.clients.push(HandedClient {
        device: "Apple TV".to_owned(),
        client: "a paid player".to_owned(),
        open_source: false,
        code: ADDRESS.to_owned(),
        deep_link: false,
    });
    let said = handoff(&report).text();

    assert!(said.contains("a paid player (not open source)"), "{said}");
    assert!(
        !said.contains("official Jellyfin app (not open source)"),
        "{said}"
    );
}

/// A rehearsal says nothing was written down, and still shows the code.
#[test]
fn a_rehearsal_says_nothing_was_written_down() {
    let said = handoff(&Handoff {
        rehearsed: true,
        issued: None,
        ..issued()
    })
    .text();

    assert!(said.starts_with("Nothing was written down."), "{said}");
    assert!(said.contains('\u{2588}'), "{said}");
}

/// The caution about an address is said beside it.
#[test]
fn a_caution_about_the_address_is_said_beside_it() {
    let said = handoff(&Handoff {
        caution: Some("That address is a number.".to_owned()),
        ..issued()
    })
    .text();

    assert!(said.contains("That address is a number."), "{said}");
}

/// An address too long to draw still gets the words, and the steps under it.
#[test]
fn an_address_too_long_to_draw_still_gets_the_words() {
    let far_too_long = format!("http://{}", "h".repeat(8000));
    let mut too_long = issued();
    too_long.address = Some(far_too_long.clone());
    for client in &mut too_long.clients {
        client.code.clone_from(&far_too_long);
    }
    let said = handoff(&too_long).text();

    assert!(
        said.contains(&far_too_long),
        "the address itself went missing"
    );
    assert!(
        !said.contains('\u{2588}'),
        "an address that fits in no code was drawn"
    );
    assert!(
        said.contains("1. Open the app."),
        "{}",
        &said[..said.len().min(200)]
    );
}

/// Each remedy is said as the command that takes it here, and none is said where there
/// is none.
#[test]
fn a_remedy_is_said_as_the_command_that_takes_it() {
    let said = |state, remedy| {
        handoff(&Handoff {
            state,
            remedy,
            ..issued()
        })
        .text()
    };

    let invite = said(HandoffState::Unprovisioned, Some(HandoffRemedy::Invite));
    assert!(
        invite.contains("`lemonfiber invite ana --confirm`"),
        "{invite}"
    );
    let waiting = said(HandoffState::Ready, Some(HandoffRemedy::AskAgain));
    assert!(
        waiting.contains("`lemonfiber household handoff ana` again once they have"),
        "{waiting}"
    );
    let unread = said(HandoffState::Failed, Some(HandoffRemedy::AskAgain));
    assert!(unread.contains("again once the server answers"), "{unread}");
    let silent = said(HandoffState::Failed, Some(HandoffRemedy::StartServer));
    assert!(silent.contains("`lemonfiber status`"), "{silent}");
    let nowhere = said(HandoffState::Failed, Some(HandoffRemedy::RecordAddress));
    assert!(
        nowhere.contains("HOMEPAGE_VAR_LAN_HOST <address>"),
        "{nowhere}"
    );
    let none = said(HandoffState::Connected, None);
    assert!(!none.contains('`'), "{none}");
}

/// A client opened at the address by a link of its own has that link offered as a code
/// of its own, labelled with the client and the device (`G1-R15`); one opened at the
/// address itself is not drawn twice.
#[test]
fn a_clients_own_link_is_offered_as_a_code_of_its_own() {
    let mut linked = issued();
    linked.clients.push(HandedClient {
        device: "iPhone or iPad".to_owned(),
        client: "Swiftfin".to_owned(),
        open_source: true,
        code: format!("swiftfin://server?url={ADDRESS}"),
        deep_link: true,
    });
    let said = handoff(&linked).text();

    assert!(
        said.contains("To open Swiftfin on iPhone or iPad at it, this:"),
        "{said}"
    );
    assert!(
        !said.contains("To open the official Jellyfin app"),
        "a client opened at the address was drawn twice: {said}"
    );
}
