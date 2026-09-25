//! The terminal that puts setup's questions to a person.
//!
//! What is asked, in what order, and what an answer means — all of it decided
//! here and none of it reaching a real terminal, which arrives through
//! [`Answers`](super::Answers) instead. That is what lets the conversation be
//! proven against a script.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use lemonfiber_core::alert::Appetite;
use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry, StorageWarning};
use lemonfiber_core::autostart::DECLINE_CONSEQUENCE;
use lemonfiber_core::config::Protocols;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::prerequisites::PrerequisiteMap;
use lemonfiber_core::storage::COPY_CONSEQUENCE;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{Library, Plan};

use lemonfiber_core::config::display::in_full;

use super::flags::parse_ids;
use super::Answers;
use crate::say::say;

/// A prompt that reads the operator's answers from the terminal.
pub struct Terminal {
    environment: Environment,
    default_data: PathBuf,
    answers: Box<dyn Answers>,
    /// Words this conversation has already explained.
    met: RefCell<Vec<&'static str>>,
    /// Whether this run explains its words at all.
    explaining: bool,
}

impl Terminal {
    /// A terminal prompt for `environment`, proposing `default_data` where the
    /// operator does not name a data location of their own.
    pub fn new(environment: Environment, default_data: PathBuf) -> Self {
        Self::answered_by(
            environment,
            default_data,
            Box::new(crate::keyboard::Keyboard),
        )
    }

    /// The same, reading its answers from somewhere else — a script, in a test.
    pub fn answered_by(
        environment: Environment,
        default_data: PathBuf,
        answers: Box<dyn Answers>,
    ) -> Self {
        Self {
            environment,
            default_data,
            answers,
            met: RefCell::new(Vec::new()),
            explaining: crate::render::glossary::wanted(),
        }
    }

    /// The same prompt, explaining nothing — for a run that asked for none.
    ///
    /// A builder rather than a latch read inside `introduce`, so a test can have
    /// both kinds of conversation without settling a value that outlives it.
    #[cfg(test)]
    pub(crate) const fn explaining_nothing(mut self) -> Self {
        self.explaining = false;
        self
    }

    /// Explain a word, once, the first time this conversation uses it.
    ///
    /// Worth reading the first time and noise every time after — the rule the
    /// footnote block follows inside one report. A setup is one long report whose
    /// parts arrive as questions, so the rule carries across them rather than
    /// starting again at each.
    ///
    /// The words come from the glossary rather than from here. Two of them were
    /// written in this file — "an indexer is where the stack searches for content"
    /// — and a second copy of an explanation is one that drifts from the first.
    /// That copy was also a definition, where what somebody needs is what the thing
    /// is for and what it will cost them.
    fn introduce(&self, word: &str) {
        if !self.explaining {
            return;
        }
        let Some(term) = lemonfiber_core::glossary::explain(word) else {
            return;
        };
        if !self.first_meeting(term.word) {
            return;
        }
        crate::render::glossary::introduced(term).print();
    }

    /// Whether this conversation is meeting the word for the first time, recording
    /// it if so.
    ///
    /// Its own function so the borrow ends with it. Showing the word is what the
    /// caller does next, and doing that while still holding the list would be the
    /// only way this could be asked twice at once — so keeping the two apart is
    /// what makes the borrow safe rather than a check that it was.
    fn first_meeting(&self, word: &'static str) -> bool {
        let mut met = self.met.borrow_mut();
        if met.contains(&word) {
            return false;
        }
        met.push(word);
        true
    }

    /// Ask a yes-or-no question, taking the default where the answer is neither.
    fn yes_no(&self, question: &str, default: bool) -> bool {
        let hint = if default { "[Y/n]" } else { "[y/N]" };
        match self
            .answers
            .ask(&format!("{question} {hint}:"))
            .to_lowercase()
            .as_str()
        {
            "y" | "yes" => true,
            "n" | "no" => false,
            _ => default,
        }
    }
}

impl Prompt for Terminal {
    fn protocols(&self) -> Protocols {
        say!("\nHow will you fetch content?");
        say!("  1) Usenet only");
        say!("  2) Torrents only");
        say!("  3) Both");
        say!("  4) Neither — serve an existing library only");
        match self.answers.ask("Choose [3]:").as_str() {
            "1" => Protocols {
                usenet: true,
                torrent: false,
            },
            "2" => Protocols {
                usenet: false,
                torrent: true,
            },
            "4" => Protocols {
                usenet: false,
                torrent: false,
            },
            _ => Protocols::both(),
        }
    }

