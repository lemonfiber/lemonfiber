/// Reading every setting, or changing one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settling {
    /// Every setting and secret the services read.
    Read,
    /// Record a value for a setting.
    Set {
        /// The setting.
        setting: String,
        /// The value.
        value: String,
        /// The offer being answered, as the reading of this change named it.
        agreement: Option<String>,
    },
    /// Take the operator's value away, so the default is written again.
    Clear {
        /// The setting.
        setting: String,
        /// The offer being answered, as the reading of this change named it.
        agreement: Option<String>,
    },
}
