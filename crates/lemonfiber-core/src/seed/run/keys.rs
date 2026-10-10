//! The key a service of the curator shape wrote for itself, and what a connection comes
//! to where the file holding it was refused.

use super::{Ctx, Path};
use crate::ports::filesystem::Beneath;

/// The API key a service of the curator shape wrote for itself, read from the file
/// its own declaration names — beneath its own directory where a plugin brought it.
///
/// [`Beneath::Read`] holds the key itself; a file holding none is as absent as one not
/// written, and a file refused stays refused, so the caller can say so.
pub(super) async fn curator_key(ctx: &Ctx, filler: &crate::wiring::Filler) -> Beneath {
    match crate::app::targets::credential_file(ctx, filler).await {
        Beneath::Read(text) => {
            crate::servarr::api_key(&text).map_or(Beneath::Absent, Beneath::Read)
        }
        other => other,
    }
}

/// A connection refused because the filler's credential file was, saying why.
///
/// Refused rather than skipped: no later run reads the file while it stays what it is,
/// and it is either a mistake in the plugin or an attempt by it, which the operator has
/// to see either way.
pub(super) fn refused(connection: String, filler: &crate::wiring::Filler) -> crate::seed::Wiring {
    crate::seed::Wiring::settled(connection, refusal(filler))
}

/// What a connection comes to where the filler's credential file was refused.
pub(super) fn refusal(filler: &crate::wiring::Filler) -> crate::seed::State {
    crate::seed::State::Refused {
        reason: crate::app::targets::escaped(filler),
    }
}

/// A curator's API key, read from the configuration file it wrote it
/// to, or nothing where it has not written one yet.
pub(super) async fn read_curator_key(ctx: &Ctx, config: &Path) -> Option<String> {
    let within = crate::within::directory_of(config);
    let text =
        crate::app::targets::read_owned(ctx.seams.filesystem.as_ref(), config, within).await?;
    crate::servarr::api_key(&text)
}
