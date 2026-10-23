//! What the core and its pass service hand each other, and the pass itself.
//!
//! Two files, in the pass service's configuration directory, both written by the core
//! and mounted read-only:
//!
//! - [`File::Key`], the key every pass is made with, written owner-only. It never leaves
//!   lemonfiber's own processes.
//! - [`File::Live`], the media server's live-stream paths, which say where a request's
//!   path names a live stream.
//!
//! A pass names the member it was issued to and the moment it lapses, and carries an
//! HMAC-SHA256 under the key over the live stream it is for, that member and that moment.
//! It is written `<member>.<lapses>.<mac>`, the member and the MAC in base64url without
//! padding and the moment in seconds since the Unix epoch.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ring::hmac;
use serde::{Deserialize, Serialize};

use crate::shape::{formatted, read, unreadable, written};
use crate::Unreadable;

/// The query parameter a pass travels in.
pub const PARAMETER: &str = "lfpass";

/// How long a pass is good for after it is issued, in seconds.
pub const LIFETIME: u64 = 300;

/// How many bytes the key holds.
pub const KEY_BYTES: usize = 32;

/// The shape of [`Live`] this build reads and writes.
pub const FORMAT: u32 = 1;

/// The files in the pass service's configuration directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum File {
    /// The key every pass is made with.
    Key,
    /// The media server's live-stream paths.
    Live,
}

impl File {
    /// The file's name in the configuration directory.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Key => "pass.key",
            Self::Live => "live.json",
        }
    }
}

/// The key every pass is made with.
#[derive(Clone, PartialEq, Eq)]
pub struct Key([u8; KEY_BYTES]);

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(..)")
    }
}

impl Key {
    /// The key holding `bytes`, drawn from a source of randomness by whoever mints it.
    #[must_use]
    pub const fn of(bytes: [u8; KEY_BYTES]) -> Self {
        Self(bytes)
    }

    /// The key as it is written to [`File::Key`]: base64url without padding, then a
    /// newline.
    #[must_use]
    pub fn written(&self) -> String {
        let mut text = URL_SAFE_NO_PAD.encode(self.0);
        text.push('\n');
        text
    }

    /// The key [`File::Key`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not one key of [`KEY_BYTES`] bytes.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let refused = || {
            unreadable(
                File::Key.name(),
                "it is not one key written in base64url without padding",
            )
        };
        let bytes = URL_SAFE_NO_PAD
            .decode(text.trim_end_matches('\n'))
            .map_err(|_| refused())?;
        bytes.try_into().map(Self).map_err(|_| refused())
    }

    /// The HMAC-SHA256 under this key over `stream`, `member` and `lapses`, each field
    /// written with its length in front so no two of them run together.
    fn tag(&self, stream: &str, member: &str, lapses: u64) -> hmac::Tag {
        hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, &self.0),
            &signed(stream, member, lapses),
        )
    }

    /// The pass for `stream`, issued to `member` at `now`, in seconds since the Unix
    /// epoch, lapsing [`LIFETIME`] seconds later.
    #[must_use]
    pub fn issue(&self, stream: &str, member: &str, now: u64) -> String {
        let lapses = now.saturating_add(LIFETIME);
        format!(
            "{}.{lapses}.{}",
            URL_SAFE_NO_PAD.encode(member),
            URL_SAFE_NO_PAD.encode(self.tag(stream, member, lapses).as_ref())
        )
    }

    /// Whether `pass` is good for `stream` at `now`, in seconds since the Unix epoch: in
    /// the one shape a pass is written in, made with this key for exactly that live
    /// stream, and not yet lapsed.
    #[must_use]
    pub fn confirms(&self, pass: &str, stream: &str, now: u64) -> bool {
        let mut parts = pass.split('.');
        let (Some(member), Some(lapses), Some(tag), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return false;
        };
        let Some(lapses) = lapses
            .parse::<u64>()
            .ok()
            .filter(|parsed| parsed.to_string() == lapses)
        else {
            return false;
        };
        let (Ok(member), Ok(tag)) = (URL_SAFE_NO_PAD.decode(member), URL_SAFE_NO_PAD.decode(tag))
        else {
            return false;
        };
        let Ok(member) = String::from_utf8(member) else {
            return false;
        };
        now < lapses
            && hmac::verify(
                &hmac::Key::new(hmac::HMAC_SHA256, &self.0),
                &signed(stream, &member, lapses),
                &tag,
            )
            .is_ok()
    }
}

/// What a pass's MAC is taken over: each field with its length in front.
fn signed(stream: &str, member: &str, lapses: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(stream.len() + member.len() + 24);
    for field in [stream.as_bytes(), member.as_bytes()] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field);
    }
    bytes.extend_from_slice(&lapses.to_be_bytes());
    bytes
}

/// One path the media server serves a live stream's bytes under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LivePath {
    /// The paths it covers, holding one group named `stream` where the live stream's id
    /// sits.
    pub pattern: String,
    /// The shape a live stream's id has.
    pub stream: String,
}

/// The media server's live-stream paths, as the pass service is told them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Live {
    /// The shape this file is written in; [`FORMAT`] for this build.
    pub format: u32,
    /// Every live-stream path, the first that covers a path deciding it.
    pub paths: Vec<LivePath>,
}

impl Live {
    /// The file holding `paths`.
    #[must_use]
    pub const fn of(paths: Vec<LivePath>) -> Self {
        Self {
            format: FORMAT,
            paths,
        }
    }

    /// The file as it is written to [`File::Live`].
    #[must_use]
    pub fn written(&self) -> String {
        written(self)
    }

    /// The live-stream paths [`File::Live`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not the file, or the file in another format.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let live: Self = read(File::Live.name(), text)?;
        formatted(File::Live.name(), live.format, FORMAT)?;
        Ok(live)
    }
}

#[cfg(test)]
mod tests;
