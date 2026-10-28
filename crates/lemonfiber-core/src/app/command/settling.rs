use std::collections::BTreeMap;

/// Reading every setting, or changing some at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settling {
    /// Every setting and secret the services read.
    Read,
    /// Record a value for each setting named, checked together.
    Set {
        /// Each setting, with the value it is to hold.
        values: BTreeMap<String, String>,
        /// The offer being answered, as the reading of this change named it.
        agreement: Option<String>,
    },
    /// Take the operator's value away from each setting named, so each default is written
    /// again.
    Clear {
        /// The settings.
        settings: Vec<String>,
        /// The offer being answered, as the reading of this change named it.
        agreement: Option<String>,
    },
}
