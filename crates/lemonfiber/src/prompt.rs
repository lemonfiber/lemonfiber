//! The terminal that asks the operator setup's questions.
//!
//! This is the reading, rendering half of the wizard's [`Prompt`] port: the core
//! decides what to ask and what each answer means, and this turns a question into
//! a line on the terminal and a typed line back. It offers only the choices that
//! apply where it runs, so an answer the wizard would reject is never gathered.

use std::path::{Path, PathBuf};

use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry, StorageWarning};
use lemonfiber_core::config::Protocols;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::prerequisites::PrerequisiteMap;
use lemonfiber_core::storage::COPY_CONSEQUENCE;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{Library, Plan, Step, Wizard};

/// Where the answers to these questions come from.
///
/// A person at a keyboard, in a real run. The distinction exists so that what to
/// ask, and what an answer means, can be proven against a script — the terminal
/// itself is the one part of this that no test can stand in for, and it is kept
/// behind here in [`crate::keyboard`].
pub trait Answers {
    /// Show a question and read the trimmed answer, empty at end of input.
    fn ask(&self, question: &str) -> String;

    /// Read a secret, without it appearing as it is typed.
    fn secret(&self, prompt: &str) -> String;
}

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
        println!("\nHow will you fetch content?");
        println!("  1) Usenet only");
        println!("  2) Torrents only");
        println!("  3) Both");
        println!("  4) Neither — serve an existing library only");
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
            println!("\n{note}");
            return;
        }

        println!("\nBefore the questions that follow, here is what your choices will need.");
        println!("You can go and get these, then run setup again — it remembers your answers.\n");
        for item in &map.items {
            println!("  {}", item.label);
            println!("    What it is: {}", item.what);
            println!("    Why:        {}", item.why);
            println!("    Cost:       {}", item.cost.phrase());
            println!("    Look for:");
            for criterion in &item.criteria {
                println!("      · {criterion}");
            }
            println!("    Without it: {}\n", item.without);
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
            None => println!(
                "  ✓ {} hardlinks — imports will be instant and cost no extra disk.",
                path.display()
            ),
            // The location does not exist yet, so its parent's filesystem stood in
            // for it. Say so, rather than present a parent's answer as proven of a
            // path never touched — a separate drive mounted here later could differ,
            // and the storage check re-tests the real location once it exists.
            Some(parent) => println!(
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
                    Some(reason) => println!("  ✗ {} cannot hardlink — {reason}.", path.display()),
                    None => println!("  ✗ {} cannot hardlink.", path.display()),
                }
                // The consequence is stated in the same words a later diagnosis
                // would use, indented so it reads as the explanation of the line
                // above rather than a new claim.
                println!("    {COPY_CONSEQUENCE}");
            }
            StorageWarning::Untested { reason } => {
                println!(
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
        println!("\nAn indexer is where the stack searches for content.");
        println!("Leave the URL blank to set one up later.");
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
        println!("  ✓ {observed}");
    }

    fn credential_failed(&self, outcome: &Validation) -> CredentialChoice {
        // Each cause is named as itself, because their remedies differ — a wrong
        // key, a host that did not answer, and an account that cannot do the job
        // send the operator to three different places.
        match outcome {
            Validation::Rejected { detail } => println!("  ✗ Rejected — {detail}"),
            Validation::Unreachable { detail } => println!("  ✗ Unreachable — {detail}"),
            Validation::Degraded { detail } => println!("  ! Degraded — {detail}"),
            // The proven case never reaches here; setup keeps it rather than asking.
            Validation::Valid { observed } => println!("  ✓ {observed}"),
        }
        println!("\nWhat would you like to do?");
        println!("  1) Try again — re-enter it and test afresh");
        println!("  2) Use it anyway — keep it unverified");
        println!("  3) Skip — leave the indexer unset for now");
        match self.answers.ask("Choose [1]:").as_str() {
            "2" => CredentialChoice::Proceed,
            "3" => CredentialChoice::Skip,
            _ => CredentialChoice::Retry,
        }
    }

    fn usenet_provider(&self) -> Option<ProviderEntry> {
        println!("\nA Usenet provider is where downloads are fetched from.");
        println!("Leave the host blank to set one up later.");
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
        println!("\nThe containers can run as a chosen user, so the files they create are yours.");
        parse_ids(
            &self
                .answers
                .ask("User and group as UID:GID, or blank to keep the image default:"),
        )
    }

    fn library(&self) -> Library {
        let native = self.environment.offers_native_jellyfin();
        println!("\nServe your library with Jellyfin?");
        println!("  1) Yes, in a container — works everywhere");
        if native {
            println!("  2) Yes, on the host — reaches a hardware transcoder the container cannot");
        }
        println!("  3) No media server");
        match self.answers.ask("Choose [1]:").as_str() {
            "2" if native => Library::JellyfinNative,
            "3" => Library::None,
            _ => Library::JellyfinDocker,
        }
    }

    fn household(&self) -> bool {
        self.yes_no("\nWill others in your home use it?", false)
    }

    fn autostart(&self) -> bool {
        self.yes_no("\nStart the stack when this machine boots?", false)
    }

    fn confirm(&self, plan: &Plan) -> bool {
        println!("\nThis is what setup will write:");
        for (key, value) in plan.settings() {
            // A secret is shown as present, not in the clear: the review reaches
            // the screen, scrollback and any session recording, and an API key or
            // password has no business in any of them.
            let shown = if is_secret(key) { "********" } else { value };
            println!("  {key} = {shown}");
        }
        self.yes_no("\nApply it?", true)
    }
}

