//! The terminal that asks the operator setup's questions.
//!
//! This is the reading, rendering half of the wizard's [`Prompt`] port: the core
//! decides what to ask and what each answer means, and this turns a question into
//! a line on the terminal and a typed line back. It offers only the choices that
//! apply where it runs, so an answer the wizard would reject is never gathered.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use lemonfiber_core::app::setup::{CredentialChoice, Prompt, StorageWarning};
use lemonfiber_core::config::Protocols;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::prerequisites::PrerequisiteMap;
use lemonfiber_core::storage::COPY_CONSEQUENCE;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{Library, Plan, Step, Wizard};

/// A prompt that reads the operator's answers from the terminal.
pub struct Terminal {
    environment: Environment,
    default_data: PathBuf,
}

impl Terminal {
    /// A terminal prompt for `environment`, proposing `default_data` where the
    /// operator does not name a data location of their own.
    pub const fn new(environment: Environment, default_data: PathBuf) -> Self {
        Self {
            environment,
            default_data,
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
        match ask("Choose [3]:").as_str() {
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
        let _ = ask("Press enter when you have noted these.");
    }

    fn data_location(&self) -> PathBuf {
        let shown = self.default_data.display();
        let answer = ask(&format!(
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
        yes_no("\nUse this location anyway?", false)
    }

    fn credential(&self) -> Option<(String, String)> {
        println!("\nAn indexer is where the stack searches for content.");
        println!("Leave the URL blank to set one up later.");
        let url = ask("Indexer URL:");
        if url.is_empty() {
            return None;
        }
        // The key is read on its own line and never printed back — only the outcome
        // of testing it ever reaches the screen.
        let key = ask("Indexer API key:");
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
        match ask("Choose [1]:").as_str() {
            "2" => CredentialChoice::Proceed,
            "3" => CredentialChoice::Skip,
            _ => CredentialChoice::Retry,
        }
    }

    fn service_user(&self) -> Option<(u32, u32)> {
        println!("\nThe containers can run as a chosen user, so the files they create are yours.");
        parse_ids(&ask(
            "User and group as UID:GID, or blank to keep the image default:",
        ))
    }

    fn library(&self) -> Library {
        let native = self.environment.offers_native_jellyfin();
        println!("\nServe your library with Jellyfin?");
        println!("  1) Yes, in a container — works everywhere");
        if native {
            println!("  2) Yes, on the host — reaches a hardware transcoder the container cannot");
        }
        println!("  3) No media server");
        match ask("Choose [1]:").as_str() {
            "2" if native => Library::JellyfinNative,
            "3" => Library::None,
            _ => Library::JellyfinDocker,
        }
    }

    fn household(&self) -> bool {
        yes_no("\nWill others in your home use it?", false)
    }

    fn autostart(&self) -> bool {
        yes_no("\nStart the stack when this machine boots?", false)
    }

    fn confirm(&self, plan: &Plan) -> bool {
        println!("\nThis is what setup will write:");
        for (key, value) in plan.settings() {
            println!("  {key} = {value}");
        }
        yes_no("\nApply it?", true)
    }
}

/// Print a question and read the operator's trimmed answer, empty on end-of-input.
fn ask(question: &str) -> String {
    print!("{question} ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_owned()
}

/// Ask a yes-or-no question, taking the default where the answer is neither.
fn yes_no(question: &str, default: bool) -> bool {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    match ask(&format!("{question} {hint}:")).to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    }
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
    service_user: Option<(u32, u32)>,
    library: Option<Library>,
    household: Option<bool>,
    autostart: Option<bool>,
}

/// The setup flags exactly as the command line gives them, before they are typed
/// and checked — a plain carrier so the parse is one argument, not nine.
pub struct RawSetup {
    /// `--yes`: standing consent to apply without a prompt.
    pub yes: bool,
    /// `--protocols`.
    pub protocols: Option<String>,
    /// `--data-location`.
    pub data_location: Option<PathBuf>,
    /// `--indexer-url`.
    pub indexer_url: Option<String>,
    /// `--indexer-key`.
    pub indexer_key: Option<String>,
    /// `--library`.
    pub library: Option<String>,
    /// `--service-user`.
    pub service_user: Option<String>,
    /// `--household`.
    pub household: Option<bool>,
    /// `--autostart`.
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
    use super::{RawSetup, SetupFlags};
    use lemonfiber_core::platform::Environment;
    use lemonfiber_core::wizard::Wizard;

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
            .unwrap_or_else(SetupFlags::none);
        let missing = flags.missing(&wizard());
        assert!(!missing.iter().any(|flag| flag.contains("indexer")));
        assert!(!missing.iter().any(|flag| flag.contains("service-user")));
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
    }
}
