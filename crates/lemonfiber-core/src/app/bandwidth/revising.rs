//! Turning what was asked into what is declared, or refusing to.
//!
//! Pure, and apart from the command that carries it out, because everything here
//! is a decision about words rather than about a stack: what a limit means, what
//! is refused, and what an unstated half defaults to. A refusal that needed a
//! running stack to reproduce is a refusal nobody can write a case for.
//!
//! Every refusal here is about the request rather than the machine. That is the
//! whole reason they are gathered: a limit that could not be read, a cap declared
//! with nothing to do at it, an override longer than one may ask for — these are
//! things to say differently, not things to try again.

use crate::bandwidth::limit::UPLOAD_SHARE;
use crate::bandwidth::{Cap, Capacity, Declared, Limit, Respite, Rhythm, WhenExceeded, UNREADABLE};
use crate::error::{Amiss, Problem, Remedy, Severity};

use super::Asked;

/// The words that mean "take this away again".
const NONE: [&str; 3] = ["none", "off", "unset"];

/// What the operator declares, once this request has been folded into it.
///
/// # Errors
///
/// Returns a [`Problem`] where anything asked for could not be read as the thing
/// it names, where a cap arrives with nothing to do at it, or where an override
/// is longer than one may ask for.
pub(super) fn revised(now: u64, held: Declared, asked: &Asked) -> Result<Declared, Box<Problem>> {
    let mut declared = held;

    if let Some(text) = asked.line.as_deref() {
        declared.capacity = declared_line(text, now)?;
    }
    if let Some(text) = asked.down.as_deref() {
        declared.down = Some(read(
            text,
            "a download limit",
            "50%, 2MiB or unlimited",
            Limit::read,
        )?);
    }
    if let Some(text) = asked.up.as_deref() {
        declared.up = Some(read(
            text,
            "an upload limit",
            "25%, 512KiB or unlimited",
            Limit::read,
        )?);
    }
    declared.up = declared
        .up
        .or_else(|| defaulted_upload(asked, declared.down));

    if let Some(text) = asked.active.as_deref() {
        declared.rhythm = cleared(text)
            .map(|text| read(text, "the household's hours", "07:00-23:00", Rhythm::read))
            .transpose()?;
    }
    if let Some(text) = asked.cap.as_deref() {
        declared.cap = cleared(text)
            .map(|text| declared_cap(text, asked, declared.cap))
            .transpose()?;
    } else if let Some(text) = asked.exceeded.as_deref() {
        // Naming what to do at a cap that was never declared is a request that
        // cannot be answered as it stands, rather than one that quietly does
        // nothing — and neither is a word this build does not know.
        match (declared.cap, WhenExceeded::read(text)) {
            (Some(cap), Some(exceeded)) => declared.cap = Some(Cap { exceeded, ..cap }),
            (None, _) | (_, None) => {
                return Err(Box::new(unreadable(
                    "what to do at the cap",
                    text,
                    "pause, throttle or continue, and only where a cap is declared",
                )))
            }
        }
    }

    if let Some(minutes) = asked.unrestricted_for {
        declared.respite = Some(respite(now, minutes)?);
    }
    Ok(declared)
}

/// The measured line, as `<down>/<up>`.
fn declared_line(text: &str, now: u64) -> Result<Option<Capacity>, Box<Problem>> {
    let Some(text) = cleared(text) else {
        return Ok(None);
    };
    let read = text
        .split_once('/')
        .and_then(|(down, up)| Some((crate::bytes::read(down)?, crate::bytes::read(up)?)))
        .filter(|(down, up)| *down > 0 && *up > 0);
    let Some((down, up)) = read else {
        return Err(Box::new(unreadable(
            "what the line carries",
            text,
            "the two directions, faster one first, as in 60MiB/6MiB",
        )));
    };
    Ok(Some(Capacity {
        down,
        up,
        source: crate::bandwidth::capacity::Source::Declared,
        taken: now,
        through_tunnel: false,
    }))
}

