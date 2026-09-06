//! What this household agreed about the requests nobody rules on, kept between runs.
//!
//! The arrangement itself is [`crate::asking::Expiry`] and the reason it exists is
//! written there. This is only where it lives on disk: beside the settings, because it is
//! something the operator decided rather than something a run can work out again — and
//! because two different things read it. The clock reads it to know what it is holding the
//! household to; the household reading reads it to say so, on the list the operator reads
//! and in the message a member is handed, before anything is closed.
//!
//! **Reading is best effort and writing is not, and here the two part company sharply.** A
//! file that cannot be read is no arrangement, which closes nothing — the only safe
//! direction there is, because the other one would hold a household to a period nobody
//! could read back. A file that cannot be *written* is refused out loud: an operator told
//! their household would close requests after thirty days, by a run that recorded nothing,
//! would find every reading afterwards saying the opposite.

use crate::asking::Expiry;
use crate::error::Problem;

use super::Ctx;

/// What the arrangement is called, beside the environment file.
///
/// Named once and used from both sides: a reader and a writer disagreeing about the file
/// name would look exactly like a household that had never arranged this.
const NAME: &str = "expiring.json";

/// What this household agreed to, or nothing where it has agreed to nothing.
#[must_use]
pub(super) fn load(ctx: &Ctx) -> Expiry {
    super::record::beside(ctx, NAME)
}

/// Write the arrangement where the next run and the next reading will find it.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub(super) fn keep(ctx: &Ctx, agreed: &Expiry) -> Result<(), Box<Problem>> {
    super::record::keep(super::targets::beside_env(ctx, NAME).as_deref(), agreed)
}

#[cfg(test)]
mod tests {
    use super::{keep, load, NAME};
    use crate::asking::Expiry;
    use crate::config::Settings;
    use crate::test_support::{a_context, a_password, env_at};

    /// A context whose settings point at a scratch install of its own.
    fn ctx(name: &str) -> super::Ctx {
        a_context()
            .settings(Settings {
                env_file: Some(env_at(name, &a_password())),
                ..Settings::default()
            })
            .build()
    }

    /// What was agreed to is there for the next run and the next reading.
    #[test]
    fn what_was_agreed_to_is_read_back() {
        let ctx = ctx("arranged");
        assert_eq!(load(&ctx).after(), None, "a fresh install closes something");

        assert!(keep(&ctx, &Expiry::agreed_to(30, None)).is_ok());

        assert_eq!(load(&ctx).after(), Some(30));
    }

    /// Withdrawing it puts the household back where it started.
    #[test]
    fn withdrawing_it_puts_the_household_back_where_it_started() {
        let ctx = ctx("withdrawn");
        assert!(keep(&ctx, &Expiry::agreed_to(30, None)).is_ok());

        assert!(keep(&ctx, &Expiry::default()).is_ok());

        assert_eq!(load(&ctx).after(), None);
    }

    /// It sits beside the settings, which is what a backup carries.
    #[test]
    fn it_sits_with_the_settings() {
        let ctx = ctx("beside-settings");
        assert!(keep(&ctx, &Expiry::default()).is_ok());

        assert!(
            ctx.settings
                .env_file
                .as_deref()
                .is_some_and(|env| env.with_file_name(NAME).exists()),
            "nothing was written where the next run would look"
        );
    }

    /// An install with nowhere to keep it has agreed to nothing, and refuses to agree.
    ///
    /// A machine with no settings has arranged nothing either, so reading is nothing
    /// rather than a failure. Writing is the other way round: an operator told their
    /// household closes requests after thirty days by a run that recorded it nowhere
    /// would be told the opposite by every reading afterwards.
    #[test]
    fn nowhere_to_keep_it_reads_as_none_and_refuses_to_be_written() {
        let nowhere = a_context().build();

        assert_eq!(load(&nowhere).after(), None);
        assert!(keep(&nowhere, &Expiry::agreed_to(30, None)).is_err());
    }
}
