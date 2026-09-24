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
mod tests;
