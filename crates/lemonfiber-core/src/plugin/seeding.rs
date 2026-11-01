//! What a plugin's services start with: the files its source ships for them, the secrets
//! lemonfiber mints written into those files, and the fields an adapter reads its
//! upstream's credentials from.
//!
//! A starting file names a secret as a placeholder, so the file as shipped, as planned and
//! as shown holds the placeholder and only the file on the machine holds the value. What
//! is written there is decided when it is written, from the secret store, which is why a
//! plan carries the file as shipped.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::path::Path;

use base64::Engine as _;
use lemonfiber_contract::adapter::{MINTED_DOMAIN, MINTED_HEX};
use lemonfiber_manifest::container::Minted;
use lemonfiber_plugin::seeding::{filled, placeholders, Form, MOST_PLACEHOLDERS};
use lemonfiber_plugin::{Manifest, Violation};
use lemonfiber_ports::random::Random;

use super::placing::{configuration, Lands, Write};
use super::program::{shipped as held, Starting};

/// How many random bytes a minted secret is made of, each written as two of its
/// [`MINTED_HEX`] hexadecimal digits.
pub(crate) const MINTED_BYTES: usize = MINTED_HEX / 2;

/// How many rounds the hashed form of a secret is derived over.
const ROUNDS: NonZeroU32 = match NonZeroU32::new(100_000) {
    Some(rounds) => rounds,
    None => NonZeroU32::MIN,
};

/// How many random bytes salt the hashed form of a secret.
const SALT_BYTES: usize = 16;

/// What is said where the machine would not supply the random bytes a secret is made of.
pub(crate) const UNMINTED: &str =
    "this machine would not supply the random bytes a secret is made from";

/// One file a plugin's source ships for one of its services.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shipped {
    /// The service whose configuration directory it goes in.
    pub service: String,
    /// Where it goes beneath that directory.
    pub path: String,
    /// What it holds, as shipped.
    pub bytes: Vec<u8>,
}

/// Every file the plugin's source at `source` ships for each of `services`.
///
/// # Errors
///
/// Why one service's starting files could not be read, as a copy for a proof refuses them.
pub fn shipped<'a>(
    services: impl IntoIterator<Item = &'a str>,
    source: &Path,
) -> Result<Vec<Shipped>, String> {
    let mut found = Vec::new();
    for service in services {
        let starting = Starting {
            root: source,
            service,
        };
        found.extend(
            held(Some(starting))?
                .into_iter()
                .map(|(path, bytes)| Shipped {
                    service: service.to_owned(),
                    path,
                    bytes,
                }),
        );
    }
    Ok(found)
}

/// Every file the plugin's source at `source` ships for each service `installed` runs.
///
/// # Errors
///
/// As [`shipped`].
pub(crate) fn shipped_by(
    installed: &super::Installed,
    source: &Path,
) -> Result<Vec<Shipped>, String> {
    shipped(
        installed
            .services
            .iter()
            .map(|placed| placed.service.as_str()),
        source,
    )
}

/// What is wrong with any of the starting files `shipped` against `manifest`, each file on
/// its own and each service's files together.
#[must_use]
pub fn refused(manifest: &Manifest, shipped: &[Shipped]) -> Vec<Violation> {
    shipped
        .iter()
        .flat_map(|file| {
            lemonfiber_plugin::seeding::refused(manifest, &file.service, &file.path, &file.bytes)
        })
        .chain(crowded(shipped))
        .chain(covered(manifest, shipped))
        .collect()
}

/// Each starting file shipped where its service's declared certificate or key is written.
fn covered(manifest: &Manifest, shipped: &[Shipped]) -> Vec<Violation> {
    shipped
        .iter()
        .filter(|file| {
            manifest
                .services
                .iter()
                .filter(|service| service.id == file.service)
                .filter_map(|service| service.certificate.as_ref())
                .any(|laid| file.path == laid.key || file.path == laid.certificate)
        })
        .map(|file| Violation {
            location: format!("service {}.certificate", file.service),
            message: format!(
                "{} is a starting file, where lemonfiber writes the certificate it declares",
                file.path
            ),
        })
        .collect()
}

