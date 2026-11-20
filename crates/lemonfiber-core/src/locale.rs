//! The house's locale: its language and region, as a `language-REGION` tag.

use serde::{Deserialize, Serialize};

/// The setting the house's locale is recorded as.
pub const SETTING: &str = "LEMONFIBER_LOCALE";

/// The house's locale where none is recorded, and setup's proposal where the host names
/// none.
pub const DEFAULT: &str = "en-US";

/// The host's variable setup proposes the house's locale from.
pub const HOST: &str = "LANG";

/// A `language-REGION` tag: two or three lowercase letters, a hyphen, then two uppercase
/// letters or three digits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Locale(String);

impl Locale {
    /// The tag `text` is.
    ///
    /// # Errors
    ///
    /// Why `text` is not a `language-REGION` tag, naming the shape.
    pub fn read(text: &str) -> Result<Self, String> {
        Some(text)
            .filter(|tag| shaped(tag))
            .map(|tag| Self(tag.to_owned()))
            .ok_or_else(|| {
                format!(
                    "{SETTING} is the house's language and region as one tag, \
                     two or three lowercase letters, a hyphen, then two uppercase letters or \
                     three digits, such as {DEFAULT}, and {text} is not one"
                )
            })
    }

    /// The tag setup proposes from the host's `LANG`: its language and territory, or
    /// [`DEFAULT`] where it names none that is such a tag.
    #[must_use]
    pub fn proposed(host: Option<&str>) -> Self {
        host.and_then(|lang| lang.split(['.', '@']).next())
            .map(|named| named.replace('_', "-"))
            .and_then(|tag| Self::read(&tag).ok())
            .unwrap_or_default()
    }

    /// The tag.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Locale {
    fn default() -> Self {
        Self(DEFAULT.to_owned())
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}

impl TryFrom<String> for Locale {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::read(&text)
    }
}

impl From<Locale> for String {
    fn from(locale: Locale) -> Self {
        locale.0
    }
}

impl schemars::JsonSchema for Locale {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Locale".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": PATTERN,
            "examples": [DEFAULT],
        })
    }
}

/// The shape of a `language-REGION` tag, as a pattern.
const PATTERN: &str = "^[a-z]{2,3}-([A-Z]{2}|[0-9]{3})$";

fn shaped(tag: &str) -> bool {
    tag.split_once('-').is_some_and(|(language, region)| {
        (2..=3).contains(&language.len())
            && language.bytes().all(|byte| byte.is_ascii_lowercase())
            && match region.len() {
                2 => region.bytes().all(|byte| byte.is_ascii_uppercase()),
                3 => region.bytes().all(|byte| byte.is_ascii_digit()),
                _ => false,
            }
    })
}

#[cfg(test)]
mod tests;
