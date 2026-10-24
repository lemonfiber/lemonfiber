use lemonfiber::cli::{Granting, RawCredentials};
use lemonfiber_core::app::{Asking, Command};

/// What is being asked about the credentials this stack holds.
///
/// Naming nothing is the reading. Naming one to reveal or one to rotate is an act on
/// a line of that reading, and the two cannot arrive together — the command line
/// refuses the pair, so there is no order of precedence here to get wrong. Revoking a
/// grant is an act on one of the grants the reading lists.
///
/// The confirmation belongs to the reveal and to nothing else. Carried through
/// rather than acted on here, because what an unconfirmed reveal answers with is a
/// warning that has to be written once, where the value would otherwise be.
pub(crate) fn credentials(asked: RawCredentials) -> Command {
    if let Some(Granting::Revoke {
        plugin,
        credential,
        offer,
    }) = asked.grant
    {
        return Command::Credentials(Asking::Revoke {
            plugin,
            credential,
            agreement: offer,
        });
    }
    Command::Credentials(match (asked.reveal, asked.rotate) {
        (Some(credential), _) => Asking::Reveal {
            credential,
            confirmed: asked.confirm,
        },
        (None, Some(credential)) => Asking::Rotate { credential },
        (None, None) => Asking::Read,
    })
}