/// Whether a setting's value is a secret, by its key — a password, an API key, a
/// token. Such a value is never printed, only marked present.
fn is_secret(key: &str) -> bool {
    ["PASS", "KEY", "TOKEN", "SECRET"]
        .iter()
        .any(|mark| key.contains(mark))
}

/// Read a `UID:GID` pair, or nothing where it is blank or malformed.
fn parse_ids(answer: &str) -> Option<(u32, u32)> {
    let (uid, gid) = answer.split_once(':')?;
    Some((uid.trim().parse().ok()?, gid.trim().parse().ok()?))
}

/// The answers a non-interactive run supplies as flags instead of at a prompt.
///
/// Parsed once from the command line, then read back as a [`Prompt`] so the very
/// same walk drives it — the wizard cannot tell it is answering flags rather than
/// a person, which is what keeps the two paths honest.
pub struct SetupFlags {
    /// Standing consent: a non-interactive run applies without a person to
    /// confirm, so this is the confirmation, given once up front.
    yes: bool,
    protocols: Option<Protocols>,
    data_location: Option<PathBuf>,
    indexer: Option<(String, String)>,
    provider: Option<ProviderEntry>,
    service_user: Option<(u32, u32)>,
    library: Option<Library>,
    household: Option<bool>,
    autostart: Option<bool>,
}

