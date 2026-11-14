//! What the core and its pass service hand each other, and the passes themselves.
//!
//! Two files, in the pass service's configuration directory, both written by the core
//! and mounted read-only:
//!
//! - [`File::Key`], the key every pass is made with, written owner-only. It never leaves
//!   lemonfiber's own processes.
//! - [`File::Paths`], the media server's live-stream paths and byte paths, which say
//!   which live stream or item a request names.
//! - [`File::Renewals`], each pass the core renewed, named by a digest and never by the
//!   pass, with the moment it now lapses.
//!
//! Two keys are drawn from the key with HKDF-SHA256, one per kind of pass, so neither
//! kind is ever good as the other.
//!
//! A live-stream pass names the member it was issued to and the moment it lapses, and
//! carries an HMAC-SHA256 under the live-stream key over the live stream it is for, that
//! member and that moment, written `<member>.<lapses>.<mac>`.
//!
//! An item pass seals with AES-256-GCM under the item key, behind a nonce drawn for it,
//! the member and the token of a session on their own account, bound to the item it is
//! for and the moment it lapses, written `<nonce>.<lapses>.<sealed>`.
//!
//! Every part but the moment is base64url without padding, and the moment is in seconds
//! since the Unix epoch.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ring::{aead, hmac};
use serde::{Deserialize, Serialize};

use crate::shape::{formatted, read, unreadable, written};
use crate::{TokenHash, Unreadable};

/// The query parameter a pass travels in.
pub const PARAMETER: &str = "lfpass";

/// The header the guard names the request it forwards in, when it asks the pass service.
pub const FORWARDED: &str = "X-Forwarded-Uri";

/// The header the pass service hands the guard an item pass's session in.
pub const SESSION: &str = "Lemonfiber-Session";

/// How long a pass is good for after it is issued, in seconds.
pub const LIFETIME: u64 = 300;

/// How many bytes the key holds.
pub const KEY_BYTES: usize = 32;

/// How many bytes an item pass's nonce holds.
pub const NONCE_BYTES: usize = aead::NONCE_LEN;

/// The shape of [`Paths`] this build reads and writes.
pub const FORMAT: u32 = 1;

/// What the live-stream key is drawn from the key under.
const LIVE_INFO: &[u8] = b"lemonfiber live pass";

/// What the item key is drawn from the key under.
const ITEM_INFO: &[u8] = b"lemonfiber item pass";

/// The files in the pass service's configuration directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum File {
    /// The key every pass is made with.
    Key,
    /// The media server's live-stream paths and byte paths.
    Paths,
    /// Each pass the core renewed, and the moment it now lapses.
    Renewals,
}

impl File {
    /// The file's name in the configuration directory.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Key => "pass.key",
            Self::Paths => "paths.json",
            Self::Renewals => "renewals.json",
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

