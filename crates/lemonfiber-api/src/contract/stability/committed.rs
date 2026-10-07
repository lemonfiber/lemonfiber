//! The surface as the directory it is committed as: an index, and a file per type.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::Surface;
use crate::contract::layout::{rendered, Files};
use crate::contract::INDEX;

/// Where the types are, inside the surface's directory.
const TYPES: &str = "types/";

/// What the surface's index holds: everything but the types, which have a file each.
#[derive(Debug, Serialize, Deserialize)]
struct Index {
    /// The wire version these shapes belong to.
    api_version: u32,
    /// Each emitted kind, and the shape of the payload it carries.
    #[serde(default)]
    kinds: BTreeMap<String, String>,
    /// Every code a refusal may carry, and the status it is answered with.
    #[serde(default)]
    refusals: BTreeMap<String, u16>,
}

impl Surface {
    /// Every file of the surface's directory, keyed by its path inside it, written the
    /// way the contract's directory is, so the two diff alike.
    #[must_use]
    pub fn files(&self) -> Files {
        let mut files = Files::new();
        let index = Index {
            api_version: self.api_version,
            kinds: self.kinds.clone(),
            refusals: self.refusals.clone(),
        };
        files.insert(INDEX.to_owned(), rendered(&index));
        for (name, shape) in &self.types {
            files.insert(format!("{TYPES}{name}.json"), rendered(shape));
        }
        files
    }

    /// What was committed, or nothing where there is no readable surface to compare
    /// against: no index, or any file in the directory that is not one this writes.
    #[must_use]
    pub fn from_files(files: &Files) -> Option<Self> {
        let index: Index = serde_json::from_str(files.get(INDEX)?).ok()?;
        let mut types = BTreeMap::new();
        for (path, text) in files.iter().filter(|(path, _)| path.as_str() != INDEX) {
            let name = path.strip_prefix(TYPES)?.strip_suffix(".json")?;
            types.insert(name.to_owned(), serde_json::from_str(text).ok()?);
        }
        Some(Self {
            api_version: index.api_version,
            kinds: index.kinds,
            types,
            refusals: index.refusals,
            strings: BTreeSet::new(),
        })
    }
}
