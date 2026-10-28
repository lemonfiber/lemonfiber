//! When what an operation applied is in force: at once, or once the upstream restarts.

/// When what an operation applied is in force.
///
/// It crosses as `null` where it is in force at once, and as `"on-restart"` where it is in
/// force once the upstream restarts, which an adapter answers as `205`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(from = "Option<Restart>", into = "Option<Restart>")]
pub enum Applied {
    /// At once.
    Now,
    /// Once the upstream restarts.
    OnRestart,
}

/// What an operation applied that is in force only once the upstream restarts says.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Restart {
    /// In force once the upstream restarts.
    OnRestart,
}

impl From<Option<Restart>> for Applied {
    fn from(restart: Option<Restart>) -> Self {
        match restart {
            Some(Restart::OnRestart) => Self::OnRestart,
            None => Self::Now,
        }
    }
}

impl From<Applied> for Option<Restart> {
    fn from(applied: Applied) -> Self {
        match applied {
            Applied::OnRestart => Some(Restart::OnRestart),
            Applied::Now => None,
        }
    }
}

#[cfg(test)]
mod tests;
