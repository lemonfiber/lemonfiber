//! Telling the stream guard what it enforces: the media server's declaration, checked
//! first, inside the envelope with where the guard reaches the media server and its
//! adapter, the one proxy the guard trusts and where it asks about a live-stream pass. An
//! envelope that does not pass its check is never applied.

use crate::ports::service::{joined, Addressee, Failure, Guarded, Guarding, Undeclarable};

/// Why the guard was not told what it enforces.
#[derive(Debug, thiserror::Error)]
pub enum Unguarded {
    /// The media server's declaration is one no guard may be handed.
    #[error("the media server's declaration was refused: {0}")]
    Undeclarable(#[from] Undeclarable),
    /// The envelope does not say where the guard reaches what its question is sent to,
    /// or says it as something other than a scheme, a host and a port.
    #[error("the stream guard was not told where to reach {0}")]
    Unaddressed(String),
    /// The guard did not take the envelope.
    #[error("the stream guard did not take what it enforces: {0}")]
    Untaken(#[from] Failure),
}

/// What the guard's question is sent to, by name.
const UPSTREAM: &str = "the media server";

/// What the guard's question is sent to where the adapter answers it, by name.
const ADAPTER: &str = "the media server's adapter";

/// Hand `guard` the envelope `guarded`, where it passes its check.
///
/// # Errors
///
/// [`Unguarded::Undeclarable`] where the declaration fails its check, and
/// [`Unguarded::Unaddressed`] where the envelope gives no address, or not a bare one, for
/// the media server or for the adapter its question is sent to, each without asking the
/// guard anything; [`Unguarded::Untaken`] where the guard refuses the envelope.
pub async fn guard(guard: &dyn Guarding, guarded: Guarded) -> Result<(), Unguarded> {
    let declared = guarded.declared.checked()?;
    let asks_adapter = declared
        .question
        .as_ref()
        .is_some_and(|question| question.to == Addressee::Adapter);
    let bare = |address: &str| joined(address, "/").is_some();
    if !bare(&guarded.upstream) {
        return Err(Unguarded::Unaddressed(UPSTREAM.to_owned()));
    }
    let adapter_reached =
        guarded.adapter.as_deref().is_none_or(bare) && (!asks_adapter || guarded.adapter.is_some());
    if !adapter_reached {
        return Err(Unguarded::Unaddressed(ADAPTER.to_owned()));
    }
    let checked = Guarded {
        declared,
        ..guarded
    };
    guard.guard(&checked).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