    /// The key HKDF-SHA256 draws from this one under `info`, with no salt: one block of
    /// output, `HMAC(HMAC(zeros, key), info || 1)`, which is all of it at this length.
    fn drawn(&self, info: &[u8]) -> [u8; KEY_BYTES] {
        let extracted = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &[0; KEY_BYTES]), &self.0);
        let mut block = info.to_vec();
        block.push(1);
        let expanded = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, extracted.as_ref()),
            &block,
        );
        let mut drawn = [0; KEY_BYTES];
        for (to, from) in drawn.iter_mut().zip(expanded.as_ref()) {
            *to = *from;
        }
        drawn
    }

    /// The key live-stream passes are made with.
    fn live(&self) -> hmac::Key {
        hmac::Key::new(hmac::HMAC_SHA256, &self.drawn(LIVE_INFO))
    }

    /// The key item passes are sealed with.
    fn item(&self) -> Option<aead::LessSafeKey> {
        aead::UnboundKey::new(&aead::AES_256_GCM, &self.drawn(ITEM_INFO))
            .ok()
            .map(aead::LessSafeKey::new)
    }

    /// The live-stream pass for `stream`, issued to `member` at `now`, lapsing
    /// [`LIFETIME`] seconds later.
    #[must_use]
    pub fn issue(&self, stream: &str, member: &str, now: u64) -> String {
        self.issued(stream, member, now.saturating_add(LIFETIME))
    }

    /// The live-stream pass for `stream`, issued to `member`, written to lapse at
    /// `lapses`: the same pass every time, so a renewal can answer the pass a player
    /// already holds.
    #[must_use]
    pub fn issued(&self, stream: &str, member: &str, lapses: u64) -> String {
        let tag = hmac::sign(
            &self.live(),
            &bound(&[stream.as_bytes(), member.as_bytes()], lapses),
        );
        format!(
            "{}.{lapses}.{}",
            URL_SAFE_NO_PAD.encode(member),
            URL_SAFE_NO_PAD.encode(tag.as_ref())
        )
    }

    /// Whether `pass` is a live-stream pass good for `stream` at `now`: in the one shape
    /// one is written in, made with this key for exactly that live stream, and not yet
    /// lapsed, its own moment held later where `renewals` renewed it.
    #[must_use]
    pub fn confirms(&self, pass: &str, stream: &str, now: u64, renewals: &Renewals) -> bool {
        let Some([member, lapses, tag]) = three(pass) else {
            return false;
        };
        let (Some(lapses), Ok(member), Ok(tag)) = (
            moment(lapses),
            URL_SAFE_NO_PAD.decode(member),
            URL_SAFE_NO_PAD.decode(tag),
        ) else {
            return false;
        };
        now < renewals.lapses(pass, lapses)
            && hmac::verify(
                &self.live(),
                &bound(&[stream.as_bytes(), &member], lapses),
                &tag,
            )
            .is_ok()
    }

    /// The item pass for `item`, sealing `session` behind `nonce` at `now`, lapsing
    /// [`LIFETIME`] seconds later; nothing where it cannot be sealed.
    #[must_use]
    pub fn seal(
        &self,
        item: &str,
        session: &Session,
        nonce: [u8; NONCE_BYTES],
        now: u64,
    ) -> Option<String> {
        let lapses = now.saturating_add(LIFETIME);
        let mut sealed = fielded(&[session.member.as_bytes(), session.token.as_bytes()]);
        self.item()?
            .seal_in_place_append_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(bound(&[item.as_bytes()], lapses)),
                &mut sealed,
            )
            .ok()?;
        Some(format!(
            "{}.{lapses}.{}",
            URL_SAFE_NO_PAD.encode(nonce),
            URL_SAFE_NO_PAD.encode(sealed)
        ))
    }

    /// The session `pass` seals, where it is an item pass made with this key for exactly
    /// `item`; `lapsed` says whether it lapsed at `now`, its own moment held later where
    /// `renewals` renewed it, which a renewal may still open.
    #[must_use]
    pub fn open(&self, pass: &str, item: &str, now: u64, renewals: &Renewals) -> Option<Opened> {
        let [nonce, lapses, sealed] = three(pass)?;
        let lapses = moment(lapses)?;
        let nonce = <[u8; NONCE_BYTES]>::try_from(URL_SAFE_NO_PAD.decode(nonce).ok()?).ok()?;
        let mut sealed = URL_SAFE_NO_PAD.decode(sealed).ok()?;
        let plain = self
            .item()?
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(bound(&[item.as_bytes()], lapses)),
                &mut sealed,
            )
            .ok()?;
        let [member, token] = unfielded(plain)?;
        Some(Opened {
            session: Session {
                member: String::from_utf8(member).ok()?,
                token: String::from_utf8(token).ok()?,
            },
            lapsed: now >= renewals.lapses(pass, lapses),
        })
    }
}

/// The name a renewal knows `pass` by: the SHA-256, in lower-case hex, of the part that
/// only the key could have made, its MAC or what it seals, so the renewals file holds
/// nothing that opens or presents a pass.
#[must_use]
pub fn renewed_as(pass: &str) -> Option<String> {
    let [_, _, made] = three(pass)?;
    Some(TokenHash::of(made).as_str().to_owned())
}

/// One renewed pass: the member it was issued to and the moment it now lapses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Renewed {
    /// The member the pass names, so their renewals go when they leave.
    pub member: String,
    /// The moment it now lapses, in seconds since the Unix epoch.
    pub lapses: u64,
}

/// Each pass the core renewed, by [`renewed_as`], as the pass service is told them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Renewals {
    /// The shape this file is written in; [`FORMAT`] for this build.
    pub format: u32,
    /// Each renewed pass by the name a renewal knows it by.
    pub passes: std::collections::BTreeMap<String, Renewed>,
}

impl Renewals {
    /// No renewal at all.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            format: FORMAT,
            passes: std::collections::BTreeMap::new(),
        }
    }

    /// The moment `pass`, written to lapse at `own`, lapses: the later of its own and the
    /// one a renewal names for it.
    #[must_use]
    pub fn lapses(&self, pass: &str, own: u64) -> u64 {
        renewed_as(pass)
            .and_then(|name| self.passes.get(&name))
            .map_or(own, |renewed| renewed.lapses.max(own))
    }

    /// The file as it is written to [`File::Renewals`].
    #[must_use]
    pub fn written(&self) -> String {
        written(self)
    }

    /// The renewals [`File::Renewals`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not the file, or the file in another format.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let renewals: Self = read(File::Renewals.name(), text)?;
        formatted(File::Renewals.name(), renewals.format, FORMAT)?;
        Ok(renewals)
    }
}

