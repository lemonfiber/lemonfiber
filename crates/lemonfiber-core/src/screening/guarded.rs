//! Telling the stream guard what it enforces: the media server's declaration, checked
//! first, inside the envelope with the one proxy the guard trusts and where it asks about
//! a live-stream pass. A declaration that does not pass its check is never applied.

use crate::ports::service::{Declared, Failure, Guarded, Guarding, Undeclarable};

/// Why the guard was not told what it enforces.
#[derive(Debug, thiserror::Error)]
pub enum Unguarded {
    /// The media server's declaration is one no guard may be handed.
    #[error("the media server's declaration was refused: {0}")]
    Undeclarable(#[from] Undeclarable),
    /// The guard did not take the envelope.
    #[error("the stream guard did not take what it enforces: {0}")]
    Untaken(#[from] Failure),
}

/// Hand `guard` the envelope around `declared`, where the declaration passes its check.
///
/// # Errors
///
/// [`Unguarded::Undeclarable`] without asking the guard anything where the declaration
/// fails its check, and [`Unguarded::Untaken`] where the guard refuses the envelope.
pub async fn guard(
    guard: &dyn Guarding,
    declared: Declared,
    proxy: String,
    pass: String,
) -> Result<(), Unguarded> {
    let guarded = Guarded {
        declared: declared.checked()?,
        proxy,
        pass,
    };
    guard.guard(&guarded).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
