//! The apps a device key may come from: the official companion's identities this build
//! ships, and the operator's own builds, named in a core setting.
//!
//! An attestation names the app that made the key. Only an app named here may register
//! one, so a program on the phone that is not the companion cannot plant a key it
//! controls.

use std::borrow::Cow;
use std::path::Path;

use crate::settings::Settings;

/// The core setting the operator's own builds are named in.
pub const SETTING: &str = "LEMONFIBER_DEVICE_KEY_APPS";

/// What separates one identity from the next in the setting.
pub const SEPARATOR: char = ',';

/// What an Android identity begins with.
const ANDROID: &str = "android:";

/// What an iOS identity begins with.
const IOS: &str = "ios:";

/// What separates an Android package from its signing certificate's digest.
const SIGNED_BY: char = '@';

/// What separates an iOS Team ID from its bundle ID.
const TEAM_OF: char = '.';

/// Hex digits in a SHA-256 digest.
const DIGEST_DIGITS: usize = 64;

/// Characters in an Apple Team ID.
const TEAM_LENGTH: usize = 10;

/// The official companion's identities.
///
/// Empty until the companion's first store release; the core release that follows it
/// fills the list.
pub const OFFICIAL: &[AppIdentity] = &[];

/// One app a device key may come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIdentity {
    /// An Android app, by its package and the SHA-256 of its signing certificate.
    Android {
        /// The package, as in `app.lemonfiber.companion`.
        package: Cow<'static, str>,
        /// The SHA-256 of the signing certificate, in lower-case hex.
        certificate: Cow<'static, str>,
    },
    /// An iOS app, by its Team ID and bundle ID.
    Ios {
        /// The Team ID the app is signed under.
        team: Cow<'static, str>,
        /// The bundle ID.
        bundle: Cow<'static, str>,
    },
}

impl AppIdentity {
    /// The identity `written` names, as the setting spells one, or nothing where it is in
    /// neither form.
    #[must_use]
    pub fn read(written: &str) -> Option<Self> {
        if let Some(rest) = written.strip_prefix(ANDROID) {
            let (package, certificate) = rest.split_once(SIGNED_BY)?;
            return (is_package(package) && is_digest(certificate)).then(|| Self::Android {
                package: Cow::Owned(package.to_owned()),
                certificate: Cow::Owned(certificate.to_ascii_lowercase()),
            });
        }
        let (team, bundle) = written.strip_prefix(IOS)?.split_once(TEAM_OF)?;
        (is_team(team) && is_bundle(bundle)).then(|| Self::Ios {
            team: Cow::Owned(team.to_owned()),
            bundle: Cow::Owned(bundle.to_owned()),
        })
    }

    /// The identity as the setting spells it.
    #[must_use]
    pub fn written(&self) -> String {
        match self {
            Self::Android {
                package,
                certificate,
            } => format!("{ANDROID}{package}{SIGNED_BY}{certificate}"),
            Self::Ios { .. } => format!("{IOS}{}", self.app_id().unwrap_or_default()),
        }
    }

    /// The App ID App Attest hashes, `<Team ID>.<bundle ID>`, for an iOS identity.
    #[must_use]
    pub fn app_id(&self) -> Option<String> {
        match self {
            Self::Ios { team, bundle } => Some(format!("{team}{TEAM_OF}{bundle}")),
            Self::Android { .. } => None,
        }
    }
}

/// Every identity `value` names, and every word in it that names none.
#[must_use]
pub fn read_all(value: &str) -> (Vec<AppIdentity>, Vec<String>) {
    let mut named = Vec::new();
    let mut wrong = Vec::new();
    for word in value.split(SEPARATOR).map(str::trim) {
        match AppIdentity::read(word) {
            Some(identity) => named.push(identity),
            None => wrong.push(word.to_owned()),
        }
    }
    (named, wrong)
}

/// Every app this stack trusts now: the official ones, and the operator's own builds as
/// the settings store at `settings` holds them.
#[must_use]
pub fn trusted(settings: Option<&Path>) -> Vec<AppIdentity> {
    let added = settings
        .and_then(|path| Settings::at(path).ok())
        .and_then(|held| held.value(SETTING).map(|value| read_all(value).0))
        .unwrap_or_default();
    OFFICIAL.iter().cloned().chain(added).collect()
}

/// Whether `package` is an Android package: two or more parts joined by dots, each
/// beginning with a letter and holding letters, digits and underscores.
fn is_package(package: &str) -> bool {
    let mut parts = package.split('.');
    let every = parts.clone().all(|part| {
        part.chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && part
                .chars()
                .all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
    });
    every && parts.nth(1).is_some()
}

/// Whether `digest` is a SHA-256 written in hex.
fn is_digest(digest: &str) -> bool {
    digest.len() == DIGEST_DIGITS && digest.chars().all(|digit| digit.is_ascii_hexdigit())
}

/// Whether `team` is an Apple Team ID: ten upper-case letters and digits.
fn is_team(team: &str) -> bool {
    team.len() == TEAM_LENGTH
        && team
            .chars()
            .all(|letter| letter.is_ascii_uppercase() || letter.is_ascii_digit())
}

/// Whether `bundle` is a bundle ID: letters, digits, dots and dashes.
fn is_bundle(bundle: &str) -> bool {
    !bundle.is_empty()
        && bundle
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '.' || letter == '-')
}

#[cfg(test)]
mod tests;
