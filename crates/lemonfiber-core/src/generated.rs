//! What a key a service generated for itself looks like.
//!
//! The \*arrs, the Usenet client, the subtitle finder and the request service each write
//! the key they made into a file in their own directory, and lemonfiber reads it from
//! there, publishes it to the stack's other services and presents it in headers and
//! queries. The file is the container's to write, so what is in it is only taken for a
//! key where it is spelled the way every one of them spells a key: letters, digits and
//! the few marks hex, base64 and a UUID are written in. Anything else — text that would
//! read as a reference to another setting, a quote, a header broken across lines — is
//! not a key, and reads as one not written yet.

/// The longest key taken. Every service here makes one well under a hundred characters
/// long; this is room to spare and still a bound.
pub const KEY_LIMIT: usize = 256;

/// The marks a key may hold besides letters and digits: hex has none, base64 has `+`,
/// `/` and `=`, and a UUID or a hand-made key has `-`, `_` and `.`.
const KEY_MARKS: &[u8] = b"-_.+/=";

/// `key`, where it is spelled the way a generated key is and no longer than
/// [`KEY_LIMIT`].
#[must_use]
pub fn key(key: &str) -> Option<String> {
    let spelled = key
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || KEY_MARKS.contains(&byte));
    (!key.is_empty() && key.len() <= KEY_LIMIT && spelled).then(|| key.to_owned())
}

#[cfg(test)]
mod tests;