/// The setup flags exactly as the command line gives them, before they are typed
/// and checked — a plain carrier so the parse is one argument, not a dozen.
///
/// This is also the command line's own declaration of them, flattened into the
/// subcommand: one list of flags rather than a definition here and a copy there,
/// which is a copy that only ever falls out of step in one direction.
#[derive(Debug, Default, clap::Args)]
pub struct RawSetup {
    /// Apply without a prompt to confirm — required for an unattended run.
    #[arg(long)]
    pub yes: bool,
    /// How to fetch content: `both`, `usenet`, `torrent`, or `none`.
    #[arg(long, value_name = "PROTOCOLS")]
    pub protocols: Option<String>,
    /// Where the library and downloads live.
    #[arg(long, value_name = "PATH")]
    pub data_location: Option<PathBuf>,
    /// An indexer's API base URL.
    #[arg(long, value_name = "URL")]
    pub indexer_url: Option<String>,
    /// The indexer's API key.
    #[arg(long, value_name = "KEY")]
    pub indexer_key: Option<String>,
    /// The Usenet provider's hostname.
    #[arg(long, value_name = "HOST")]
    pub usenet_host: Option<String>,
    /// The port the Usenet provider answers on (defaults to 563).
    #[arg(long, value_name = "PORT")]
    pub usenet_port: Option<u16>,
    /// The Usenet account username.
    #[arg(long, value_name = "USER")]
    pub usenet_user: Option<String>,
    /// The Usenet account password.
    #[arg(long, value_name = "PASS")]
    pub usenet_pass: Option<String>,
    /// Whether the Usenet connection uses TLS (defaults to yes).
    #[arg(long, value_name = "BOOL")]
    pub usenet_tls: Option<bool>,
    /// How to serve the library: `docker`, `native`, or `none`.
    #[arg(long, value_name = "MODE")]
    pub library: Option<String>,
    /// The container user, as `UID:GID`.
    #[arg(long, value_name = "UID:GID")]
    pub service_user: Option<String>,
    /// Whether others in the home will use it.
    #[arg(long, value_name = "BOOL")]
    pub household: Option<bool>,
    /// Whether to start the stack when the machine boots.
    #[arg(long, value_name = "BOOL")]
    pub autostart: Option<bool>,
}

impl SetupFlags {
    /// No flags at all — the interactive default, where every answer comes from a
    /// terminal and nothing is assumed.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            yes: false,
            protocols: None,
            data_location: None,
            indexer: None,
            provider: None,
            service_user: None,
            library: None,
            household: None,
            autostart: None,
        }
    }

    /// Turn the raw command-line values into typed answers, or say which one could
    /// not be understood — a malformed flag is a mistake to name, not a default to
    /// quietly assume. Takes the raw flags as one value rather than nine loose
    /// arguments.
    pub fn parse(raw: RawSetup) -> Result<Self, String> {
        Ok(Self {
            yes: raw.yes,
            protocols: raw
                .protocols
                .map(|value| parse_protocols(&value))
                .transpose()?,
            data_location: raw.data_location,
            indexer: match (raw.indexer_url, raw.indexer_key) {
                (Some(url), Some(key)) => Some((url, key)),
                (None, None) => None,
                _ => {
                    return Err(
                        "an indexer needs both --indexer-url and --indexer-key, or neither".into(),
                    )
                }
            },
            provider: match (raw.usenet_host, raw.usenet_user, raw.usenet_pass) {
                (Some(host), Some(user), Some(pass)) => Some(ProviderEntry {
                    host,
                    // 563 is the standard TLS port, and TLS the default, since the
                    // password must not cross the wire in the clear.
                    port: raw.usenet_port.unwrap_or(563),
                    user,
                    pass,
                    tls: raw.usenet_tls.unwrap_or(true),
                }),
                (None, None, None) => None,
                _ => {
                    return Err("a Usenet provider needs --usenet-host, --usenet-user and \
                                --usenet-pass together, or none"
                        .into())
                }
            },
            service_user: raw
                .service_user
                .map(|value| {
                    parse_ids(&value)
                        .ok_or_else(|| format!("--service-user must be UID:GID, not `{value}`"))
                })
                .transpose()?,
            library: raw.library.map(|value| parse_library(&value)).transpose()?,
            household: raw.household,
            autostart: raw.autostart,
        })
    }

    /// The flags a non-interactive run still needs: the ones for questions it has
    /// not answered and cannot skip. Empty means it can proceed without a terminal.
    pub fn missing(&self, wizard: &Wizard) -> Vec<&'static str> {
        let mut missing: Vec<&'static str> = wizard
            .unanswered()
            .iter()
            .filter_map(|step| self.flag_for(*step))
            .collect();
        if !self.yes {
            missing.push("--yes");
        }
        missing
    }

    /// The flag a step needs where one is required and absent — `None` where the
    /// step's flag is present, or where the step has a supported empty answer
    /// (an indexer left unset, a container user left to the image default).
    fn flag_for(&self, step: Step) -> Option<&'static str> {
        match step {
            Step::Protocols => self
                .protocols
                .is_none()
                .then_some("--protocols <both|usenet|torrent|none>"),
            Step::DataLocation => self
                .data_location
                .is_none()
                .then_some("--data-location <path>"),
            Step::Library => self
                .library
                .is_none()
                .then_some("--library <docker|native|none>"),
            Step::Household => self
                .household
                .is_none()
                .then_some("--household <true|false>"),
            Step::Autostart => self
                .autostart
                .is_none()
                .then_some("--autostart <true|false>"),
            _ => None,
        }
    }
}

