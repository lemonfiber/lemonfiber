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

/// The most placeholders one service's starting files may hold in all.
///
/// Each is written when the service is installed and again whenever its secret is
/// replaced, and its hashed form is a hundred thousand rounds of PBKDF2, so a file of
/// them would be a manifest that holds an install up rather than one that configures it.
pub const MOST_PLACEHOLDERS: usize = 16;

/// The form a placeholder asks its secret to be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginPlaceholderForm")]
pub enum Form {
    /// The value itself.
    Plain,
    /// PBKDF2-HMAC-SHA512 of the value under a fresh salt, as a service keeping only a
    /// hash of its password reads it.
    Pbkdf2,
}

impl Form {
    /// The word a placeholder names [`Form::Pbkdf2`] by.
    pub const PBKDF2: &str = "pbkdf2";

    /// The word [`Form::Plain`] is named by, which a placeholder asking for it leaves out.
    pub const PLAIN: &str = "plain";

    /// The word this form is named by.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Plain => Self::PLAIN,
            Self::Pbkdf2 => Self::PBKDF2,
        }
    }

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

impl Placeholder<'_> {
    /// The placeholder as it is named to an operator, as [`named`] names it.
    #[must_use]
    pub fn named(&self) -> String {
        named(self.secret, self.form)
    }
}

/// A placeholder naming `secret` in `form`, as it is named to an operator: by its secret
/// and its form, never quoted.
#[must_use]
pub fn named(secret: &str, form: Form) -> String {
    format!(
        "the placeholder for secret {secret} in the {} form",
        form.word()
    )
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
                        "has the placeholder for secret {secret} asking for the form {word}, and \
                         the one form a placeholder may ask for is {}",
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
        if found.len() > MOST_PLACEHOLDERS {
            return Err(format!(
                "has more than {MOST_PLACEHOLDERS} placeholders, the most a service's \
                 starting files may hold"
            ));
        }
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
/// placeholder names a secret the manifest declares as `service`'s own.
#[must_use]
pub fn refused(manifest: &Manifest, service: &str, path: &str, bytes: &[u8]) -> Vec<Violation> {
    let location = format!("config/{service}/{path}");
    let Ok(text) = std::str::from_utf8(bytes) else {
        return unreadable(bytes)
            .into_iter()
            .map(|message| Violation {
                location: location.clone(),
                message,
            })
            .collect();
    };
    let named = match placeholders(text) {
        Ok(named) => named,
        Err(message) => return vec![Violation { location, message }],
    };
    named
        .iter()
        .filter_map(|placeholder| {
            unowned(manifest, placeholder.secret)
                .or_else(|| not_its_own(manifest, service, placeholder.secret))
                .map(|clause| Violation {
                    location: location.clone(),
                    message: format!("has {}, whose secret {clause}", placeholder.named()),
                })
        })
        .collect()
}

/// What is said of a starting file holding `bytes` that are not UTF-8: one refusal for
/// each placeholder that can still be read, one for the file where a placeholder opens
/// and none can, and none where no placeholder opens at all.
fn unreadable(bytes: &[u8]) -> Vec<String> {
    const UNREAD: &str = "and is not UTF-8, so where its value would be written cannot be read";
    if !bytes
        .windows(OPENS.len())
        .any(|window| window == OPENS.as_bytes())
    {
        return Vec::new();
    }
    let lossy = String::from_utf8_lossy(bytes);
    let named: Vec<String> = placeholders(&lossy)
        .unwrap_or_default()
        .iter()
        .filter(|placeholder| !placeholder.secret.contains(char::REPLACEMENT_CHARACTER))
        .map(|placeholder| format!("has {} {UNREAD}", placeholder.named()))
        .collect();
    if named.is_empty() {
        vec![format!("names a secret {UNREAD}")]
    } else {
        named
    }
}

/// Why `secret`, which the manifest declares, is not `service`'s own, where it is not.
fn not_its_own(manifest: &Manifest, service: &str, secret: &str) -> Option<String> {
    manifest
        .secrets
        .iter()
        .find(|one| one.id == secret && one.of != service)
        .map(|declared| {
            format!(
                "is {}'s rather than {service}'s, and a starting file holds only its own \
                 service's secrets",
                declared.of
            )
        })
}

/// Whether the plugin declares `secret` as one of its own services' credentials.
///
/// # Errors
///
/// Why it does not, in words that follow the placeholder or field naming it.
pub fn owned(manifest: &Manifest, secret: &str) -> Result<(), String> {
    unowned(manifest, secret).map_or(Ok(()), |clause| {
        Err(format!("names the secret {secret}, which {clause}"))
    })
}

/// Why the plugin does not declare `secret` as one of its own services' credentials, as
/// what follows the secret, where it does not.
fn unowned(manifest: &Manifest, secret: &str) -> Option<String> {
    let Some(declared) = manifest.secrets.iter().find(|one| one.id == secret) else {
        return Some("no [[secret]] of this manifest declares".to_owned());
    };
    (!manifest
        .services
        .iter()
        .any(|service| service.id == declared.of))
    .then(|| {
        format!(
            "is {}'s rather than one of this plugin's own services', and a credential that \
             crosses stays out of this plugin's files",
            declared.of
        )
    })
}

#[cfg(test)]
mod tests;
