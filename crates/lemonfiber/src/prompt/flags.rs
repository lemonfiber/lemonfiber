//! Reading the command line's answers, and the words it accepts them in.
//!
//! A non-interactive run answers with flags, and every one of them has a small
//! vocabulary a person has to be told when they get it wrong. Kept together so
//! the words and the message that names them cannot drift apart.

use std::path::{Path, PathBuf};

use lemonfiber_core::alert::Appetite;
use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry, StorageWarning};
use lemonfiber_core::config::Protocols;
use lemonfiber_core::prerequisites::PrerequisiteMap;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{Library, Plan, Step, Wizard};

/// Read `--protocols` into a protocol choice, or name what was expected.
pub(super) fn parse_protocols(value: &str) -> Result<Protocols, String> {
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
pub(super) fn parse_library(value: &str) -> Result<Library, String> {
    match value.to_lowercase().as_str() {
        "docker" => Ok(Library::JellyfinDocker),
        "native" => Ok(Library::JellyfinNative),
        "none" => Ok(Library::None),
        other => Err(format!(
            "--library must be docker, native or none, not `{other}`"
        )),
    }
}

/// Read `--notifications` into an appetite, or name what was expected.
///
/// Takes the plain word rather than the stored label, since that is what an
/// operator types: `problems`, not `problems only`.
pub(super) fn parse_appetite(value: &str) -> Result<Appetite, String> {
    match value.to_lowercase().as_str() {
        "problems" => Ok(Appetite::ProblemsOnly),
        "completions" => Ok(Appetite::WithCompletions),
        "everything" => Ok(Appetite::Everything),
        other => Err(format!(
            "--notifications must be problems, completions or everything, not `{other}`"
        )),
    }
}

/// Read a `UID:GID` pair, or nothing where it is blank or malformed.
pub(super) fn parse_ids(answer: &str) -> Option<(u32, u32)> {
    let (uid, gid) = answer.split_once(':')?;
    Some((uid.trim().parse().ok()?, gid.trim().parse().ok()?))
}

/// Whether a setting's value is a secret, by its key — a password, an API key, a
/// token. Such a value is never printed, only marked present.
pub(super) fn is_secret(key: &str) -> bool {
    ["PASS", "KEY", "TOKEN", "SECRET"]
        .iter()
        .any(|mark| key.contains(mark))
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
    /// Whether a VPN carries the torrent traffic. Where torrents are chosen and
    /// this is absent, the run proceeds unprotected and records that it did.
    #[arg(long, value_name = "BOOL")]
    pub vpn: Option<bool>,
    /// Whether others in the home will use it.
    #[arg(long, value_name = "BOOL")]
    pub household: Option<bool>,
    /// What to be told about: `problems`, `completions`, or `everything`.
    #[arg(long, value_name = "APPETITE")]
    pub notifications: Option<String>,
    /// Whether to start the stack when the machine boots.
    #[arg(long, value_name = "BOOL")]
    pub autostart: Option<bool>,
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
    vpn: Option<bool>,
    household: Option<bool>,
    notifications: Option<Appetite>,
    autostart: Option<bool>,
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
            vpn: None,
            household: None,
            notifications: None,
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
            vpn: raw.vpn,
            household: raw.household,
            notifications: raw
                .notifications
                .map(|value| parse_appetite(&value))
                .transpose()?,
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
    /// step's flag is present, or where the step has a supported default answer
    /// (an indexer left unset, a container user left to the image default, a
    /// notification appetite left at the quiet preset). Requiring a flag for a
    /// question that has a safe answer is friction an unattended install pays for
    /// nothing.
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
    fn vpn(&self) -> bool {
        // A non-interactive run states it with a flag; absent, it is taken as no,
        // so a scripted torrent setup that never mentions a VPN reaches the
        // warning rather than passing silently as though one had been claimed.
        self.flags.vpn.unwrap_or(false)
    }
    fn unprotected(&self) -> bool {
        // Nobody is here to be warned, so there is nobody to confirm. The run
        // proceeds — refusing would be the one thing this must not do — and the
        // answer records that it went unprotected, which is what a later
        // diagnosis reads.
        true
    }
    fn household(&self) -> bool {
        self.flags.household.unwrap_or(false)
    }
    fn notifications(&self) -> Appetite {
        self.flags
            .notifications
            .unwrap_or_else(Appetite::default_appetite)
    }
    fn autostart(&self) -> bool {
        self.flags.autostart.unwrap_or(false)
    }
    fn confirm(&self, _plan: &Plan) -> bool {
        self.flags.yes
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry, StorageWarning};
    use lemonfiber_core::config::Protocols;

    use lemonfiber_core::prerequisites::prerequisites;
    use lemonfiber_core::validate::Validation;
    use lemonfiber_core::wizard::{Library, Step};

    use super::{is_secret, parse_ids, Flags, RawSetup, SetupFlags};
    use crate::prompt::fixtures::{answered, raw, wizard, workable};

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
    fn a_key_a_password_or_a_token_is_a_secret_by_its_name() {
        assert!(is_secret("INDEXER_KEY"));
        assert!(is_secret("USENET_PASS"));
        assert!(is_secret("SOME_TOKEN"));
        assert!(is_secret("A_SECRET"));
        assert!(!is_secret("DATA_ROOT"));
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

    #[test]
    fn each_notification_appetite_can_be_named_on_the_command_line() {
        use lemonfiber_core::alert::Appetite;
        for (given, expected) in [
            ("problems", Appetite::ProblemsOnly),
            ("completions", Appetite::WithCompletions),
            ("EVERYTHING", Appetite::Everything),
        ] {
            // Through the parse an operator's command line actually takes, so the
            // flag is proven wired and not merely parseable.
            let raw = RawSetup {
                notifications: Some(given.to_owned()),
                ..raw()
            };
            let prompt = SetupFlags::parse(raw).map(|flags| Flags::new(flags, PathBuf::new()));
            assert_eq!(
                prompt.map(|prompt| prompt.notifications()),
                Ok(expected),
                "{given}"
            );
        }
        // And a word that is none of them says which ones it expected, rather than
        // quietly falling back to a preset the operator did not ask for.
        let bad = RawSetup {
            notifications: Some("loudly".to_owned()),
            ..raw()
        };
        let refused = SetupFlags::parse(bad).err().unwrap_or_default();
        assert!(
            refused.contains("problems, completions or everything"),
            "{refused}"
        );
    }
}
