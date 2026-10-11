//! A member's ask for a title the house does not hold yet.
//!
//! Apart from the table that names the rest because it is a member's own: a member's
//! request is narrowed to them whatever it names, and the operator names the member.
//! Who is named, whether the title is one they may ask for and what it would leave of
//! their allowance are the core's to judge, in the household's own sentences.

use lemonfiber_core::app::{Command, Wishing};
use lemonfiber_core::ports::media::Kind;
use lemonfiber_core::ports::service::Wish;

use crate::actions::{Arguments, Refused};

/// The action this file answers.
const ADDRESSED: &str = "ask";

/// Whether this action is the one it answers.
#[must_use]
pub(super) fn about_wishing(action: &str) -> bool {
    action == ADDRESSED
}

/// The command an ask names, or why it names none.
pub(super) fn asked_for(given: Arguments) -> Result<Command, Refused> {
    let Arguments {
        name,
        id,
        kind,
        seasons,
        ..
    } = given;
    let missing = |argument: &str| Refused::Missing {
        action: ADDRESSED.to_owned(),
        argument: argument.to_owned(),
    };
    let id = id.ok_or_else(|| missing("id"))?;
    let written = kind.ok_or_else(|| missing("kind"))?;
    let kind = Kind::for_media_type(&written).ok_or_else(|| Refused::Unrecognised {
        argument: "kind".to_owned(),
        offered: format!(
            "`{written}` is neither `{}` nor `{}`",
            Kind::Movies.media_type(),
            Kind::Tv.media_type()
        ),
    })?;
    Ok(Command::Wish(Wishing::Ask {
        member: name.unwrap_or_default(),
        wish: Wish { id, kind, seasons },
    }))
}
