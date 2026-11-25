//! Raw bytes as a command answers them, with the media type that says what they are.

use serde::Serialize;

use super::kind::{self, Kind};

/// Bytes a surface hands over as they are, and what they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Octets {
    /// Their media type.
    pub media_type: &'static str,
    /// The bytes themselves.
    pub bytes: Vec<u8>,
}

impl Octets {
    /// The kind they answer under, which no envelope holds.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        kind::OCTETS
    }

    /// The shapes the contract publishes for them: none, because no envelope holds them.
    pub const fn schemas(_each: &mut impl FnMut(Kind, schemars::Schema)) {}
}