/// Each service whose starting files hold more placeholders in all than one may.
fn crowded(shipped: &[Shipped]) -> Vec<Violation> {
    let mut counted: BTreeMap<&str, usize> = BTreeMap::new();
    for file in shipped {
        *counted.entry(&file.service).or_default() += named(&file.bytes);
    }
    counted
        .into_iter()
        .filter(|(_, count)| *count > MOST_PLACEHOLDERS)
        .map(|(service, count)| Violation {
            location: format!("config/{service}"),
            message: format!(
                "holds {count} placeholders, past the {MOST_PLACEHOLDERS} a service's starting \
                 files may hold"
            ),
        })
        .collect()
}

/// How many placeholders `bytes` hold, none where they are not text or cannot be read.
fn named(bytes: &[u8]) -> usize {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| placeholders(text).ok())
        .map_or(0, |named| named.len())
}

/// Whether `bytes` name a secret, and so are written with its value, owner-only.
#[must_use]
pub fn seeded(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| placeholders(text).ok())
        .is_some_and(|named| !named.is_empty())
}

/// A write for each shipped file whose place in the stack `present` does not already hold.
///
/// A file already there is the service's own by then: it rewrites its settings as it
/// runs, and a reinstall or a part moved in place keeps what it holds.
#[must_use]
pub fn starting(
    shipped: Vec<Shipped>,
    stack: &Path,
    present: &dyn Fn(&Path) -> bool,
) -> Vec<Write> {
    shipped
        .into_iter()
        .map(|file| Write {
            path: configuration(stack, &file.service).join(&file.path),
            lands: Lands::Starting {
                seeded: seeded(&file.bytes),
                bytes: file.bytes,
            },
        })
        .filter(|write| !present(&write.path))
        .collect()
}

/// A secret lemonfiber mints: 32 lowercase hexadecimal digits from `random`, or nothing
/// where it supplies no bytes.
#[must_use]
pub(crate) fn mint(random: &dyn Random) -> Option<String> {
    random
        .bytes(MINTED_BYTES)
        .map(|bytes| crate::secret::render(&bytes))
}

/// A secret the plugin `plugin` has lemonfiber mint in `shape`: a key as [`mint`] makes
/// one, a password of [`PASSWORD_LENGTH`] characters holding one of each of [`CLASSES`], or
/// a key at `<plugin>.invalid` as an email address; nothing where it is not minted or
/// `random` supplies no bytes.
#[must_use]
pub(crate) fn minted_as(random: &dyn Random, shape: Minted, plugin: &str) -> Option<String> {
    match shape {
        Minted::Not => None,
        Minted::Key => mint(random),
        Minted::Password => password(random),
        Minted::Email => mint(random).map(|key| format!("{key}@{plugin}{MINTED_DOMAIN}")),
    }
}

/// How many characters a minted password holds.
const PASSWORD_LENGTH: usize = 32;

/// The character classes a minted password holds one of each of, and draws the rest from:
/// the symbols are those every environment file, URL and form takes as written.
const CLASSES: [&[u8]; 4] = [
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    b"abcdefghijklmnopqrstuvwxyz",
    b"0123456789",
    lemonfiber_contract::adapter::PASSWORD_MARKS,
];