    fn prerequisites(&self, map: &PrerequisiteMap) {
        // Nothing required is stated first and plainly — a folder of existing media
        // reaching a working Jellyfin with no accounts is an end state, not a lesser
        // one. Otherwise each thing is named, explained, costed in a band, and given
        // the criteria that decide it — no vendors, since those age and vary.
        if let Some(note) = map.library_only {
            say!("\n{note}");
            return;
        }

        say!("\nBefore the questions that follow, here is what your choices will need.");
        say!("You can go and get these, then run setup again — it remembers your answers.\n");
        for item in &map.items {
            say!("  {}", item.label);
            say!("    What it is: {}", item.what);
            say!("    Why:        {}", item.why);
            say!("    Cost:       {}", item.cost.phrase());
            say!("    Look for:");
            for criterion in &item.criteria {
                say!("      · {criterion}");
            }
            say!("    Without it: {}\n", item.without);
        }
        let _ = self.answers.ask("Press enter when you have noted these.");
    }

    fn data_location(&self) -> PathBuf {
        let shown = self.default_data.display();
        let answer = self.answers.ask(&format!(
            "\nWhere should the library and downloads live? [{shown}]:"
        ));
        if answer.is_empty() {
            self.default_data.clone()
        } else {
            PathBuf::from(answer)
        }
    }

    fn hardlinks(&self, path: &Path, inferred_from: Option<&Path>) {
        match inferred_from {
            // Tested directly: the chosen location itself proved it links.
            None => say!(
                "  ✓ {} hardlinks — imports will be instant and cost no extra disk.",
                path.display()
            ),
            // The location does not exist yet, so its parent's filesystem stood in
            // for it. Say so, rather than present a parent's answer as proven of a
            // path never touched — a separate drive mounted here later could differ,
            // and the storage check re-tests the real location once it exists.
            Some(parent) => say!(
                "  ✓ {} will hardlink — its filesystem ({}) does. If it becomes a \
                 separate drive, that is checked when the stack first runs.",
                path.display(),
                parent.display()
            ),
        }
    }

    fn storage_warning(&self, path: &Path, warning: &StorageWarning) -> bool {
        match warning {
            StorageWarning::CopyOnly { limitation } => {
                match limitation {
                    Some(reason) => say!("  ✗ {} cannot hardlink — {reason}.", path.display()),
                    None => say!("  ✗ {} cannot hardlink.", path.display()),
                }
                // The consequence is stated in the same words a later diagnosis
                // would use, indented so it reads as the explanation of the line
                // above rather than a new claim.
                say!("    {COPY_CONSEQUENCE}");
            }
            StorageWarning::Untested { reason } => {
                say!(
                    "  ? {} could not be tested for hardlinks — {reason}.",
                    path.display()
                );
            }
        }
        // Defaulting to no nudges the operator toward a location that links,
        // without taking the choice away — some know their setup and mean it.
        self.yes_no("\nUse this location anyway?", false)
    }

    fn credential(&self) -> Option<(String, String)> {
        say!("");
        self.introduce("indexer");
        say!("Leave the URL blank to set one up later.");
        let url = self.answers.ask("Indexer URL:");
        if url.is_empty() {
            return None;
        }
        // Read without echo and never printed back — the review redacts it, so the
        // key reaches neither the screen as it is typed nor the summary after.
        let key = self.answers.secret("Indexer API key:");
        Some((url, key))
    }

    fn credential_valid(&self, observed: &str) {
        say!("  ✓ {observed}");
    }

    fn credential_failed(&self, outcome: &Validation) -> CredentialChoice {
        // Each cause is named as itself, because their remedies differ — a wrong
        // key, a host that did not answer, and an account that cannot do the job
        // send the operator to three different places.
        match outcome {
            Validation::Rejected { detail } => say!("  ✗ Rejected — {detail}"),
            Validation::Unreachable { detail } => say!("  ✗ Unreachable — {detail}"),
            Validation::Degraded { detail } => say!("  ! Degraded — {detail}"),
            // The proven case never reaches here; setup keeps it rather than asking.
            Validation::Valid { observed } => say!("  ✓ {observed}"),
        }
        say!("\nWhat would you like to do?");
        say!("  1) Try again — re-enter it and test afresh");
        say!("  2) Use it anyway — keep it unverified");
        say!("  3) Skip — leave the indexer unset for now");
        match self.answers.ask("Choose [1]:").as_str() {
            "2" => CredentialChoice::Proceed,
            "3" => CredentialChoice::Skip,
            _ => CredentialChoice::Retry,
        }
    }

