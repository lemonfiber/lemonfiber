//! What a key a service generated for itself looks like.
//!
//! The curators, the Usenet client, the subtitle finder and the request service each write
//! the key they made into a file in their own directory, and lemonfiber reads it from
//! there, publishes it to the stack's other services and presents it in headers and
//! queries. The file is the container's to write, so what is in it is only taken for a
//! key where it is spelled the way every one of them spells a key: letters, digits and
//! the few marks hex, base64 and a UUID are written in. Anything else — text that would
//! read as a reference to another setting, a quote, a header broken across lines — is
//! not a key, and reads as one not written yet.

use lemonfiber_contract::adapter::{KEY_LIMIT, KEY_MARKS};

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
