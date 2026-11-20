//! The zone the stack runs in, as a name from the time zone database.

use serde::{Deserialize, Serialize};

/// A zone the time zone database names, such as `Europe/Amsterdam`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Zone(String);

impl Zone {
    /// The zone `name` is.
    ///
    /// # Errors
    ///
    /// Why `name` is not a zone the database names, naming the setting.
    pub fn read(name: &str) -> Result<Self, String> {
        Some(name)
            .filter(|named| !named.is_empty() && jiff::tz::TimeZone::get(named).is_ok())
            .map(|named| Self(named.to_owned()))
            .ok_or_else(|| {
                format!(
                    "{} is the zone the stack runs in, a name from the time zone database such \
                     as {}, and {name} is not one",
                    crate::config::ZONE_KEY,
                    crate::config::DEFAULT_ZONE
                )
            })
    }

    /// The zone setup proposes from the host's: the one it names, or the default where
    /// it names none the database holds.
    #[must_use]
    pub fn proposed(host: Option<&str>) -> Self {
        host.and_then(|named| Self::read(named).ok())
            .unwrap_or_default()
    }

    /// The zone the host's own clock keeps, where it names one.
    #[must_use]
    pub fn host() -> Option<String> {
        named(jiff::tz::TimeZone::try_system().ok())
    }

    /// The zone's name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Zone {
    fn default() -> Self {
        Self(crate::config::DEFAULT_ZONE.to_owned())
    }
}

impl std::fmt::Display for Zone {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}

impl TryFrom<String> for Zone {
    type Error = String;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::read(&name)
    }
}

impl From<Zone> for String {
    fn from(zone: Zone) -> Self {
        zone.0
    }
}

impl schemars::JsonSchema for Zone {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Zone".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "examples": [crate::config::DEFAULT_ZONE],
        })
    }
}

fn named(zone: Option<jiff::tz::TimeZone>) -> Option<String> {
    zone?.iana_name().map(str::to_owned)
}

#[cfg(test)]
mod tests;