/// A password of [`PASSWORD_LENGTH`] characters drawn from every one of [`CLASSES`], each
/// class then set at a distinct place, two bytes of `random` to each choice.
fn password(random: &dyn Random) -> Option<String> {
    let alphabet: Vec<u8> = CLASSES.concat();
    let drawn = random.bytes(2 * (PASSWORD_LENGTH + 2 * CLASSES.len()))?;
    let mut choices = drawn.chunks_exact(2).map(|pair| {
        Some(usize::from(u16::from_be_bytes([
            *pair.first()?,
            *pair.get(1)?,
        ])))
    });
    let mut chars = Vec::with_capacity(PASSWORD_LENGTH);
    for _ in 0..PASSWORD_LENGTH {
        chars.push(*alphabet.get(choices.next()?? % alphabet.len())?);
    }
    let mut places: Vec<usize> = (0..PASSWORD_LENGTH).collect();
    for (at, class) in CLASSES.iter().enumerate() {
        let place = at + choices.next()?? % (PASSWORD_LENGTH - at);
        places.swap(at, place);
        *chars.get_mut(*places.get(at)?)? = *class.get(choices.next()?? % class.len())?;
    }
    String::from_utf8(chars).ok()
}

/// The plugin a manifest declares and the shape each secret it declares is minted in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Shapes {
    /// The plugin's id.
    plugin: String,
    /// Each secret's shape, by its id.
    of: BTreeMap<String, Minted>,
}

impl Shapes {
    /// The shape of `secret`, a key where it names none.
    #[must_use]
    pub(crate) fn of(&self, secret: &str) -> Minted {
        self.of
            .get(secret)
            .copied()
            .filter(|shape| shape.is())
            .unwrap_or(Minted::Key)
    }

    /// A value of `secret` minted in its shape from `random`.
    #[must_use]
    pub(crate) fn minted(&self, random: &dyn Random, secret: &str) -> Option<String> {
        minted_as(random, self.of(secret), &self.plugin)
    }

    /// The plugin `plugin` declaring each secret of `of` in its shape.
    #[cfg(test)]
    pub(crate) fn declaring(plugin: &str, of: &[(&str, Minted)]) -> Self {
        Self {
            plugin: plugin.to_owned(),
            of: of
                .iter()
                .map(|(id, shape)| ((*id).to_owned(), *shape))
                .collect(),
        }
    }
}

/// The plugin the manifest at `root` declares and the shape of each secret it declares:
/// nothing where no manifest is there or it does not read.
#[must_use]
pub(crate) fn shapes(root: &Path) -> Shapes {
    std::fs::read_to_string(root.join(lemonfiber_bundle::MANIFEST))
        .ok()
        .and_then(|text| Manifest::from_toml(&text).ok())
        .map(|manifest| Shapes {
            plugin: manifest.plugin.id,
            of: manifest
                .secrets
                .into_iter()
                .map(|secret| (secret.id, secret.minted))
                .collect(),
        })
        .unwrap_or_default()
}

/// `value` as a service keeping only a hash of its password reads it: PBKDF2-HMAC-SHA512
/// over a fresh salt, `@ByteArray(<salt>:<hash>)` with both in base64.
fn hashed(value: &str, random: &dyn Random) -> Option<String> {
    let salt = random.bytes(SALT_BYTES)?;
    let mut hash = [0_u8; ring::digest::SHA512_OUTPUT_LEN];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA512,
        ROUNDS,
        &salt,
        value.as_bytes(),
        &mut hash,
    );
    let encoded = base64::engine::general_purpose::STANDARD;
    Some(format!(
        "@ByteArray({}:{})",
        encoded.encode(&salt),
        encoded.encode(hash)
    ))
}

/// `bytes` with each secret they name written as `value` holds it, in the form named.
///
/// # Errors
///
/// A placeholder naming a secret `value` holds nothing for, or a hash with no random
/// bytes to salt it, named by the placeholder rather than by any value.
pub(crate) fn written(
    bytes: &[u8],
    value: &dyn Fn(&str) -> Option<String>,
    random: &dyn Random,
) -> Result<Vec<u8>, String> {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) if seeded(bytes) => text,
        _ => return Ok(bytes.to_vec()),
    };
    filled(text, |placeholder| {
        let held = value(placeholder.secret).ok_or_else(|| {
            format!(
                "a placeholder names the secret {}, which nothing holds",
                placeholder.secret
            )
        })?;
        match placeholder.form {
            Form::Plain => Ok(held),
            Form::Pbkdf2 => hashed(&held, random).ok_or_else(|| UNMINTED.to_owned()),
        }
    })
    .map(String::into_bytes)
}

