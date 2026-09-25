use std::net::SocketAddr;

use lemonfiber_core::platform::HostOs;

use super::super::fixtures::{bound, exited, missing};
use super::super::reach::{address, held, Offered};
use super::{announcement, opener, opening, reverted, Browser};

/// Everything a starting surface says, as one block of text.
fn said(browser: Browser) -> String {
    announcement(&[bound()], Offered::Machine, "000fa5ff", browser).join("\n")
}

/// The word the claim about the transport turns on. A rewording that drops it
/// has changed the claim rather than the wording.
const ENCRYPTION: &str = "encrypt";

/// How a sentence says there is none of something.
///
/// The closed set English denies with, rather than a list of ways to phrase this
/// particular sentence: a reword is free to say it however it likes, so long as
/// it still says *not*.
const DENIAL: &[&str] = &[
    "not",
    "no",
    "none",
    "nothing",
    "never",
    "without",
    "unencrypted",
];

/// What an unencrypted connection lets somebody do.
const READING: &[&str] = &["read", "see"];

/// Who it lets do it, which is somebody who is not the operator.
const SOMEBODY_ELSE: &[&str] = &["else", "other"];

/// What a starting surface says about the connection, as sentences, with the
/// address taken out of them.
///
/// The address goes first because `http` in front of it is not this product
/// saying anything. A guard that read the scheme would pass a run that had
/// deleted every word about the transport and left the address to speak for
/// itself, which is the failure this is here for.
fn about_the_connection(browser: Browser) -> Vec<String> {
    said(browser)
        .replace(&address(bound()), " ")
        .to_lowercase()
        .split(['.', ';', '\n', '\u{2014}'])
        .map(|sentence| sentence.trim().to_owned())
        .filter(|sentence| !sentence.is_empty())
        .collect()
}

/// Whether a sentence says there is none of what it is about.
fn denies(sentence: &str) -> bool {
    sentence
        .split_whitespace()
        .map(|word| word.trim_matches(|mark: char| !mark.is_alphanumeric()))
        .any(|word| DENIAL.contains(&word))
}

#[test]
fn each_desktop_is_opened_the_way_that_desktop_opens_things() {
    assert_eq!(
        opener(HostOs::MacOs, "http://127.0.0.1:8471"),
        vec!["open".to_owned(), "http://127.0.0.1:8471".to_owned()]
    );
    assert_eq!(
        opener(HostOs::Linux, "http://127.0.0.1:8471")
            .first()
            .map(String::as_str),
        Some("xdg-open")
    );
    assert_eq!(
        opener(HostOs::Other, "http://127.0.0.1:8471")
            .first()
            .map(String::as_str),
        Some("xdg-open")
    );
    let windows = opener(HostOs::Windows, "http://127.0.0.1:8471");
    assert_eq!(
        windows.len(),
        5,
        "the title it is not being given: {windows:?}"
    );
    assert_eq!(
        windows.last().map(String::as_str),
        Some("http://127.0.0.1:8471")
    );
}

#[tokio::test]
async fn a_browser_that_opens_is_reported_as_opened() {
    assert_eq!(
        opening(&exited(0), HostOs::MacOs, "http://127.0.0.1:8471").await,
        Browser::Opened
    );
}

#[test]
fn a_browser_that_will_not_open_leaves_the_address_to_open_by_hand() {
    let said = said(Browser::Unopened);
    assert!(said.contains("could not be opened"), "{said}");
    assert!(said.contains("Open the address above yourself"), "{said}");
    assert!(said.contains("http://127.0.0.1:8471"), "{said}");
}

#[tokio::test]
async fn whatever_the_browser_did_the_address_is_the_first_thing_said() {
    // The outcomes come from runners rather than being named, so these are the
    // three a run reaches: one that opened, one that would not, and one that was
    // never asked for. Line 0, because an operator whose browser did not open has
    // to find the address, and one printed below an apology is one they scroll for.
    let url = address(bound());
    let reached = [
        Browser::Unasked,
        opening(&exited(0), HostOs::Linux, &url).await,
        opening(&exited(1), HostOs::Linux, &url).await,
        opening(&missing(), HostOs::Linux, &url).await,
    ];
    for outcome in [Browser::Opened, Browser::Unopened, Browser::Unasked] {
        assert!(
            reached.contains(&outcome),
            "{outcome:?} is not among {reached:?}, so this proves less than it reads as"
        );
    }
    for browser in reached {
        assert_eq!(
            announcement(&[bound()], Offered::Machine, "000fa5ff", browser)
                .first()
                .map(|line| line.contains(&url)),
            Some(true),
            "{browser:?}"
        );
    }
}

