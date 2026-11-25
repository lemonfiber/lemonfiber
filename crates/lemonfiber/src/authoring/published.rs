//! The published documents read back for a person: the parts of each a page shows,
//! read from the document itself, so what is shown and what is published cannot differ.

use std::fmt;
use std::marker::PhantomData;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
pub(crate) struct Vocabulary {
    pub(crate) vocabulary_version: u32,
    pub(crate) capabilities: Vec<Capability>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Capability {
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) contract: String,
    pub(crate) declared_by: Vec<String>,
    pub(crate) probes: Vec<Probe>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Probe {
    pub(crate) id: String,
    pub(crate) kind: Put,
    pub(crate) title: String,
    pub(crate) asks: String,
    pub(crate) credential: AskedAs,
    pub(crate) requires: Requires,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Put {
    Request,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AskedAs {
    None,
    Operator,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Requires {
    #[serde(default)]
    pub(crate) status: Vec<i64>,
    #[serde(default)]
    pub(crate) exit: Vec<i64>,
    #[serde(default)]
    pub(crate) body: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Points {
    pub(crate) extension_points_version: u32,
    pub(crate) points: Vec<Point>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Point {
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) register: String,
    pub(crate) engine: String,
    pub(crate) requires: String,
    pub(crate) row: Row,
    pub(crate) occupied: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Row {
    pub(crate) required: Vec<String>,
    pub(crate) optional: Vec<String>,
    #[serde(default, deserialize_with = "in_order")]
    pub(crate) bounds: Vec<(String, Limits)>,
    #[serde(default, deserialize_with = "in_order")]
    pub(crate) enums: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Limits {
    pub(crate) min: i64,
    pub(crate) max: i64,
    pub(crate) default: i64,
}

fn in_order<'de, D, T>(from: D) -> Result<Vec<(String, T)>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    from.deserialize_map(Entries(PhantomData))
}

struct Entries<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for Entries<T> {
    type Value = Vec<(String, T)>;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("a table keyed by field")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut table: A) -> Result<Self::Value, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = table.next_entry()? {
            entries.push(entry);
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests;
