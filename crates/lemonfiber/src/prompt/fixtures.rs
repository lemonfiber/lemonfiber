//! The scripts these questions are answered from.
//!
//! Shared because both ways of answering — a terminal and the command line — are
//! proven against the same walk, and a fixture copied per module is a fixture that
//! drifts per module.

use std::cell::RefCell;
use std::path::PathBuf;

use lemonfiber::cli::RawSetup;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::wizard::Wizard;

use super::{Answers, Terminal};

/// Answers handed out in order, so a test reads as the conversation it is.
///
/// A question past the end is answered with nothing, which is what a person
/// pressing enter — or an input that has ended — gives.
pub(crate) struct Script {
    lines: RefCell<Vec<String>>,
}

impl Script {
    pub(crate) fn of(lines: &[&str]) -> Self {
        Self {
            lines: RefCell::new(lines.iter().rev().map(|line| (*line).to_owned()).collect()),
        }
    }
}

impl Answers for Script {
    fn ask(&self, _question: &str) -> String {
        self.lines.borrow_mut().pop().unwrap_or_default()
    }

    fn secret(&self, prompt: &str) -> String {
        self.ask(prompt)
    }
}

/// A terminal answered by the given script, on a platform that offers a native
/// media server so the choice that depends on it can be reached.
pub(crate) fn answered(lines: &[&str]) -> Terminal {
    Terminal::answered_by(
        Environment::MacOs,
        PathBuf::from("/srv/media"),
        Box::new(Script::of(lines)),
    )
}

/// A macOS wizard, where the container-user step does not apply, so the required
/// set is the questions that do.
pub(crate) fn wizard() -> Wizard {
    Wizard::new(Environment::MacOs)
}

/// Raw flags with nothing set, for a test to fill only what it means to.
pub(crate) fn raw() -> RawSetup {
    RawSetup {
        status: false,
        yes: false,
        protocols: None,
        data_location: None,
        indexer_url: None,
        indexer_key: None,
        usenet_host: None,
        usenet_port: None,
        usenet_user: None,
        usenet_pass: None,
        usenet_tls: None,
        library: None,
        service_user: None,
        vpn: None,
        household: None,
        notifications: None,
        autostart: None,
    }
}

/// A fully-flagged, workable run.
pub(crate) fn workable() -> RawSetup {
    RawSetup {
        yes: true,
        protocols: Some("both".to_owned()),
        data_location: Some("/srv/media".into()),
        library: Some("docker".to_owned()),
        household: Some(true),
        autostart: Some(false),
        ..raw()
    }
}
