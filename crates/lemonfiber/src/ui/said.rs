//! What a starting surface says, and what it says it with.
//!
//! The words are here and the printing is at the edge, so what an operator is told
//! is proven rather than demonstrated — and the two facts worth being told are not
//! the same on a network as they are on one machine, so the sentences are not the
//! same either.
//!
//! Asking a desktop to open a browser lives here too, because what came of that is
//! one of the things the words have to say.

use std::net::SocketAddr;

use lemonfiber_core::platform::HostOs;
use lemonfiber_core::ports::process::Runner;
use lemonfiber_core::PRODUCT;

use crate::ui::reach::{address, Offered};

/// What asking this desktop to open a browser came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Browser {
    /// One opened.
    Opened,
    /// None opened, which is not this command's failure.
    Unopened,
    /// None was asked for.
    Unasked,
}

/// The program this desktop opens an address with.
///
/// Which of the three it is comes from the one module allowed to know what this
/// machine is, rather than being asked here.
pub(crate) fn opener(host: HostOs, url: &str) -> Vec<String> {
    let argv: &[&str] = match host {
        HostOs::MacOs => &["open"],
        // An empty first argument, which `start` reads as the window title it is
        // not being given. Without it the address becomes the title and nothing
        // opens.
        HostOs::Windows => &["cmd", "/c", "start", ""],
        HostOs::Linux | HostOs::Other => &["xdg-open"],
    };
    argv.iter()
        .map(|word| (*word).to_owned())
        .chain(std::iter::once(url.to_owned()))
        .collect()
}

/// Ask this desktop to open the address, and say what came of it.
///
/// Every way of not opening one is the same answer. A desktop with no browser, a
/// machine with no desktop and a program that exited badly all leave the operator
/// with an address to open themselves, and none of them is a reason for the
/// command to have failed.
pub(crate) async fn opening(runner: &dyn Runner, host: HostOs, url: &str) -> Browser {
    let ran = runner.run(&opener(host, url)).await;
    if ran.is_ok_and(|output| output.succeeded()) {
        Browser::Opened
    } else {
        Browser::Unopened
    }
}

/// What starting the surface says, in order.
///
/// The transport is stated as a sentence rather than left to the scheme in the
/// address. `http` in front of a name is a fact an operator has no reason to be
/// able to read, and what it costs them is the thing worth saying out loud — and
/// what it costs them is not the same on a network as it is on one machine, so the
/// sentence is not the same either.
///
/// Every address that was actually taken is listed, rather than the one that was
/// meant: on a machine whose IPv6 wildcard already answers for IPv4 there is one
/// socket, and being told about two would be being told about a socket nothing
/// holds.
pub(crate) fn announcement(
    at: &[SocketAddr],
    offered: Offered,
    token: &str,
    browser: Browser,
) -> Vec<String> {
    // One address is a sentence and several are a list. A machine whose IPv6
    // wildcard already answers for IPv4 has one socket, and being handed a list of
    // one would be being handed a shape that exists for somebody else's machine.
    let mut lines = match at {
        [only] => vec![format!("{PRODUCT} is serving at {}", address(*only))],
        several => std::iter::once(format!("{PRODUCT} is serving at:"))
            .chain(several.iter().map(|bound| format!("  {}", address(*bound))))
            .collect(),
    };
    lines.push(String::new());
    lines.extend(transport(offered));
    lines.push(String::new());
    lines.push("The token for this run, which the page will ask you for:".to_owned());
    lines.push(format!("  {token}"));
    lines.push("It is kept in memory, written down nowhere, and gone when this stops.".to_owned());
    lines.push(String::new());
    // Singular where there is one and plural where there are several, because a
    // sentence pointing at "the address above" over a list of two is pointing at
    // nothing in particular.
    let above = match at {
        [_] => "the address above",
        _ => "one of the addresses above",
    };
    lines.push(match browser {
        Browser::Opened => "A browser has been opened at that address.".to_owned(),
        Browser::Unopened => {
            format!("A browser could not be opened here. Open {above} yourself.")
        }
        Browser::Unasked => format!("Open {above} in a browser."),
    });
    lines.push(format!("Stop {PRODUCT} with Ctrl-C when you are finished."));
    lines
}

/// What being reachable where it is reachable costs, said plainly.
///
/// Two sentences on one machine and four on a network, because the facts are
/// different and the second set is the one nobody would guess. A certificate is
/// named among them rather than quietly not being there: an operator who knows
/// browsers complain about plain text will wonder why this one does not offer to
/// stop them, and the answer is worth the line.
pub(super) fn transport(offered: Offered) -> Vec<String> {
    match offered {
        Offered::Network => vec![
            "This connection is not encrypted. Anything between a device and this machine can \
             read what passes over it, including the password as it is typed in."
                .to_owned(),
            format!(
                "{PRODUCT} does not make a certificate of its own for this. One it made would \
                 be one your browser warns you about, and learning to click past that warning \
                 costs you more than plain text on a network you trust."
            ),
            "Anything on your network can reach this, which is what you asked for. It asks \
             whoever opens it for the password you set."
                .to_owned(),
        ],
        Offered::Machine | Offered::Refused => vec![
            "This connection is not encrypted. Anything else running on this machine can read \
             what passes over it."
                .to_owned(),
            "Nothing on your network can reach it — it listens on this machine and nowhere \
             else."
                .to_owned(),
        ],
    }
}

/// What giving up a network binding says.
///
/// Said rather than left to be noticed: an operator whose devices stop reaching this
/// would otherwise go looking at their network, and what changed was here.
///
/// **Both halves, because they happen at different moments.** Nothing on the network
/// is admitted from the instant the password goes — every request is checked against
/// the credential as it stands now, so that half is immediate and needs no interval.
/// The socket is a different thing: it is given up at the next look, which is up to
/// five seconds later. An operator told only the end state has been told the truth
/// and not the whole of it, and the difference is exactly what somebody asking "was
/// it really immediate?" needs.
pub(crate) fn reverted() -> Vec<String> {
    vec![
        String::new(),
        format!("The password for {PRODUCT}'s web interface is gone, so this is no longer offered to your network."),
        "Nothing on the network was let in from the moment it went — every request is \
         checked against the password as it stands, and there is none."
            .to_owned(),
        format!(
            "The socket itself is given up at the next check, within {} seconds, after which \
             this listens on this machine only.",
            super::LOOK.as_secs()
        ),
    ]
}

#[cfg(test)]
mod tests {
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
}