/// `current` with the value on each line `template` puts a placeholder naming `secret`
/// on replaced by `value`, in the form that placeholder names, and every other byte kept;
/// nothing where `template` names no such placeholder.
///
/// The line is found by the text around the placeholder on its line in `template`, which
/// has to stand on exactly one line of `current` and has to be some text.
///
/// # Errors
///
/// Where `template` or `current` is not text, the text around a placeholder is none or
/// is found on no line or on more than one, or a hash has no random bytes to salt it.
pub(crate) fn replaced(
    template: &[u8],
    current: &[u8],
    secret: &str,
    value: &str,
    random: &dyn Random,
) -> Result<Option<Vec<u8>>, String> {
    let shipped = std::str::from_utf8(template).map_err(|_| NOT_TEXT.to_owned())?;
    let named: Vec<_> = placeholders(shipped)?
        .into_iter()
        .filter(|placeholder| placeholder.secret == secret)
        .collect();
    if named.is_empty() {
        return Ok(None);
    }
    let mut now = std::str::from_utf8(current)
        .map_err(|_| NOT_TEXT.to_owned())?
        .to_owned();
    for placeholder in named {
        let before = shipped[..placeholder.span.start]
            .rsplit('\n')
            .next()
            .unwrap_or_default();
        let after = shipped[placeholder.span.end..]
            .split('\n')
            .next()
            .unwrap_or_default();
        if before.trim().is_empty() && after.trim().is_empty() {
            return Err(format!(
                "the placeholder for {secret} stands alone on its line, so nothing finds where its \
                 value is now"
            ));
        }
        let written = match placeholder.form {
            Form::Plain => value.to_owned(),
            Form::Pbkdf2 => hashed(value, random).ok_or_else(|| UNMINTED.to_owned())?,
        };
        now = anchored(&now, before, after, &written).ok_or_else(|| {
            format!("the line holding {secret} is not where the service keeps it now")
        })?;
    }
    Ok(Some(now.into_bytes()))
}

/// `text` with the one line that starts with `before` and ends with `after` holding
/// `written` between them, or nothing where no line or more than one does.
fn anchored(text: &str, before: &str, after: &str, written: &str) -> Option<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let mut found = lines.iter_mut().filter(|line| {
        line.len() >= before.len() + after.len()
            && line.starts_with(before)
            && line.ends_with(after)
    });
    let line = found.next()?;
    *line = format!("{before}{written}{after}");
    if found.next().is_some() {
        return None;
    }
    Some(lines.join("\n"))
}

/// What is said of a starting file that is not text where a secret is to be written in it.
const NOT_TEXT: &str = "it is not text, so where a secret stands in it cannot be read";

/// The `upstream.json` an adapter reads: each of `fields` with its secret's value, a
/// field whose secret is among the `awaited` ones left out while nothing holds it.
///
/// # Errors
///
/// A field whose secret `value` holds nothing for and that is not awaited, named by
/// the field.
pub(crate) fn upstream(
    fields: &BTreeMap<String, String>,
    awaited: &BTreeSet<String>,
    value: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    let held: Result<serde_json::Map<String, serde_json::Value>, String> = fields
        .iter()
        .filter_map(|(field, secret)| match value(secret) {
            Some(held) => Some(Ok((field.clone(), serde_json::Value::String(held)))),
            None if awaited.contains(secret) => None,
            None => Some(Err(format!(
                "{field} names the secret {secret}, which nothing holds"
            ))),
        })
        .collect();
    held.map(|held| serde_json::Value::Object(held).to_string())
}

#[cfg(test)]
mod tests;