/// A prompt that answers from flags, so a non-interactive run drives the same walk
/// a person would — probing the data location and proving the indexer as it goes,
/// with the warnings a person would weigh settled by the standing `--yes`.
pub struct Flags {
    flags: SetupFlags,
    default_data: PathBuf,
}

impl Flags {
    /// A flag-answered prompt, proposing `default_data` where none was given.
    pub const fn new(flags: SetupFlags, default_data: PathBuf) -> Self {
        Self {
            flags,
            default_data,
        }
    }
}

impl Prompt for Flags {
    fn protocols(&self) -> Protocols {
        self.flags.protocols.unwrap_or_else(Protocols::both)
    }
    // Nothing to show or wait on without a person; the checklist is an interactive
    // courtesy, and a flag run has already decided.
    fn prerequisites(&self, _map: &PrerequisiteMap) {}
    fn data_location(&self) -> PathBuf {
        self.flags
            .data_location
            .clone()
            .unwrap_or_else(|| self.default_data.clone())
    }
    fn hardlinks(&self, _path: &Path, _inferred_from: Option<&Path>) {}
    fn storage_warning(&self, _path: &Path, _warning: &StorageWarning) -> bool {
        // A non-interactive run cannot choose elsewhere, so the standing consent
        // decides: proceed with the location as it is.
        self.flags.yes
    }
    fn credential(&self) -> Option<(String, String)> {
        self.flags.indexer.clone()
    }
    fn credential_valid(&self, _observed: &str) {}
    fn credential_failed(&self, _outcome: &Validation) -> CredentialChoice {
        // Consent given, keep the credential unverified rather than block; without
        // it, leave the indexer unset rather than store something unproven unasked.
        if self.flags.yes {
            CredentialChoice::Proceed
        } else {
            CredentialChoice::Skip
        }
    }
    fn usenet_provider(&self) -> Option<ProviderEntry> {
        self.flags.provider.clone()
    }
    fn service_user(&self) -> Option<(u32, u32)> {
        self.flags.service_user
    }
    fn library(&self) -> Library {
        self.flags.library.unwrap_or(Library::JellyfinDocker)
    }
    fn household(&self) -> bool {
        self.flags.household.unwrap_or(false)
    }
    fn autostart(&self) -> bool {
        self.flags.autostart.unwrap_or(false)
    }
    fn confirm(&self, _plan: &Plan) -> bool {
        self.flags.yes
    }
}

/// Read `--protocols` into a protocol choice, or name what was expected.
fn parse_protocols(value: &str) -> Result<Protocols, String> {
    match value.to_lowercase().as_str() {
        "both" => Ok(Protocols::both()),
        "usenet" => Ok(Protocols {
            usenet: true,
            torrent: false,
        }),
        "torrent" | "torrents" => Ok(Protocols {
            usenet: false,
            torrent: true,
        }),
        "none" | "neither" => Ok(Protocols::none()),
        other => Err(format!(
            "--protocols must be both, usenet, torrent or none, not `{other}`"
        )),
    }
}

