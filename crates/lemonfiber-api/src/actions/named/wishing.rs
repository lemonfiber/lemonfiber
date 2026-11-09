//! A member's ask for a title the house does not hold yet.
//!
//! Apart from the table that names the rest because it is a member's own: a member's
//! request is narrowed to them whatever it names, and the operator names the member.
//! Who is named, whether the title is one they may ask for and what it would leave of
//! their allowance are the core's to judge, in the household's own sentences.

use lemonfiber_core::app::{Command, Wishing};
use lemonfiber_core::collected::{Medium, Unit};
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
        unit,
        seasons,
        ..
    } = given;
    let missing = |argument: &str| Refused::Missing {
        action: ADDRESSED.to_owned(),
        argument: argument.to_owned(),
    };
    let id = id.ok_or_else(|| missing("id"))?;
    let written = kind.ok_or_else(|| missing("kind"))?;
    let member = name.unwrap_or_default();
    if let Some(kind) = Kind::for_media_type(&written) {
        if unit.is_some() {
            return Err(Refused::Unwanted {
                action: ADDRESSED.to_owned(),
                argument: "unit".to_owned(),
            });
        }
        return Ok(Command::Wish(Wishing::Ask {
            member,
            wish: Wish { id, kind, seasons },
        }));
    }
    let unrecognised = |argument: &str, offered: String| Refused::Unrecognised {
        argument: argument.to_owned(),
        offered,
    };
    let medium = Medium::ALL
        .into_iter()
        .find(|medium| medium.word() == written)
        .ok_or_else(|| {
            unrecognised(
                "kind",
                format!(
                    "{written} is none of {}, {}, {} and {}",
                    Kind::Movies.media_type(),
                    Kind::Tv.media_type(),
                    Medium::Music.word(),
                    Medium::Books.word()
                ),
            )
        })?;
    if !seasons.is_empty() {
        return Err(Refused::Unwanted {
            action: ADDRESSED.to_owned(),
            argument: "seasons".to_owned(),
        });
    }
    let unit = Unit::chosen(Some(medium.word()), unit.as_deref())
        .map_err(|why| unrecognised("unit", why.to_string()))?
        .unwrap_or(Unit::first_of(medium));
    Ok(Command::Wish(Wishing::Collect {
        member,
        unit,
        reference: id,
    }))
}