/// A session on a member's own account, as an item pass seals it.
#[derive(Clone, PartialEq, Eq)]
pub struct Session {
    /// The member it is on, by the id the media server files them under.
    pub member: String,
    /// The session's token.
    pub token: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("member", &self.member)
            .finish_non_exhaustive()
    }
}

/// What an item pass opened to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// The session it seals.
    pub session: Session,
    /// Whether it had lapsed when it was opened.
    pub lapsed: bool,
}

/// The three parts a pass is written in, where it is written in exactly three.
fn three(pass: &str) -> Option<[&str; 3]> {
    let mut parts = pass.split('.');
    let found = [parts.next()?, parts.next()?, parts.next()?];
    parts.next().is_none().then_some(found)
}

/// The moment `written` names, where it is written as only that moment is.
fn moment(written: &str) -> Option<u64> {
    written
        .parse::<u64>()
        .ok()
        .filter(|parsed| parsed.to_string() == written)
}

/// Each of `fields` with its length in front, so no two of them run together.
fn fielded(fields: &[&[u8]]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(
        fields.iter().map(|field| field.len()).sum::<usize>() + fields.len() * size_of::<u64>(),
    );
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field);
    }
    bytes
}

/// `fields` as [`fielded`] writes them, then `lapses`: what a pass is bound to.
fn bound(fields: &[&[u8]], lapses: u64) -> Vec<u8> {
    let mut bytes = fielded(fields);
    bytes.extend_from_slice(&lapses.to_be_bytes());
    bytes
}

/// The two fields `bytes` holds, each with its length in front, and nothing after them.
fn unfielded(bytes: &[u8]) -> Option<[Vec<u8>; 2]> {
    let mut rest = bytes;
    let mut take = || {
        let (length, after) = rest.split_first_chunk::<{ size_of::<u64>() }>()?;
        let length = usize::try_from(u64::from_be_bytes(*length)).ok()?;
        let (field, after) = (after.get(..length)?, after.get(length..)?);
        rest = after;
        Some(field.to_vec())
    };
    let found = [take()?, take()?];
    rest.is_empty().then_some(found)
}

/// One path the media server serves a live stream's bytes under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LivePath {
    /// The paths it covers, holding one group named `stream` where the live stream's id
    /// sits, or none where [`Self::parameter`] holds it.
    pub pattern: String,
    /// The query parameter whose value is the live stream's id, where the path does not
    /// hold it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    /// The shape a live stream's id has.
    pub stream: String,
}

/// One path the media server serves an item's bytes under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemPath {
    /// The paths it covers, holding one group named `item` where the item's id sits, or
    /// none where [`Self::query`] finds it.
    pub pattern: String,
    /// Where in a query value the item's id sits, where the path does not hold it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<InQuery>,
    /// The shape an item's id has.
    pub item: String,
}

/// Where in a query value an item's id sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InQuery {
    /// The parameter, carried once under this spelling.
    pub parameter: String,
    /// What its value, percent-decoded once, matches whole, holding one group named
    /// `item` where the item's id sits.
    pub pattern: String,
}

/// The media server's live-stream paths and byte paths, as the pass service is told them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paths {
    /// The shape this file is written in; [`FORMAT`] for this build.
    pub format: u32,
    /// Every live-stream path, the first that covers a path deciding it.
    pub live: Vec<LivePath>,
    /// Every byte path, the first that covers a path deciding it.
    pub bytes: Vec<ItemPath>,
}

impl Paths {
    /// The file holding `live` and `bytes`.
    #[must_use]
    pub const fn of(live: Vec<LivePath>, bytes: Vec<ItemPath>) -> Self {
        Self {
            format: FORMAT,
            live,
            bytes,
        }
    }

    /// The file as it is written to [`File::Paths`].
    #[must_use]
    pub fn written(&self) -> String {
        written(self)
    }

    /// The paths [`File::Paths`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not the file, or the file in another format.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let paths: Self = read(File::Paths.name(), text)?;
        formatted(File::Paths.name(), paths.format, FORMAT)?;
        Ok(paths)
    }
}

#[cfg(test)]
mod tests;