    fn usenet_provider(&self) -> Option<ProviderEntry> {
        say!("");
        self.introduce("usenet");
        say!("Leave the host blank to set one up later.");
        let host = self.answers.ask("Provider host:");
        if host.is_empty() {
            return None;
        }
        // 563 is the standard TLS port; TLS is the default because the password
        // must not cross the wire in the clear.
        let port = self.answers.ask("Port [563]:").parse().unwrap_or(563);
        let user = self.answers.ask("Username:");
        // Read without echo and never printed back — the review redacts it.
        let pass = self.answers.secret("Password:");
        let tls = self.yes_no("Connect over TLS?", true);
        Some(ProviderEntry {
            host,
            port,
            user,
            pass,
            tls,
        })
    }

    fn service_user(&self) -> Option<(u32, u32)> {
        say!("\nThe containers can run as a chosen user, so the files they create are yours.");
        parse_ids(
            &self
                .answers
                .ask("User and group as UID:GID, or blank to keep the image default:"),
        )
    }

    fn library(&self) -> Library {
        let native = self.environment.offers_native_jellyfin();
        say!("\nServe your library with Jellyfin?");
        say!("  1) Yes, in a container — works everywhere");
        if native {
            say!("  2) Yes, on the host — reaches a hardware transcoder the container cannot");
        }
        say!("  3) No media server");
        match self.answers.ask("Choose [1]:").as_str() {
            "2" if native => Library::JellyfinNative,
            "3" => Library::None,
            _ => Library::JellyfinDocker,
        }
    }

    fn vpn(&self) -> bool {
        // Defaulted to yes: the checklist has just explained what a VPN is for and
        // why torrents want one, so yes is the answer that follows from what they
        // were told. Nothing is assumed from the default — a no is asked about.
        self.yes_no("\nWill a VPN carry your torrent traffic?", true)
    }

    fn unprotected(&self) -> bool {
        // Said plainly and in the second person, because this is the one
        // consequence of the protocol choice that cannot be discovered afterwards:
        // by the time it matters, the address has already been seen.
        say!(
            "\nWithout a VPN, every peer you exchange torrent data with sees your \
             home address. That includes anyone watching a swarm to record who is in it."
        );
        // Defaulted to no, so pressing enter goes back to the question rather than
        // past the warning. Going on has to be typed.
        self.yes_no("Set up torrents without a VPN anyway?", false)
    }

    fn household(&self) -> bool {
        self.yes_no("\nWill others in your home use it?", false)
    }

    fn notifications(&self) -> Appetite {
        // Three presets rather than a checklist of thirteen events: an operator
        // setting up a media stack has no basis for deciding whether they want to
        // hear about a degraded hardlink, and every event stays switchable later.
        say!("\nWhat should lemonfiber tell you about?");
        for (index, preset) in Appetite::ALL.iter().enumerate() {
            say!(
                "  {}) {} — {}",
                index + 1,
                preset.label(),
                preset.describe()
            );
        }
        match self.answers.ask("Choose [1]:").as_str() {
            "2" => Appetite::WithCompletions,
            "3" => Appetite::Everything,
            _ => Appetite::default_appetite(),
        }
    }

    fn autostart(&self) -> bool {
        // The consequence goes before the question, not after it, because after it
        // there is nothing left to decide. "Start on boot" sounds like a
        // convenience, and an operator who reads it that way says no to keep their
        // machine quiet — and then loses the stack to the next operating-system
        // update with nothing anywhere to tell them. Said in the same words a later
        // report would use, from the one place they are written.
        say!("\n{DECLINE_CONSEQUENCE}");
        // Defaulted to no all the same. The sentence above is what makes a no an
        // informed answer rather than the absence of one, and it is a reasonable
        // answer — a laptop that is not always on has no business starting a media
        // stack at login. Nothing here decides for them.
        self.yes_no("Start the stack when this machine boots?", false)
    }

    fn confirm(&self, plan: &Plan) -> bool {
        say!("\nThis is what setup will write:");
        for line in reviewed(plan) {
            say!("{line}");
        }
        self.yes_no("\nApply it?", true)
    }
}

/// What is shown in place of a secret: that there is one, and nothing more.
const MASKED: &str = "********";

/// The review of what setup will write, as the lines it puts on the screen.
///
/// A value is shown only where somebody has written down what makes showing it safe:
/// the review reaches the screen, scrollback and any session recording, and an API
/// key or password has no business in any of them. Decided by the same allow-list
/// `config show` displays by, so the two surfaces cannot disagree about one setting.
///
/// Built rather than printed straight out, so that what it says can be read back — a
/// mask nothing reads is a mask that can stop masking silently.
fn reviewed(plan: &Plan) -> Vec<String> {
    plan.settings()
        .iter()
        .map(|(key, value)| {
            let shown = if in_full(key) { value.as_str() } else { MASKED };
            format!("  {key} = {shown}")
        })
        .collect()
}

#[cfg(test)]
mod tests;
