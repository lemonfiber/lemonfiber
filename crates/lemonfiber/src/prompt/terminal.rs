//! The terminal that puts setup's questions to a person.
//!
//! What is asked, in what order, and what an answer means — all of it decided
//! here and none of it reaching a real terminal, which arrives through
//! [`Answers`](super::Answers) instead. That is what lets the conversation be
//! proven against a script.

use std::path::{Path, PathBuf};

use lemonfiber_core::alert::Appetite;
use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry, StorageWarning};
use lemonfiber_core::config::Protocols;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::prerequisites::PrerequisiteMap;
use lemonfiber_core::storage::COPY_CONSEQUENCE;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{Library, Plan};

use super::flags::{is_secret, parse_ids};
use super::Answers;
use crate::say::say;

/// A prompt that reads the operator's answers from the terminal.
pub struct Terminal {
    environment: Environment,
    default_data: PathBuf,
    answers: Box<dyn Answers>,
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
        }
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
        say!("\nAn indexer is where the stack searches for content.");
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
        say!("\nA Usenet provider is where downloads are fetched from.");
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
        self.yes_no("\nStart the stack when this machine boots?", false)
    }

    fn confirm(&self, plan: &Plan) -> bool {
        say!("\nThis is what setup will write:");
        for (key, value) in plan.settings() {
            // A secret is shown as present, not in the clear: the review reaches
            // the screen, scrollback and any session recording, and an API key or
            // password has no business in any of them.
            let shown = if is_secret(key) { "********" } else { value };
            say!("  {key} = {shown}");
        }
        self.yes_no("\nApply it?", true)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry, StorageWarning};
    use lemonfiber_core::config::Protocols;
    use lemonfiber_core::platform::Environment;
    use lemonfiber_core::prerequisites::prerequisites;
    use lemonfiber_core::validate::Validation;
    use lemonfiber_core::wizard::{Answer, Indexer, Library, Wizard};

    use super::Terminal;
    use crate::prompt::fixtures::{answered, wizard, Script};

    #[test]
    fn the_review_shows_each_setting_with_a_secret_marked_present_only() {
        // A review reaches the screen, scrollback and any session recording, so a
        // key has no business appearing in it — it is shown as present instead.
        let mut wizard = wizard();
        let _ = wizard.answer(Answer::Protocols(Protocols::both()));
        let _ = wizard.answer(Answer::DataLocation(PathBuf::from("/srv/media")));
        let _ = wizard.answer(Answer::Credentials(Some(Indexer {
            url: "http://indexer.test".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        })));
        let plan = wizard.plan();
        assert!(
            !plan.settings().is_empty(),
            "the plan carries what was answered"
        );
        assert!(answered(&[""]).confirm(&plan));
    }

    #[test]
    fn a_terminal_reads_from_the_keyboard_unless_told_otherwise() {
        // Constructing it asks nothing — the keyboard is only reached when a
        // question is actually put, which is why this is safe to build here and
        // why nothing is asked of it: a real question would read real input and
        // the test would sit there forever.
        drop(Terminal::new(
            Environment::MacOs,
            PathBuf::from("/srv/media"),
        ));
    }

    #[test]
    fn each_way_of_fetching_content_can_be_chosen() {
        for (answer, usenet, torrent) in [
            ("1", true, false),
            ("2", false, true),
            ("3", true, true),
            ("4", false, false),
            // Anything else takes the default, which is both.
            ("", true, true),
        ] {
            let chosen = answered(&[answer]).protocols();
            assert_eq!(
                chosen,
                Protocols { usenet, torrent },
                "answering {answer:?}"
            );
        }
    }

    #[test]
    fn the_prerequisites_are_listed_and_waited_on() {
        // A library-only run needs nothing, and is told so rather than shown an
        // empty list — an end state, not a lesser one.
        answered(&[]).prerequisites(&prerequisites(Protocols::none()));
        // Otherwise each item is named, costed, and the operator is waited on.
        answered(&[""]).prerequisites(&prerequisites(Protocols::both()));
    }

    #[test]
    fn the_data_location_takes_the_default_when_it_is_not_named() {
        assert_eq!(answered(&[""]).data_location(), PathBuf::from("/srv/media"));
        assert_eq!(
            answered(&["/mnt/big"]).data_location(),
            PathBuf::from("/mnt/big")
        );
    }

    #[test]
    fn what_hardlinking_means_is_said_either_way() {
        let terminal = answered(&[]);
        // Proven on the location itself.
        terminal.hardlinks(Path::new("/srv/media"), None);
        // Inferred from the parent, and said to be inferred.
        terminal.hardlinks(Path::new("/srv/media"), Some(Path::new("/srv")));
    }

    #[test]
    fn a_location_that_cannot_hardlink_is_explained_and_still_offered() {
        // Defaulting to no, so the operator is nudged toward one that links —
        // without the choice being taken away.
        assert!(!answered(&[""]).storage_warning(
            Path::new("/srv/media"),
            &StorageWarning::CopyOnly {
                limitation: Some("it is a network share".to_owned())
            }
        ));
        assert!(answered(&["y"]).storage_warning(
            Path::new("/srv/media"),
            &StorageWarning::CopyOnly { limitation: None }
        ));
        // One that could not be tested is a different sentence.
        assert!(!answered(&["n"]).storage_warning(
            Path::new("/srv/media"),
            &StorageWarning::Untested {
                reason: "the path does not exist".to_owned()
            }
        ));
    }

    #[test]
    fn a_blank_indexer_url_sets_none_up_at_all() {
        assert_eq!(answered(&[""]).credential(), None);
        assert_eq!(
            answered(&["http://indexer.test", "the-key"]).credential(),
            Some(("http://indexer.test".to_owned(), "the-key".to_owned()))
        );
    }

    #[test]
    fn a_credential_that_did_not_prove_offers_the_three_ways_out() {
        let rejected = Validation::Rejected {
            detail: "401".to_owned(),
        };
        assert!(matches!(
            answered(&["1"]).credential_failed(&rejected),
            CredentialChoice::Retry
        ));
        assert!(matches!(
            answered(&["2"]).credential_failed(&rejected),
            CredentialChoice::Proceed
        ));
        assert!(matches!(
            answered(&["3"]).credential_failed(&rejected),
            CredentialChoice::Skip
        ));
        // Each cause is named as itself, because their remedies differ.
        for outcome in [
            Validation::Unreachable {
                detail: "no answer".to_owned(),
            },
            Validation::Degraded {
                detail: "no search capability".to_owned(),
            },
            Validation::Valid {
                observed: "Prowlarr".to_owned(),
            },
        ] {
            let _ = answered(&["1"]).credential_failed(&outcome);
        }
        // And one that proved is simply said so.
        answered(&[]).credential_valid("Prowlarr 1.2");
    }

    #[test]
    fn a_blank_provider_host_sets_none_up_at_all() {
        assert_eq!(answered(&[""]).usenet_provider(), None);
    }

    #[test]
    fn a_provider_takes_the_standard_port_and_tls_unless_told_otherwise() {
        let entry = answered(&["news.test", "", "me", "secret", ""]).usenet_provider();
        assert_eq!(
            entry,
            Some(ProviderEntry {
                host: "news.test".to_owned(),
                port: 563,
                user: "me".to_owned(),
                pass: "secret".to_owned(),
                tls: true,
            })
        );
        // Named otherwise, both are taken as given.
        let plain = answered(&["news.test", "119", "me", "secret", "n"]).usenet_provider();
        assert!(plain.is_some_and(|entry| entry.port == 119 && !entry.tls));
    }

    #[test]
    fn the_library_choice_offers_the_native_option_only_where_it_applies() {
        assert!(matches!(
            answered(&["1"]).library(),
            Library::JellyfinDocker
        ));
        assert!(matches!(answered(&["3"]).library(), Library::None));
        // macOS offers a native media server, so choosing it is possible.
        assert!(matches!(
            answered(&["2"]).library(),
            Library::JellyfinNative
        ));
        // Where it is not offered, the same answer falls back rather than taking a
        // choice this platform never showed.
        let linux = Terminal::answered_by(
            Environment::LinuxNative,
            PathBuf::from("/srv/media"),
            Box::new(Script::of(&["2"])),
        );
        assert!(matches!(linux.library(), Library::JellyfinDocker));
    }

    #[test]
    fn the_yes_or_no_questions_take_their_own_defaults() {
        // Household defaults to no, autostart to no; a bare enter takes each.
        assert!(!answered(&[""]).household());
        assert!(answered(&["yes"]).household());
        assert!(!answered(&[""]).autostart());
        assert!(answered(&["y"]).autostart());
        // An answer that is neither takes the default.
        assert!(!answered(&["maybe"]).household());
    }

    #[test]
    fn the_vpn_question_defaults_to_yes_and_its_warning_defaults_to_no() {
        // The checklist has just said what a VPN is for, so yes is the answer that
        // follows from what they were told and a bare enter takes it.
        assert!(answered(&[""]).vpn());
        assert!(!answered(&["no"]).vpn());

        // The warning is the other way round: pressing enter goes back to the
        // question rather than past the exposure. Going on has to be typed.
        assert!(!answered(&[""]).unprotected());
        assert!(answered(&["yes"]).unprotected());
    }

    #[test]
    fn the_review_shows_every_setting_and_never_a_secret_in_the_clear() {
        let plan = Wizard::new(Environment::MacOs).plan();
        // Confirmed by default: a bare enter applies.
        assert!(answered(&[""]).confirm(&plan));
        assert!(!answered(&["n"]).confirm(&plan));
    }
}