/// The cap, and what is to happen at it.
///
/// A cap declared with nothing to do at it is refused rather than given a default,
/// because the whole of what the requirement asks for is that the choice is made
/// in advance — and a default chosen here is a choice nobody made, arriving at two
/// in the morning on a stack nobody is watching. A cap that already has a choice
/// keeps it, so changing the figure later is one decision rather than two.
fn declared_cap(text: &str, asked: &Asked, held: Option<Cap>) -> Result<Cap, Box<Problem>> {
    let Some(monthly) = crate::bytes::read(text).filter(|bytes| *bytes > 0) else {
        return Err(Box::new(unreadable(
            "a monthly cap",
            text,
            "a size, as in 1TiB or 500GiB",
        )));
    };
    // A word this build does not know is refused rather than falling back to what
    // was already recorded: an operator who typed `stop` meant something by it, and
    // quietly keeping `continue` is the cap doing the opposite of what they asked.
    let exceeded = match asked.exceeded.as_deref() {
        Some(word) => Some(WhenExceeded::read(word).ok_or_else(|| {
            Box::new(unreadable(
                "what to do at the cap",
                word,
                "pause, throttle or continue",
            ))
        })?),
        None => held.map(|cap| cap.exceeded),
    };
    let Some(exceeded) = exceeded else {
        return Err(Box::new(
            Problem::new(
                UNREADABLE,
                Severity::Error,
                "A cap needs to be told what happens when it is reached",
                "The point of declaring it in advance is that the answer is not \
                 decided at two in the morning by whatever is running. Say now what \
                 the stack should do, and it will do that.",
                Remedy::new("Say what happens at the cap")
                    .with_detail("--when-exceeded pause, throttle or continue"),
            )
            .lies_in(Amiss::Asking),
        ));
    };
    Ok(Cap { monthly, exceeded })
}

/// The override, or a refusal saying what may be asked for.
fn respite(now: u64, minutes: u64) -> Result<Respite, Box<Problem>> {
    Respite::asked_for(now, minutes.saturating_mul(60)).ok_or_else(|| {
        Box::new(
            Problem::new(
                UNREADABLE,
                Severity::Error,
                format!("{minutes} minutes is not a length the limits may be lifted for"),
                "An override is time-boxed on purpose. Something switched off just \
                 for now at eleven at night is the thing nobody remembers at eight \
                 the next morning, and the household finds out during the school run.",
                Remedy::new("Ask for an hour or two").with_detail(format!(
                    "anything from a minute up to {} minutes",
                    crate::bandwidth::respite::LONGEST / 60
                )),
            )
            .lies_in(Amiss::Asking),
        )
    })
}

/// The upload limit an unstated one falls back to, where anything was asked of the
/// download and nothing has ever been said about the upload.
///
/// Lower than the download's, always. A saturated uplink degrades everything the
/// line carries, downloads included, because the acknowledgements that keep a
/// download moving cannot get out past the queue of upload data — so the direction
/// that is not asked about is the one that gets the more careful figure.
fn defaulted_upload(asked: &Asked, down: Option<Limit>) -> Option<Limit> {
    if asked.up.is_some() || asked.down.is_none() {
        return None;
    }
    match down? {
        Limit::Unlimited => None,
        Limit::Share(share) => Some(Limit::Share(share.min(UPLOAD_SHARE))),
        // In the same terms the download was given in, rather than as a share. A
        // share of a line nobody has measured is refused, and refusing somebody who
        // asked for a figure in bytes — because of an upload figure they never
        // mentioned — is a refusal they did not cause and cannot act on.
        Limit::Absolute(bytes) => Some(Limit::Absolute(
            (bytes.saturating_mul(u64::from(UPLOAD_SHARE)) / 100).max(1),
        )),
    }
}

/// The text, or nothing where it is one of the words that take a setting away.
fn cleared(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    (!NONE.contains(&trimmed.to_ascii_lowercase().as_str())).then_some(trimmed)
}

/// One value read the way its own type reads it, or a refusal naming the shape.
fn read<T>(
    text: &str,
    what: &str,
    shape: &str,
    reading: fn(&str) -> Option<T>,
) -> Result<T, Box<Problem>> {
    reading(text).ok_or_else(|| Box::new(unreadable(what, text, shape)))
}

/// A request that could not be answered as it stands.
fn unreadable(what: &str, given: &str, shape: &str) -> Problem {
    Problem::new(
        UNREADABLE,
        Severity::Error,
        format!("`{given}` could not be read as {what}"),
        "A limit read wrongly is a household wondering all evening why the calls \
         keep dropping. Nothing here guesses at one.",
        Remedy::new(format!("Write {what} as {shape}")),
    )
    .lies_in(Amiss::Asking)
}

#[cfg(test)]
mod tests {
    use super::{defaulted_upload, revised, Asked};
    use crate::bandwidth::limit::UPLOAD_SHARE;
    use crate::bandwidth::{Cap, Declared, Limit, WhenExceeded, UNREADABLE};

