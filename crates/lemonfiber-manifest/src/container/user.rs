//! Who a service's container runs as: the image's own choice, the operator's ids in the
//! variables its image reads them from, or the operator's ids as its user.

use serde::{Deserialize, Serialize};

use super::words::is_variable;
use super::{Container, Posture, PGID, PUID, ZONE};

/// Who the container runs as.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(from = "UserModeWritten", into = "UserModeWritten")]
pub enum UserMode {
    /// No user: the image decides.
    Image,
    /// The operator's uid and gid in the two variables named, for an image that drops to
    /// them itself.
    IdsIn(IdVariables),
    /// The operator's uid and gid as the container's user.
    Uid,
}

/// The two variables an image reads the operator's uid and gid from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceIdVariables")]
pub struct IdVariables {
    /// The variable it reads the operator's uid from.
    pub uid_env: String,
    /// The variable it reads the operator's gid from.
    pub gid_env: String,
}

impl IdVariables {
    /// `PUID` and `PGID`, the pair `puid-env` names.
    #[must_use]
    pub fn puid() -> Self {
        Self {
            uid_env: PUID.to_owned(),
            gid_env: PGID.to_owned(),
        }
    }
}

/// How a manifest writes [`UserMode`]: a mode by name, or the variables an image reads
/// the operator's ids from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "ServiceUserModeWritten")]
pub enum UserModeWritten {
    /// A mode by name.
    Named(UserModeName),
    /// The variables its image reads the operator's uid and gid from.
    Variables(IdVariables),
}

impl<'de> Deserialize<'de> for UserModeWritten {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UserModeVisitor)
    }
}

/// Reads [`UserModeWritten`], naming the modes an unknown name is not one of.
struct UserModeVisitor;

impl<'de> serde::de::Visitor<'de> for UserModeVisitor {
    type Value = UserModeWritten;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("image, puid-env, uid, or a table of uid_env and gid_env")
    }

    fn visit_str<E: serde::de::Error>(self, name: &str) -> Result<UserModeWritten, E> {
        UserModeName::deserialize(serde::de::value::StrDeserializer::<E>::new(name))
            .map(UserModeWritten::Named)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<UserModeWritten, A::Error> {
        IdVariables::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            .map(UserModeWritten::Variables)
    }
}

/// A user mode a manifest writes by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ServiceUserMode")]
pub enum UserModeName {
    /// No user: the image decides.
    Image,
    /// `PUID` and `PGID` set to the operator's.
    PuidEnv,
    /// The operator's uid and gid as the container's user.
    Uid,
}

impl From<UserModeWritten> for UserMode {
    fn from(written: UserModeWritten) -> Self {
        match written {
            UserModeWritten::Named(UserModeName::Image) => Self::Image,
            UserModeWritten::Named(UserModeName::PuidEnv) => Self::IdsIn(IdVariables::puid()),
            UserModeWritten::Named(UserModeName::Uid) => Self::Uid,
            UserModeWritten::Variables(variables) => Self::IdsIn(variables),
        }
    }
}

impl From<UserMode> for UserModeWritten {
    fn from(mode: UserMode) -> Self {
        match mode {
            UserMode::Image => Self::Named(UserModeName::Image),
            UserMode::IdsIn(variables) if variables == IdVariables::puid() => {
                Self::Named(UserModeName::PuidEnv)
            }
            UserMode::IdsIn(variables) => Self::Variables(variables),
            UserMode::Uid => Self::Named(UserModeName::Uid),
        }
    }
}

impl schemars::JsonSchema for UserMode {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        UserModeWritten::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        UserModeWritten::json_schema(generator)
    }
}

impl UserMode {
    /// Who a service runs as: the mode it declares, or else `uid` where it speaks a
    /// contract or is confined, and the image's choice otherwise.
    #[must_use]
    pub fn of(declared: Option<&Self>, speaks: bool, posture: Posture) -> Self {
        match declared {
            Some(mode) => mode.clone(),
            None if speaks || matches!(posture, Posture::Confined) => Self::Uid,
            None => Self::Image,
        }
    }

    /// The variables the operator's uid and gid are written into, where it names any.
    #[must_use]
    pub const fn ids_in(&self) -> Option<&IdVariables> {
        match self {
            Self::IdsIn(variables) => Some(variables),
            Self::Image | Self::Uid => None,
        }
    }
}

impl Container<'_> {
    pub(super) fn named_ids(&self, variables: &IdVariables, found: &mut Vec<(String, String)>) {
        for (field, name) in [
            ("uid_env", &variables.uid_env),
            ("gid_env", &variables.gid_env),
        ] {
            let at = self.at(&format!("user_mode.{field}"));
            if !is_variable(name) {
                found.push((
                    at.clone(),
                    "is not upper-case letters, digits and \"_\", starting with a letter or \"_\""
                        .to_owned(),
                ));
            }
            if name == ZONE {
                found.push((at.clone(), format!("{name} is lemonfiber's to write")));
            }
            if self.env.contains_key(name) {
                found.push((
                    at,
                    format!(
                        "{name} is declared in env too, where lemonfiber writes the operator's id"
                    ),
                ));
            }
        }
        if variables.uid_env == variables.gid_env {
            found.push((
                self.at("user_mode"),
                format!("names {} for both the uid and the gid", variables.uid_env),
            ));
        }
    }
}

#[cfg(test)]
mod tests;
