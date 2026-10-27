//! The keys an earlier release kept the core's own values under in the environment file.

use super::MEDIA_SERVER_MODE_KEY;

/// Each key an earlier release wrote, and the key its value is kept under now.
pub(crate) const RENAMED: [(&str, &str); 1] = [("JELLYFIN_MODE", MEDIA_SERVER_MODE_KEY)];

/// The name an earlier release printed the media server's administrator password under.
const EARLIER_MEDIA_SERVER_ADMIN_PASSWORD: &str = "Jellyfin administrator password";

/// The key an earlier release kept the value of `key` under, where it kept it under
/// another.
pub(crate) fn earlier(key: &str) -> Option<&'static str> {
    RENAMED
        .iter()
        .find(|(_, now)| *now == key)
        .map(|(was, _)| *was)
}

/// What an operator asking for a credential by the name an earlier release printed it
/// under asks for now, in lower case, where they asked by one.
pub(crate) fn asked_now(asked: &str) -> Option<String> {
    (asked.to_lowercase() == EARLIER_MEDIA_SERVER_ADMIN_PASSWORD.to_lowercase())
        .then(|| crate::credential::MEDIA_SERVER_ADMIN_PASSWORD_NAME.to_lowercase())
}

#[cfg(test)]
mod tests;