    /// A moment every case here reads against.
    const NOW: u64 = 1_790_812_800;

    /// A request naming one thing.
    fn asking(field: impl FnOnce(&mut Asked)) -> Asked {
        let mut asked = Asked::default();
        field(&mut asked);
        asked
    }

    /// What a request revises a fresh declaration into.
    fn from_nothing(asked: &Asked) -> Result<Declared, String> {
        revised(NOW, Declared::default(), asked).map_err(|problem| problem.code.as_str().to_owned())
    }

    #[test]
    fn a_download_limit_can_be_a_share_or_a_figure() {
        assert_eq!(
            from_nothing(&asking(|asked| asked.down = Some("50%".to_owned())))
                .ok()
                .and_then(|declared| declared.down),
            Some(Limit::Share(50))
        );
        assert_eq!(
            from_nothing(&asking(|asked| asked.down = Some("2MiB".to_owned())))
                .ok()
                .and_then(|declared| declared.down),
            Some(Limit::Absolute(2 * 1024 * 1024))
        );
    }

    #[test]
    fn an_unstated_upload_limit_is_always_more_careful_than_the_download_one() {
        // The requirement, held as a rule rather than as two numbers somebody
        // keeps in order by hand. Whatever the download asks for, the upload asks
        // for no more.
        for share in 1..=100_u8 {
            let defaulted = defaulted_upload(
                &asking(|asked| asked.down = Some(format!("{share}%"))),
                Some(Limit::Share(share)),
            );
            assert_eq!(defaulted, Some(Limit::Share(share.min(UPLOAD_SHARE))));
            assert!(
                defaulted.is_some_and(|up| matches!(up, Limit::Share(up) if up <= share)),
                "{share}%"
            );
        }
    }

    /// A figure asked for in bytes defaults an upload in bytes, not a share.
    ///
    /// The refusal for a share of an unmeasured line offers exactly this as the way
    /// out — *give a figure instead of a share*, `--down 2MiB`. Defaulting that to a
    /// share of the same unmeasured line refused the operator for an upload they had
    /// not mentioned, and made the remedy unfollowable: doing what it said reproduced
    /// the error it said it would avoid.
    #[test]
    fn an_absolute_download_limit_leaves_the_upload_careful_in_the_same_terms() {
        assert_eq!(
            defaulted_upload(
                &asking(|asked| asked.down = Some("2MiB".to_owned())),
                Some(Limit::Absolute(2 * 1024 * 1024))
            ),
            Some(Limit::Absolute(2 * 1024 * 1024 / 4))
        );
        // Never nothing, however small the figure it is a quarter of.
        assert_eq!(
            defaulted_upload(
                &asking(|asked| asked.down = Some("1".to_owned())),
                Some(Limit::Absolute(1))
            ),
            Some(Limit::Absolute(1))
        );
    }

    #[test]
    fn an_upload_limit_that_was_asked_for_is_never_defaulted_over() {
        let asked = Asked {
            down: Some("50%".to_owned()),
            up: Some("unlimited".to_owned()),
            ..Asked::default()
        };
        assert_eq!(defaulted_upload(&asked, Some(Limit::Share(50))), None);
        assert_eq!(
            from_nothing(&asked).ok().and_then(|declared| declared.up),
            Some(Limit::Unlimited)
        );
    }

    #[test]
    fn lifting_the_download_limit_does_not_invent_an_upload_one() {
        assert_eq!(
            defaulted_upload(
                &asking(|asked| asked.down = Some("unlimited".to_owned())),
                Some(Limit::Unlimited)
            ),
            None
        );
    }

    #[test]
    fn a_cap_declared_with_nothing_to_do_at_it_is_refused_rather_than_defaulted() {
        // The whole of the requirement is that the choice is made in advance. A
        // default chosen here is a choice nobody made, arriving at two in the
        // morning on a stack nobody is watching.
        let refused = from_nothing(&asking(|asked| asked.cap = Some("1TiB".to_owned())));
        assert_eq!(refused, Err(UNREADABLE.as_str().to_owned()));
    }

