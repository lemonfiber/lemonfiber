//! Why requests were turned down, kept between runs.
//!
//! The record itself is [`crate::asking::Reasons`] and the reason it exists is written
//! there. This is only where it lives on disk: beside the settings, because it is a
//! record of something the operator decided rather than something a run can work out
//! again, and a restore that lost it would leave every refusal bare — which is the state
//! the record exists to get a household out of.
//!
//! **Best effort both ways, unlike the choices an operator answered.** A record that
//! cannot be read costs the words beside one refusal, which is a worse message rather
//! than a wrong one; and a decision already carried out at the request service must not
//! fail on the way back because a note could not be written. What that failure costs is
//! said where it happens instead.

use crate::asking::Reasons;
use crate::error::Problem;

use super::Ctx;

/// What the record is called, beside the environment file.
///
/// Named once and used from both sides: a reader and a writer disagreeing about the file
/// name would look exactly like a household nobody had ever refused anything.
const NAME: &str = "refusals.json";

/// Every reason this machine holds, or none where nothing has been turned down here.
#[must_use]
pub(super) fn load(ctx: &Ctx) -> Reasons {
    super::record::beside(ctx, NAME)
}

/// Write the reasons where the next run will find them.
///
/// # Errors
///
/// Where there is nowhere configured to keep them, or the file cannot be written. Said
/// rather than swallowed, unlike the histories a run can work out again: what is lost
/// here is the only copy of somebody's words, and an operator told a reason is theirs to
/// pass on while nothing kept it would find it gone the next time they looked.
pub(super) fn keep(ctx: &Ctx, reasons: &Reasons) -> Result<(), Box<Problem>> {
    super::record::keep(super::targets::beside_env(ctx, NAME).as_deref(), reasons)
}

#[cfg(test)]
mod tests {
    use super::{keep, load, NAME};
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

    /// A reason written down is there for the next run to read.
    #[test]
    fn a_reason_written_down_is_read_back() {
        let ctx = ctx("kept");
        let mut held = load(&ctx);
        assert!(held.is_empty(), "a fresh install holds a refusal");

        held.keep(41, "we already have it dubbed", None);
        assert!(keep(&ctx, &held).is_ok());

        assert_eq!(
            load(&ctx).of(41).map(|kept| kept.reason.as_str()),
            Some("we already have it dubbed")
        );
    }

    /// The record sits beside the settings, which is what a backup carries.
    #[test]
    fn the_record_sits_with_the_settings() {
        let ctx = ctx("beside");
        assert!(keep(&ctx, &load(&ctx)).is_ok());

        assert!(
            ctx.settings
                .env_file
                .as_deref()
                .is_some_and(|env| env.with_file_name(NAME).exists()),
            "nothing was written where the next run would look"
        );
    }

    /// An install with nowhere to keep them holds none, and says so on the way out.
    ///
    /// A machine with no settings has turned nothing down either, so reading is nothing
    /// rather than a failure. Writing is the other way round: the words are the only copy
    /// there is, and losing them in silence is the thing this record exists to stop.
    #[test]
    fn nowhere_to_keep_them_reads_as_none_and_refuses_to_be_written() {
        let nowhere = a_context().build();

        assert!(load(&nowhere).is_empty());
        assert!(keep(&nowhere, &load(&nowhere)).is_err());
    }
}
