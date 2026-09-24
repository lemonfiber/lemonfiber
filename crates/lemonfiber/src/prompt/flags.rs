//! Reading the command line's answers, and the words it accepts them in.
//!
//! A non-interactive run answers with flags, and every one of them has a small
//! vocabulary a person has to be told when they get it wrong. Kept together so
//! the words and the message that names them cannot drift apart.

use std::path::{Path, PathBuf};

use lemonfiber::cli::RawSetup;
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
mod tests;