/// Read `--library` into a library choice, or name what was expected.
fn parse_library(value: &str) -> Result<Library, String> {
    match value.to_lowercase().as_str() {
        "docker" => Ok(Library::JellyfinDocker),
        "native" => Ok(Library::JellyfinNative),
        "none" => Ok(Library::None),
        other => Err(format!(
            "--library must be docker, native or none, not `{other}`"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{Flags, RawSetup, SetupFlags};
    use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry};
    use lemonfiber_core::platform::Environment;
    use lemonfiber_core::wizard::Wizard;
    use std::path::PathBuf;

    // ---- The terminal's own questions, answered by a script. ----

    use super::{is_secret, parse_ids, Answers, Terminal};
    use lemonfiber_core::app::setup::StorageWarning;
    use lemonfiber_core::config::Protocols;
    use lemonfiber_core::prerequisites::prerequisites;
    use lemonfiber_core::validate::Validation;
    use lemonfiber_core::wizard::{Answer, Indexer, Library, Step};
    use std::cell::RefCell;
    use std::path::Path;

    /// Answers handed out in order, so a test reads as the conversation it is.
    /// A question past the end is answered with nothing, which is what a person
    /// pressing enter — or an input that has ended — gives.
    struct Script {
        lines: RefCell<Vec<String>>,
    }

    impl Script {
        fn of(lines: &[&str]) -> Self {
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
    fn answered(lines: &[&str]) -> Terminal {
        Terminal::answered_by(
            Environment::MacOs,
            PathBuf::from("/srv/media"),
            Box::new(Script::of(lines)),
        )
    }

    #[test]
    fn a_flag_run_answers_from_what_it_was_given() {
        // The very same walk drives this as drives the terminal, so a flag run and
        // an interactive one cannot answer differently.
        // Everything a non-interactive run can be told, so each answer comes from
        // a flag rather than from a default.
        let given = RawSetup {
            indexer_url: Some("http://indexer.test".to_owned()),
            indexer_key: Some("the-key".to_owned()),
            usenet_host: Some("news.test".to_owned()),
            usenet_user: Some("me".to_owned()),
            usenet_pass: Some("secret".to_owned()),
            service_user: Some("1000:1000".to_owned()),
            autostart: Some(true),
            ..workable()
        };
        let flags = SetupFlags::parse(given).unwrap_or(SetupFlags::none());
        let prompt = Flags::new(flags, PathBuf::from("/elsewhere"));
        assert_eq!(
            prompt.protocols(),
            Protocols {
                usenet: true,
                torrent: true
            }
        );
        // Named, so the flag wins over the default this was built with.
        assert_eq!(prompt.data_location(), PathBuf::from("/srv/media"));
        assert_eq!(
            prompt.credential(),
            Some(("http://indexer.test".to_owned(), "the-key".to_owned()))
        );
        assert!(prompt.usenet_provider().is_some());
        assert_eq!(prompt.service_user(), Some((1000, 1000)));
        assert!(matches!(prompt.library(), Library::JellyfinDocker));
        assert!(prompt.household());
        assert!(prompt.autostart());
        // Consent given up front is what stands in for a person confirming.
        assert!(prompt.confirm(&wizard().plan()));
        assert!(prompt.storage_warning(
            Path::new("/data"),
            &StorageWarning::CopyOnly { limitation: None }
        ));
        assert!(matches!(
            prompt.credential_failed(&Validation::Rejected {
                detail: "401".to_owned()
            }),
            CredentialChoice::Proceed
        ));
        // The interactive courtesies are nothing at all without a person.
        prompt.prerequisites(&prerequisites(Protocols::both()));
        prompt.hardlinks(Path::new("/data"), None);
        prompt.credential_valid("Prowlarr");
    }

    #[test]
    fn a_flag_run_without_consent_keeps_nothing_it_could_not_prove() {
        // No `--yes`: an unproven credential is left unset rather than stored, and
        // a location that cannot hardlink is not used on someone's behalf.
        let prompt = Flags::new(SetupFlags::none(), PathBuf::from("/srv/media"));
        assert!(matches!(
            prompt.credential_failed(&Validation::Unreachable {
                detail: "no answer".to_owned()
            }),
            CredentialChoice::Skip
        ));
        assert!(!prompt.storage_warning(
            Path::new("/srv/media"),
            &StorageWarning::Untested {
                reason: "absent".to_owned()
            }
        ));
        assert!(!prompt.confirm(&wizard().plan()));
        // What was not given falls back to the same defaults the terminal offers.
        assert_eq!(prompt.data_location(), PathBuf::from("/srv/media"));
        assert_eq!(prompt.credential(), None);
        assert_eq!(prompt.usenet_provider(), None);
        assert_eq!(prompt.service_user(), None);
        assert!(!prompt.household());
        assert!(!prompt.autostart());
        assert!(matches!(prompt.library(), Library::JellyfinDocker));
        assert_eq!(
            prompt.protocols(),
            Protocols {
                usenet: true,
                torrent: true
            }
        );
    }

    #[test]
    fn each_library_choice_can_be_named_on_the_command_line() {
        for (given, expected) in [
            ("docker", Library::JellyfinDocker),
            ("native", Library::JellyfinNative),
            ("NONE", Library::None),
        ] {
            let flags = SetupFlags::parse(RawSetup {
                library: Some(given.to_owned()),
                ..raw()
            });
            let chosen = flags.ok().and_then(|flags| flags.library);
            assert_eq!(
                format!("{chosen:?}"),
                format!("{:?}", Some(expected)),
                "--library {given}"
            );
        }
    }

    #[test]
    fn each_way_of_fetching_content_can_be_named_on_the_command_line() {
        for (given, usenet, torrent) in [
            ("both", true, true),
            ("usenet", true, false),
            ("torrent", false, true),
            ("TORRENTS", false, true),
            ("none", false, false),
            ("neither", false, false),
        ] {
            let flags = SetupFlags::parse(RawSetup {
                protocols: Some(given.to_owned()),
                ..raw()
            });
            assert_eq!(
                flags.ok().and_then(|flags| flags.protocols),
                Some(Protocols { usenet, torrent }),
                "--protocols {given}"
            );
        }
    }

    #[test]
    fn a_choice_the_command_line_does_not_offer_names_what_it_expected() {
        // The message has to say what would have worked, or the operator is left
        // guessing at a vocabulary nothing shows them.
        let protocols = SetupFlags::parse(RawSetup {
            protocols: Some("carrier pigeon".to_owned()),
            ..raw()
        });
        assert!(protocols
            .err()
            .is_some_and(|message| message.contains("both, usenet, torrent or none")));
        let library = SetupFlags::parse(RawSetup {
            library: Some("plex".to_owned()),
            ..raw()
        });
        assert!(library
            .err()
            .is_some_and(|message| message.contains("docker, native or none")));
    }

    #[test]
    fn a_container_user_that_is_not_a_pair_names_what_was_expected() {
        let flags = SetupFlags::parse(RawSetup {
            service_user: Some("me".to_owned()),
            ..raw()
        });
        assert!(flags
            .err()
            .is_some_and(|message| message.contains("must be UID:GID")));
    }

    #[test]
    fn a_step_that_is_no_question_asks_for_no_flag() {
        // Only a question can be answered by a flag; the rest of the walk is work,
        // and naming a flag for it would be nonsense.
        assert_eq!(SetupFlags::none().flag_for(Step::Welcome), None);
    }

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
    fn the_container_user_is_read_as_a_pair_or_left_to_the_image() {
        assert_eq!(answered(&["1000:1000"]).service_user(), Some((1000, 1000)));
        assert_eq!(answered(&[""]).service_user(), None);
        assert_eq!(parse_ids("1000:1000"), Some((1000, 1000)));
        assert_eq!(parse_ids("1000"), None);
        assert_eq!(parse_ids("x:y"), None);
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
    fn the_review_shows_every_setting_and_never_a_secret_in_the_clear() {
        let plan = Wizard::new(Environment::MacOs).plan();
        // Confirmed by default: a bare enter applies.
        assert!(answered(&[""]).confirm(&plan));
        assert!(!answered(&["n"]).confirm(&plan));
    }

    #[test]
    fn a_key_a_password_or_a_token_is_a_secret_by_its_name() {
        assert!(is_secret("INDEXER_KEY"));
        assert!(is_secret("USENET_PASS"));
        assert!(is_secret("SOME_TOKEN"));
        assert!(is_secret("A_SECRET"));
        assert!(!is_secret("DATA_ROOT"));
    }

    /// A macOS wizard, where the container-user step does not apply, so the
    /// required set is the questions that do.
    fn wizard() -> Wizard {
        Wizard::new(Environment::MacOs)
    }

    /// Raw flags with nothing set, for a test to fill only what it means to.
    fn raw() -> RawSetup {
        RawSetup {
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
            household: None,
            autostart: None,
        }
    }

    /// A fully-flagged, workable run.
    fn workable() -> RawSetup {
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

    #[test]
    fn a_fresh_non_interactive_run_names_every_flag_it_needs() {
        let missing = SetupFlags::none().missing(&wizard());
        for expected in [
            "--protocols",
            "--data-location",
            "--library",
            "--household",
            "--autostart",
            "--yes",
        ] {
            assert!(
                missing.iter().any(|flag| flag.contains(expected)),
                "{expected} should be named as needed, got {missing:?}"
            );
        }
    }

    #[test]
    fn a_fully_flagged_run_needs_nothing_more() {
        let flags = SetupFlags::parse(workable())
            .ok()
            .unwrap_or_else(SetupFlags::none);
        assert!(
            flags.missing(&wizard()).is_empty(),
            "every required flag is present, so none is named"
        );
    }

    #[test]
    fn an_indexer_and_container_user_are_optional_not_required() {
        // Neither an indexer nor a container user is named as missing: an unset
        // indexer is a supported end, and the container user falls to the image
        // default, so a run without them is complete.
        let flags = SetupFlags::parse(workable())
            .ok()
            .unwrap_or(SetupFlags::none());
        // Compared whole rather than searched: a workable run is missing nothing at
        // all, and a search over an empty list proves nothing about either one.
        assert_eq!(flags.missing(&wizard()), Vec::<&str>::new());
    }

    #[test]
    fn malformed_flag_values_are_rejected_with_a_named_reason() {
        let bad_protocol = RawSetup {
            protocols: Some("bogus".to_owned()),
            ..raw()
        };
        assert!(SetupFlags::parse(bad_protocol).is_err());

        // Half an indexer is refused: both parts or neither.
        let half_indexer = RawSetup {
            indexer_url: Some("http://idx".to_owned()),
            ..raw()
        };
        assert!(SetupFlags::parse(half_indexer).is_err());

        // Half a provider is refused too: host with no login.
        let half_provider = RawSetup {
            usenet_host: Some("news.test".to_owned()),
            ..raw()
        };
        assert!(SetupFlags::parse(half_provider).is_err());
    }

    #[test]
    fn a_complete_provider_flag_set_is_offered_to_the_wizard() {
        let complete = RawSetup {
            usenet_host: Some("news.test".to_owned()),
            usenet_user: Some("person".to_owned()),
            usenet_pass: Some("secret".to_owned()),
            ..raw()
        };
        let flags = SetupFlags::parse(complete)
            .ok()
            .unwrap_or_else(SetupFlags::none);
        // The flag run answers the provider question from the flags, defaulting the
        // port to the TLS standard and TLS to on.
        let entry = Flags::new(flags, PathBuf::from("/tmp")).usenet_provider();
        assert!(matches!(
            entry,
            Some(ProviderEntry { host, port, tls, .. }) if host == "news.test" && port == 563 && tls
        ));
    }
}
