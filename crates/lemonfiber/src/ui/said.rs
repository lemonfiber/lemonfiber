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
mod tests;