#[test]
fn the_transport_is_stated_in_words_rather_than_left_to_the_scheme() {
    // Every outcome, because what is said about the browser is the only part of
    // this that changes and the transport is not one of the things it changes.
    for browser in [Browser::Opened, Browser::Unopened, Browser::Unasked] {
        let about = about_the_connection(browser);
        let mentioned: Vec<&String> = about
            .iter()
            .filter(|sentence| sentence.contains(ENCRYPTION))
            .collect();
        assert!(
            !mentioned.is_empty(),
            "{browser:?} leaves the transport to the scheme: {about:?}"
        );
        assert!(
            mentioned.iter().all(|sentence| denies(sentence)),
            "{browser:?} says the connection is protected: {mentioned:?}"
        );
    }
}

#[test]
fn what_being_unencrypted_costs_is_said_as_well_as_that_it_is() {
    // A fact about a protocol is not a warning. What makes it one is who it lets
    // in, and an operator told only the fact has been told nothing they can act on.
    let about = about_the_connection(Browser::Unasked);
    assert!(
        about.iter().any(|sentence| {
            READING.iter().any(|verb| sentence.contains(verb))
                && SOMEBODY_ELSE.iter().any(|who| sentence.contains(who))
        }),
        "nothing here says what being unencrypted lets anybody do: {about:?}"
    );
}

#[tokio::test]
async fn the_words_are_about_the_connection_a_run_actually_takes() {
    // The address is one really taken, through the same call a run makes, so the
    // claim is held against the connection rather than against a number written
    // down beside it. A surface that one day serves over TLS prints a different
    // scheme here, and the sentence above it has to change with it.
    let taken = held(Offered::Machine, None).await.ok();
    let checked = taken.and_then(|taken| {
        let at: Vec<SocketAddr> = taken.iter().map(|(_, bound)| *bound).collect();
        let said = announcement(&at, Offered::Machine, "000fa5ff", Browser::Unasked).join("\n");
        at.first().map(|first| {
            (
                address(*first).starts_with("http://"),
                at.iter().all(|bound| bound.ip().is_loopback()),
                at.iter().all(|bound| said.contains(&address(*bound))),
            )
        })
    });
    assert_eq!(
        checked,
        Some((true, true, true)),
        "unencrypted, reachable from nowhere else, and printed in full — the three \
         things these words claim"
    );
}

#[test]
fn it_says_it_is_reachable_from_nowhere_else() {
    assert!(said(Browser::Unasked).contains("Nothing on your network can reach it"));
}

#[test]
fn the_token_is_printed_and_said_to_be_the_only_copy() {
    let said = said(Browser::Opened);
    assert!(said.contains("000fa5ff"), "the token itself: {said}");
    assert!(said.contains("written down nowhere"), "{said}");
}

#[test]
fn it_says_how_to_stop() {
    // It holds the terminal until it is stopped, so how to stop it is part
    // of what starting it has to say.
    assert!(said(Browser::Opened).contains("Ctrl-C"));
}

#[test]
fn a_browser_that_opened_says_so_rather_than_asking_twice() {
    let said = said(Browser::Opened);
    assert!(said.contains("has been opened"), "{said}");
    assert!(!said.contains("could not be opened"), "{said}");
}

/// Giving up the network says both halves, and says which is which.
///
/// The two happen at different moments and are fixed by different things, so an
/// operator told only the end state has been told the truth and not the whole of
/// it. The interval is read from the constant that decides it rather than
/// written out here: a sentence naming five seconds beside a loop that looked
/// every thirty would be worse than one naming none.
#[test]
fn giving_up_the_network_says_what_went_at_once_and_what_went_after() {
    let said = reverted().join(" ");

    assert!(
        said.contains("from the moment it went"),
        "the authority goes at once: {said}"
    );
    assert!(
        said.contains(&format!("within {} seconds", crate::ui::LOOK.as_secs())),
        "and the socket goes at the next look, whose interval is said: {said}"
    );
    assert!(
        said.contains("this machine only"),
        "and where it ends up: {said}"
    );
}