    #[test]
    fn a_cap_that_already_has_a_choice_keeps_it_when_the_figure_changes() {
        let held = Declared {
            cap: Some(Cap {
                monthly: 1,
                exceeded: WhenExceeded::Throttle,
            }),
            ..Declared::default()
        };
        let changed = revised(
            NOW,
            held,
            &asking(|asked| asked.cap = Some("1TiB".to_owned())),
        );
        assert_eq!(
            changed.ok().and_then(|declared| declared.cap),
            Some(Cap {
                monthly: 1024_u64.pow(4),
                exceeded: WhenExceeded::Throttle
            })
        );
    }

    #[test]
    fn what_to_do_at_a_cap_nobody_declared_cannot_be_answered_as_it_stands() {
        let refused = from_nothing(&asking(|asked| asked.exceeded = Some("pause".to_owned())));
        assert_eq!(refused, Err(UNREADABLE.as_str().to_owned()));
    }

    #[test]
    fn what_to_do_at_the_cap_can_be_changed_without_restating_the_figure() {
        let held = Declared {
            cap: Some(Cap {
                monthly: 100,
                exceeded: WhenExceeded::Continue,
            }),
            ..Declared::default()
        };
        let changed = revised(
            NOW,
            held,
            &asking(|asked| asked.exceeded = Some("pause".to_owned())),
        );
        assert_eq!(
            changed.ok().and_then(|declared| declared.cap),
            Some(Cap {
                monthly: 100,
                exceeded: WhenExceeded::Pause
            })
        );
    }

    #[test]
    fn a_word_this_does_not_know_is_refused_wherever_it_appears() {
        for asked in [
            asking(|asked| asked.down = Some("half".to_owned())),
            asking(|asked| asked.up = Some("loads".to_owned())),
            asking(|asked| asked.active = Some("evenings".to_owned())),
            asking(|asked| asked.line = Some("fast".to_owned())),
            asking(|asked| asked.line = Some("60MiB".to_owned())),
            asking(|asked| asked.line = Some("60MiB/0".to_owned())),
            asking(|asked| asked.cap = Some("lots".to_owned())),
        ] {
            assert_eq!(
                from_nothing(&asked),
                Err(UNREADABLE.as_str().to_owned()),
                "{asked:?}"
            );
        }
    }

    #[test]
    fn a_setting_can_be_taken_away_again_by_saying_so() {
        let held = Declared {
            rhythm: crate::bandwidth::Rhythm::read("07:00-23:00"),
            cap: Some(Cap {
                monthly: 100,
                exceeded: WhenExceeded::Pause,
            }),
            capacity: None,
            ..Declared::default()
        };
        let asked = Asked {
            active: Some("none".to_owned()),
            cap: Some("off".to_owned()),
            ..Asked::default()
        };
        let cleared = revised(NOW, held, &asked).ok();
        assert!(cleared
            .as_ref()
            .is_some_and(|declared| declared.rhythm.is_none() && declared.cap.is_none()));
    }

    #[test]
    fn an_override_longer_than_one_may_ask_for_is_refused_with_what_may_be() {
        let refused = revised(
            NOW,
            Declared::default(),
            &asking(|asked| asked.unrestricted_for = Some(24 * 60)),
        );
        assert!(refused.as_ref().is_err_and(|problem| {
            problem.code == UNREADABLE
                && problem
                    .remedies
                    .first()
                    .and_then(|remedy| remedy.detail.as_deref())
                    .is_some_and(|detail| detail.contains("240 minutes"))
        }));
        assert!(revised(
            NOW,
            Declared::default(),
            &asking(|asked| asked.unrestricted_for = Some(0))
        )
        .is_err());
    }

    #[test]
    fn an_override_of_an_ordinary_evening_length_is_taken() {
        let asked = asking(|asked| asked.unrestricted_for = Some(60));
        assert_eq!(
            from_nothing(&asked)
                .ok()
                .and_then(|declared| declared.respite),
            Some(crate::bandwidth::Respite {
                until: NOW + 60 * 60
            })
        );
    }

    #[test]
    fn what_the_line_carries_is_read_in_both_directions_at_once() {
        let asked = asking(|asked| asked.line = Some("60MiB/6MiB".to_owned()));
        let declared = from_nothing(&asked).ok().and_then(|held| held.capacity);
        assert!(declared.is_some_and(|line| line.down == 60 * 1024 * 1024
            && line.up == 6 * 1024 * 1024
            && line.taken == NOW));
    }

    #[test]
    fn a_request_that_asks_for_nothing_changes_nothing() {
        assert_eq!(from_nothing(&Asked::default()), Ok(Declared::default()));
    }
}
