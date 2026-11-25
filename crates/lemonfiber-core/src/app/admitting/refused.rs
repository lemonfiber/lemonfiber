//! Every refusal of who is asking that the core says, each in its own words.

use std::time::Duration;

use crate::error::codes::admit::{
    CODE_NOT_WAITING, NOT_OPEN, NOT_THE_PASSWORD, NOT_YOURS, NO_SESSION, SHORT_CHOICE,
    SIGNS_IN_BY_PASSWORD, TOO_MANY_ATTEMPTS, TOO_MANY_CODES, UNCONFIRMED,
};
use crate::error::{Code, Problem, Remedy};

/// Every code who is asking is refused with.
pub const REFUSALS: [Code; 10] = [
    NOT_YOURS,
    UNCONFIRMED,
    NOT_THE_PASSWORD,
    TOO_MANY_ATTEMPTS,
    NOT_OPEN,
    SHORT_CHOICE,
    CODE_NOT_WAITING,
    TOO_MANY_CODES,
    SIGNS_IN_BY_PASSWORD,
    NO_SESSION,
];

const AT_THE_DOOR: &str = "Nothing was opened, and no session was begun.";

impl super::Caller {
    /// The refusal of a caller whose account could not be checked with the media server.
    #[must_use]
    pub fn unconfirmed() -> Box<Problem> {
        Box::new(Problem::new(
            UNCONFIRMED,
            "This account could not be checked with the media server, so nobody was identified. \
             Nothing about the account has changed.",
            "Nobody was identified, so nothing was answered. The account has not been removed and \
             the session has not ended.",
            Remedy::new("Try again once the media server is running"),
        ))
    }

    /// The refusal of a caller asking for what belongs to somebody else, or to whoever
    /// looks after this machine.
    #[must_use]
    pub fn not_yours() -> Box<Problem> {
        Box::new(Problem::new(
            NOT_YOURS,
            "This is not something this account may ask for.",
            "Nothing is wrong with the account. What was asked for belongs to somebody else, or \
             to whoever looks after this machine.",
            Remedy::new("Ask whoever looks after this machine if you need it"),
        ))
    }

    /// The refusal of a caller whose wrong answers have earned a wait of `left`.
    #[must_use]
    pub fn held(left: Duration) -> Box<Problem> {
        let seconds = left.as_secs().max(1);
        Box::new(
            Problem::new(
                TOO_MANY_ATTEMPTS,
                format!("Too many wrong passwords and keys. Try again in {seconds} seconds."),
                "This surface has stopped looking at passwords and keys for a while. Another \
                 attempt now makes the wait longer.",
                Remedy::new("Wait as long as the refusal says, then try once more")
                    .with_detail("Retry-After"),
            )
            .waiting(left),
        )
    }
}

/// The password offered proves nobody.
#[must_use]
pub fn not_the_password() -> Box<Problem> {
    Box::new(Problem::new(
        NOT_THE_PASSWORD,
        "That is not the password for this machine.",
        AT_THE_DOOR,
        Remedy::new("Check the password and try again"),
    ))
}

/// The invitation cannot be claimed.
#[must_use]
pub fn not_open() -> Box<Problem> {
    Box::new(Problem::new(
        NOT_OPEN,
        "This invitation can no longer be claimed.",
        "Nothing was set, and no session was begun. The same is said whether the invitation \
         was claimed already, ran out, was declined or was never one.",
        Remedy::new("Ask whoever invited you for a new invitation"),
    ))
}

/// The password chosen at a claim is too short.
#[must_use]
pub fn short_choice() -> Box<Problem> {
    Box::new(Problem::new(
        SHORT_CHOICE,
        "That password is too short to choose.",
        "Nothing was set. The invitation still stands and can be claimed with a longer \
         password.",
        Remedy::new(format!(
            "Choose a password of at least {} characters",
            crate::admission::LEAST
        )),
    ))
}

/// The sign-in by a code cannot be finished.
#[must_use]
pub fn not_waiting() -> Box<Problem> {
    Box::new(Problem::new(
        CODE_NOT_WAITING,
        "This sign-in by a code can no longer be finished.",
        "No session was begun. The same is said whether the code lapsed, was used already, \
         was asked for by another session or was never one.",
        Remedy::new("Ask for a new code and approve it before it lapses"),
    ))
}

/// This source has asked for as many codes as it may for `left`.
#[must_use]
pub fn too_many_codes(left: Duration) -> Box<Problem> {
    let seconds = left.as_secs().max(1);
    Box::new(
        Problem::new(
            TOO_MANY_CODES,
            format!("Too many sign-in codes asked for. Try again in {seconds} seconds."),
            "This device or session has asked for as many sign-in codes as it may for now. \
             Other devices are still given codes.",
            Remedy::new("Wait as long as the refusal says, or finish a code already shown")
                .with_detail("Retry-After"),
        )
        .waiting(left),
    )
}

/// The household signs in only by name and password.
#[must_use]
pub fn signs_in_by_password() -> Box<Problem> {
    Box::new(Problem::new(
        SIGNS_IN_BY_PASSWORD,
        "This household signs in with a name and password.",
        "No code was asked for. This household keeps its own accounts, so its members sign in \
         with a name and password.",
        Remedy::new("Sign in with the member's name and password"),
    ))
}

/// This machine would not supply the bytes a session's secret is made of.
#[must_use]
pub fn no_session() -> Box<Problem> {
    Box::new(Problem::new(
        NO_SESSION,
        "No session could be begun.",
        "The password was right and no session was begun, because a session's secret is \
         unpredictable bytes this machine supplies and it would not supply them.",
        Remedy::new(
            "Try again, and if it happens twice the operating system's own random source is at \
             fault",
        ),
    ))
}
