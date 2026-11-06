//! The API, or the APIs, an ask talks to its filler's upstream in.

use std::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The API an ask talks in, or every API it talks in any of: filled by an upstream
/// declaring any one of them. Written as one string where it names one, and as a list
/// where it names more.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Apis(Vec<String>);

impl Apis {
    /// The one API `name`.
    #[must_use]
    pub fn one(name: &str) -> Self {
        Self(vec![name.to_owned()])
    }

    /// Every API named, in the order they were written.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.0
    }

    /// Whether `api` is one of them.
    #[must_use]
    pub fn names_any(&self, api: &str) -> bool {
        self.0.iter().any(|one| one == api)
    }

    /// Them as one phrase: the API, or each joined by "or".
    #[must_use]
    pub fn said(&self) -> String {
        self.0.join(" or ")
    }
}

impl Serialize for Apis {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0.as_slice() {
            [one] => serializer.serialize_str(one),
            many => many.serialize(serializer),
        }
    }
}

/// What an API or a list of them is written as.
#[derive(Deserialize)]
#[serde(untagged)]
enum Written {
    /// One API.
    One(String),
    /// A list of them.
    Many(Vec<String>),
}

impl<'de> Deserialize<'de> for Apis {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Written::deserialize(deserializer)? {
            Written::One(one) => Self(vec![one]),
            Written::Many(many) => Self(many),
        })
    }
}

impl schemars::JsonSchema for Apis {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Apis")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let one = generator.subschema_for::<String>();
        let many = generator.subschema_for::<Vec<String>>();
        schemars::json_schema!({ "anyOf": [one, many] })
    }
}

#[cfg(test)]
mod tests;
