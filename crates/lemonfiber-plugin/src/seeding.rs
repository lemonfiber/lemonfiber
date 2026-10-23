//! The secrets a plugin's starting files name, and where each is written in them.
//!
//! A starting file names a secret the plugin holds as `{{secret:<id>}}`, and lemonfiber
//! writes the value there; `{{secret:<id>|pbkdf2}}` asks for the salted hash a service
//! that keeps only a hash of its password reads. This reads the placeholders out of a
//! file and holds them to the manifest. Writing a value, and making the hash, belong to
//! `lemonfiber-core`, which holds the values and the random source.

use std::ops::Range;

use crate::schema::Manifest;
use crate::Violation;

/// What a placeholder begins with.
pub const OPENS: &str = "{{secret:";

/// What a placeholder ends with.
pub const CLOSES: &str = "}}";

/// What separates the secret a placeholder names from the form it asks for.
pub const FORMED: char = '|';

/// The form a placeholder asks its secret to be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// The value itself.
    Plain,
    /// PBKDF2-HMAC-SHA512 of the value under a fresh salt, as a service keeping only a
    /// hash of its password reads it.
    Pbkdf2,
}

impl Form {
    /// The word a placeholder names [`Form::Pbkdf2`] by.
    pub const PBKDF2: &'static str = "pbkdf2";

    /// The form a placeholder's word names, where it names one.
    fn named(word: &str) -> Option<Self> {
        (word == Self::PBKDF2).then_some(Self::Pbkdf2)
    }
}

/// One placeholder in a starting file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder<'a> {
    /// The secret it names.
    pub secret: &'a str,
    /// The form it asks for.
    pub form: Form,
    /// Where it lies in the file, its braces included.
    pub span: Range<usize>,
}

/// Every placeholder in `text`, in the order the file holds them.
///
/// # Errors
///
/// A placeholder that never closes, one naming no secret, and one asking for a form
/// other than `pbkdf2`. Said without quoting the placeholder, which reads as a credential
/// to everything that withholds one from what it shows.
pub fn placeholders(text: &str) -> Result<Vec<Placeholder<'_>>, String> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text.get(from..).and_then(|rest| rest.find(OPENS)) {
        let start = from + offset;
        let inside = start + OPENS.len();
        let Some(length) = text.get(inside..).and_then(|rest| rest.find(CLOSES)) else {
            return Err("opens a placeholder that never closes".to_owned());
        };
        let end = inside + length + CLOSES.len();
        let named = text.get(inside..inside + length).unwrap_or_default();
        let (secret, form) = match named.split_once(FORMED) {
            None => (named, Form::Plain),
            Some((secret, word)) => (
                secret,
                Form::named(word).ok_or_else(|| {
                    format!(
                        "has a placeholder asking for the form {word}, and the one form a \
                         placeholder may ask for is {}",
                        Form::PBKDF2
                    )
                })?,
            ),
        };
        if secret.is_empty() {
            return Err("has a placeholder that names no secret".to_owned());
        }
        found.push(Placeholder {
            secret,
            form,
            span: start..end,
        });
        from = end;
    }
    Ok(found)
}

/// `text` with every placeholder replaced by what `written` answers for it.
///
/// # Errors
///
/// Why a placeholder could not be read, or why `written` could not answer for one.
pub fn filled(
    text: &str,
    mut written: impl FnMut(&Placeholder<'_>) -> Result<String, String>,
) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for placeholder in placeholders(text)? {
        out.push_str(text.get(from..placeholder.span.start).unwrap_or_default());
        out.push_str(&written(&placeholder)?);
        from = placeholder.span.end;
    }
    out.push_str(text.get(from..).unwrap_or_default());
    Ok(out)
}

/// What is wrong with the starting file at `path` of `service`, holding `bytes`.
///
/// A file naming no placeholder may be anything. One that names one is text, and each
/// placeholder names a secret the manifest declares as one of its own services'.
#[must_use]
pub fn refused(manifest: &Manifest, service: &str, path: &str, bytes: &[u8]) -> Vec<Violation> {
    let location = format!("config/{service}/{path}");
    let Ok(text) = std::str::from_utf8(bytes) else {
        return if bytes
            .windows(OPENS.len())
            .any(|window| window == OPENS.as_bytes())
        {
            vec![Violation {
                location,
                message: "names a secret and is not UTF-8, so where its value would be written \
                          cannot be read"
                    .to_owned(),
            }]
        } else {
            Vec::new()
        };
    };
    let named = match placeholders(text) {
        Ok(named) => named,
        Err(message) => return vec![Violation { location, message }],
    };
    named
        .iter()
        .filter_map(|placeholder| {
            owned(manifest, placeholder.secret)
                .err()
                .map(|message| Violation {
                    location: location.clone(),
                    message: format!("has a placeholder that {message}"),
                })
        })
        .collect()
}

/// Whether the plugin declares `secret` as one of its own services' credentials.
///
/// # Errors
///
/// Why it does not, in words that follow the placeholder or field naming it.
pub fn owned(manifest: &Manifest, secret: &str) -> Result<(), String> {
    let Some(declared) = manifest.secrets.iter().find(|one| one.id == secret) else {
        return Err(format!(
            "names the secret {secret}, which no [[secret]] of this manifest declares"
        ));
    };
    if manifest
        .services
        .iter()
        .any(|service| service.id == declared.of)
    {
        Ok(())
    } else {
        Err(format!(
            "names the secret {secret}, which is {}'s rather than one of this plugin's own \
             services', and a credential that crosses stays out of this plugin's files",
            declared.of
        ))
    }
}

#[cfg(test)]
mod tests;
