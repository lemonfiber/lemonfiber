//! Every refusal this surface answers with, and the code that says which it is.
//!
//! A status groups refusals and cannot tell them apart. Six of them answer `403` — a
//! secret this run does not admit, a request from somewhere else, an account asking for
//! what is not its own, an account the media server could not vouch for, a key sent in
//! the clear and a key asking past its scope — and each has a different remedy: sign in
//! again, reach the right address, leave it be, try later, connect encrypted, ask
//! somebody. A client left with the status and a sentence would have to parse English
//! to choose between them, so every refusal is a problem document, and its code is the
//! answer.
//!
//! **One list, and everything answers through it.** The contract publishes the codes a
//! refusal may carry so that a client generates its list rather than copying one, and
//! a list assembled from wherever refusals happen to be raised is a list that misses
//! the one added next. [`Refusal::EVERY`] is what the contract reads, and a refusal's
//! problem is only ever built by [`Refusal::problem`], so the code a client is sent and
//! the code the contract lists come from one place.
//!
//! **The sentence is still the surface's own.** Each refusal carries the one plain line
//! a reader gets, and the few whose line names what was asked for — an action, an
//! argument, how long to wait — are answered with that line instead. The code is what
//! a client branches on; the sentence is what a person reads, and may be reworded.

use axum::body::Body;
use axum::http::{Response, StatusCode};
use lemonfiber_core::error::codes::{admit, ask, read, serve};
use lemonfiber_core::error::{Code, Problem, Remedy};
use lemonfiber_core::model::{kind, Envelope};

use crate::read::{answered_with, enveloped};

/// Why a request was not answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// It carried no secret this run admits.
    NotAdmitted,
    /// It said it came from somewhere this server is not.
    Elsewhere,
    /// It proved who it is, and this is not theirs.
    NotYours,
    /// Whether it is still anybody could not be established.
    Unconfirmed,
    /// The password offered at the door was not this machine's.
    NotThePassword,
    /// The door has had too many wrong passwords lately.
    TooManyAttempts,
    /// What was offered at the door is not a password.
    NotAPassword,
    /// A key arrived from another machine over a connection its pin does not verify.
    KeyInTheClear,
    /// A key asked for something its scope does not reach.
    NotForAKey,
    /// A read was given a parameter its answer has nowhere to put.
    Unwanted,
    /// A parameter carrying one value was given more than once.
    Repeated,
    /// No read goes by the name that was asked for.
    NoSuchRead,
    /// A trace was asked for with nothing to follow.
    NoTerm,
    /// The season to narrow a trace to is not a number.
    NotASeason,
    /// A setting was asked for by an empty name.
    NoSetting,
    /// A household member was asked for by an empty name.
    NoMember,
    /// A shelf was asked for and nobody named whose it is.
    NoShelfWithoutAMember,
    /// How many holdings to answer with is not a whole number.
    NotACount,
    /// More holdings were asked for than one read answers with.
    TooManyAtOnce,
    /// A diagnosis was narrowed to a group or check that is not one.
    NoSuchGroup,
    /// A removal was named that is none of the four.
    NoSuchRemoval,
    /// Moving forward was asked about and neither object was named.
    NoUpdateObject,
    /// How many log lines to begin with is not a number within the ceiling.
    NotALineCount,
    /// A parameter that takes a yes or a no is neither true nor false.
    NotAChoice,
    /// A household read named a member and asked for the household's defaults as well.
    MemberAndDefaults,
    /// No action goes by the name that was asked for.
    NoSuchAction,
    /// An action was not given an argument it needs.
    MissingArgument,
    /// An argument was given a value that names nothing.
    UnrecognisedArgument,
    /// An argument was given to an action whose command has nowhere to put it.
    UnwantedArgument,
    /// Two arguments arrived together that each name a different request.
    ArgumentsTogether,
    /// The body of an action is not arguments it can read.
    NotArguments,
    /// A job was asked about that this run did not start.
    NoSuchJob,
    /// The body of a setup step is not an answer it can read.
    NotAnAnswer,
    /// The body of a mint is not what a key is minted with.
    NotAKeyRequest,
    /// An action's idempotency key is not one this surface reads as a key.
    NotAnIdempotencyKey,
    /// An idempotency key already named an attempt that asked for something else.
    IdempotencyKeyReused,
    /// A path under the endpoints that no endpoint answers.
    NoEndpoint,
    /// An endpoint asked with a method it does not answer.
    WrongMethod,
    /// An answer that could not be rendered.
    Unrenderable,
    /// Work that could not be named, and so was not begun.
    NoJobName,
    /// An action's work ended before it had an answer to give.
    Unanswered,
}

/// The one refusal whose own rendering is the thing that failed, already rendered.
///
/// Written out rather than rendered at the moment it is needed, because it is needed
/// exactly when rendering has just failed, and rendering it again could fail the same
/// way. A test renders it and compares, so the text cannot drift from what
/// [`Refusal::answer`] would have written.
pub(crate) const UNRENDERED: &str = r#"{"api_version":1,"kind":"error","data":{"code":"SERVE-6","severity":"error","state":"actionable","summary":"This answer could not be rendered.","meaning":"The request was understood and carried out, and what it came to could not be written down as an answer. Nothing about the request was wrong.","remedies":[{"action":"Ask again, and send a diagnostic bundle if it keeps happening","detail":"lemonfiber support"}],"detail":null,"cause":null}}"#;

impl Refusal {
    /// Every refusal, in the order the codes are declared.
    ///
    /// What the contract lists, so a variant added above and not here is a code no
    /// client can name. A test holds the two together.
    pub const EVERY: [Self; 41] = [
        Self::NotAdmitted,
        Self::Elsewhere,
        Self::NotYours,
        Self::Unconfirmed,
        Self::NotThePassword,
        Self::TooManyAttempts,
        Self::NotAPassword,
        Self::KeyInTheClear,
        Self::NotForAKey,
        Self::Unwanted,
        Self::Repeated,
        Self::NoSuchRead,
        Self::NoTerm,
        Self::NotASeason,
        Self::NoSetting,
        Self::NoMember,
        Self::NoShelfWithoutAMember,
        Self::NotACount,
        Self::TooManyAtOnce,
        Self::NoSuchGroup,
        Self::NoSuchRemoval,
        Self::NoUpdateObject,
        Self::NotALineCount,
        Self::NotAChoice,
        Self::MemberAndDefaults,
        Self::NoSuchAction,
        Self::MissingArgument,
        Self::UnrecognisedArgument,
        Self::UnwantedArgument,
        Self::ArgumentsTogether,
        Self::NotArguments,
        Self::NoSuchJob,
        Self::NotAnAnswer,
        Self::NotAKeyRequest,
        Self::NotAnIdempotencyKey,
        Self::IdempotencyKeyReused,
        Self::NoEndpoint,
        Self::WrongMethod,
        Self::Unrenderable,
        Self::NoJobName,
        Self::Unanswered,
    ];

    /// The code a client branches on.
    #[must_use]
    pub const fn code(self) -> Code {
        match self {
            Self::NotAdmitted => admit::NOT_ADMITTED,
            Self::Elsewhere => admit::ELSEWHERE,
            Self::NotYours => admit::NOT_YOURS,
            Self::Unconfirmed => admit::UNCONFIRMED,
            Self::NotThePassword => admit::NOT_THE_PASSWORD,
            Self::TooManyAttempts => admit::TOO_MANY_ATTEMPTS,
            Self::NotAPassword => admit::NOT_A_PASSWORD,
            Self::KeyInTheClear => admit::KEY_IN_THE_CLEAR,
            Self::NotForAKey => admit::NOT_FOR_A_KEY,
            Self::Unwanted => read::UNWANTED,
            Self::Repeated => read::REPEATED,
            Self::NoSuchRead => read::NO_SUCH_READ,
            Self::NoTerm => read::NO_TERM,
            Self::NotASeason => read::NOT_A_SEASON,
            Self::NoSetting => read::NO_SETTING,
            Self::NoMember => read::NO_MEMBER,
            Self::NoShelfWithoutAMember => read::NO_SHELF_WITHOUT_A_MEMBER,
            Self::NotACount => read::NOT_A_COUNT,
            Self::TooManyAtOnce => read::TOO_MANY_AT_ONCE,
            Self::NoSuchGroup => read::NO_SUCH_GROUP,
            Self::NoSuchRemoval => read::NO_SUCH_REMOVAL,
            Self::NoUpdateObject => read::NO_UPDATE_OBJECT,
            Self::NotALineCount => read::NOT_A_LINE_COUNT,
            Self::NotAChoice => read::NOT_A_CHOICE,
            Self::MemberAndDefaults => read::MEMBER_AND_DEFAULTS,
            Self::NoSuchAction => ask::NO_SUCH_ACTION,
            Self::MissingArgument => ask::MISSING_ARGUMENT,
            Self::UnrecognisedArgument => ask::UNRECOGNISED_ARGUMENT,
            Self::UnwantedArgument => ask::UNWANTED_ARGUMENT,
            Self::ArgumentsTogether => ask::ARGUMENTS_TOGETHER,
            Self::NotArguments => ask::NOT_ARGUMENTS,
            Self::NoSuchJob => ask::NO_SUCH_JOB,
            Self::NotAnAnswer => ask::NOT_AN_ANSWER,
            Self::NotAKeyRequest => ask::NOT_A_KEY_REQUEST,
            Self::NotAnIdempotencyKey => ask::NOT_AN_IDEMPOTENCY_KEY,
            Self::IdempotencyKeyReused => ask::IDEMPOTENCY_KEY_REUSED,
            Self::NoEndpoint => ask::NO_ENDPOINT,
            Self::WrongMethod => ask::WRONG_METHOD,
            Self::Unrenderable => serve::UNRENDERABLE,
            Self::NoJobName => serve::NO_JOB_NAME,
            Self::Unanswered => serve::UNANSWERED,
        }
    }

    /// The status a refusal answers with: the one its code is declared with.
    ///
    /// `401` is the door's alone, and only for the password it was just offered. A
    /// session this run no longer admits is `403` like every other refusal of who is
    /// asking: `401` invites a browser to ask for credentials it has no way to supply,
    /// and whether signing in again would help is what the code says.
    #[must_use]
    pub fn status(self) -> StatusCode {
        answered_with(self.code())
    }

    /// What the refusal says, in the one line a reader gets.
    ///
    /// Where the line names what was asked for, the caller answers with its own line
    /// instead, and this is the same fact said without the particulars.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            // Deliberately vague, as is the one below it. Both answer somebody who has
            // proved nothing, and naming what was wrong — which secret, which header —
            // would tell them what to keep guessing at.
            Self::NotAdmitted => "This request carried no token, session or key this run admits.",
            Self::Elsewhere => "This request said it came from somewhere this server is not.",
            // Said plainly: this answers somebody who proved who they are, so there is
            // nothing left to guess, and a household member reading it is owed the
            // actual reason rather than a silence that reads as a fault.
            Self::NotYours => "This is not something this account may ask for.",
            // Neither of the refusals above, and it must not be said as either. The
            // account has not been turned away or found missing — it has not been asked
            // about — so the thing to fix is the media server, not the account.
            Self::Unconfirmed => {
                "This account could not be checked with the media server, so nobody \
                 was identified. Nothing about the account has changed."
            }
            // One sentence for a wrong password and for a right one where nothing is
            // set: to whoever is knocking they are the same fact, and saying which
            // would tell somebody guessing whether there is anything here to guess at.
            Self::NotThePassword => "That is not the password for this machine.",
            Self::TooManyAttempts => "Too many wrong passwords and keys. Try again later.",
            Self::NotAPassword => "The body of this request is not a password.",
            // Said before the key was looked at, so it says nothing about whether the key
            // was right: only that it came the wrong way.
            Self::KeyInTheClear => {
                "A key is accepted from another machine only over the encrypted connection \
                 its pin verifies."
            }
            Self::NotForAKey => "A key with this scope may not call this.",
            Self::Unwanted => "This read takes no such parameter.",
            Self::Repeated => {
                "This read takes that parameter once, and it was given more than once."
            }
            Self::NoSuchRead => "There is no read by that name.",
            Self::NoTerm => "What to follow must be named.",
            Self::NotASeason => "Which season to narrow to must be a number.",
            Self::NoSetting => "Which setting to read must be named.",
            Self::NoMember => "Which member to narrow to must be named.",
            // Apart from the one above because they refuse different things: that one is
            // said where naming nobody would have meant everybody, and this where there
            // is no everybody to fall back to.
            Self::NoShelfWithoutAMember => "Whose shelf to read must be named.",
            Self::NotACount => "How many holdings to answer with must be a whole number.",
            Self::TooManyAtOnce => "That is more holdings than one read answers with.",
            Self::NoSuchGroup => "There is no group of checks and no check by that name.",
            Self::NoSuchRemoval => {
                "Which removal must be one of stop, services, configuration or media."
            }
            // Refused rather than answered with either. Neither object is the smaller case
            // of the other — one moves somebody's services and the other this program —
            // so a page asking about one and handed the other was answered wrongly.
            Self::NoUpdateObject => "Which of stack or self to move forward must be named.",
            Self::NotALineCount => "How many lines to begin with must be a number.",
            Self::NotAChoice => "A parameter that takes a yes or a no must be true or false.",
            Self::MemberAndDefaults => {
                "Name a member or ask for the household's defaults, not both."
            }
            Self::NoSuchAction => {
                "There is no action by that name. \
                 This surface offers what the command line offers, and nothing else."
            }
            Self::MissingArgument => "This action needs an argument that was not given.",
            Self::UnrecognisedArgument => "An argument given is not one this stack knows.",
            Self::UnwantedArgument => "This action takes an argument that was given to it.",
            Self::ArgumentsTogether => {
                "This action was given two arguments that ask different things."
            }
            Self::NotArguments => "The body of this request is not arguments this action can read.",
            // What was asked for is not repeated back, and nothing tells a name never
            // minted from one another run minted: this run knows only its own.
            Self::NoSuchJob => "No work in this run goes by that name.",
            // What arrived is not quoted back. An answer carries a credential, and a
            // sentence repeating the body would carry it wherever the sentence goes.
            Self::NotAnAnswer => {
                "The body of this request is not one of setup's answers, nor a way out of \
                 an interrupted apply."
            }
            // What arrived is not quoted back, because it carries the password.
            Self::NotAKeyRequest => {
                "The body of this request is not a key's name, scope and purpose with the \
                 password."
            }
            Self::NotAnIdempotencyKey => {
                "An Idempotency-Key is one to 255 visible characters, given once, and this \
                 request's is not."
            }
            Self::IdempotencyKeyReused => {
                "This Idempotency-Key was already sent with another action or other arguments."
            }
            Self::NoEndpoint => "No endpoint answers this path.",
            Self::WrongMethod => "This endpoint does not answer that method.",
            Self::Unrenderable => "This answer could not be rendered.",
            Self::NoJobName => {
                "This machine would not supply the randomness a job needs to be named."
            }
            Self::Unanswered => {
                "This action stopped before it had an answer to give, and sending it again \
                 runs it again."
            }
        }
    }

    /// What to do about it, most likely first.
    fn remedy(self) -> Remedy {
        match self {
            Self::NotAdmitted => Remedy::new(
                "Sign in again, or open lemonfiber from the address it printed when it started",
            ),
            Self::Elsewhere => Remedy::new("Reach lemonfiber at the address it is listening on"),
            Self::NotYours => Remedy::new("Ask whoever looks after this machine if you need it"),
            Self::Unconfirmed => Remedy::new("Try again once the media server is running"),
            Self::NotThePassword => Remedy::new("Check the password and try again"),
            Self::KeyInTheClear => Remedy::new(
                "Connect over https, pinning the certificate the key was minted with, and \
                 revoke the key if it may have been read",
            ),
            Self::NotForAKey => Remedy::new(
                "Ask whoever looks after this machine, or call only what the contract lists \
                 as callable by a key",
            ),
            Self::TooManyAttempts => {
                Remedy::new("Wait as long as the refusal says, then try once more")
                    .with_detail("Retry-After")
            }
            Self::NotAPassword => Remedy::new("Send the password as the body's `password`"),
            Self::Unwanted => Remedy::new("Ask again, naming only what this read takes"),
            Self::Repeated => Remedy::new("Ask again, naming it once"),
            Self::NoSuchRead => Remedy::new("Ask for one of the reads the contract names"),
            Self::NoEndpoint => Remedy::new("Ask for one of the endpoints the contract names"),
            Self::NoTerm
            | Self::NotASeason
            | Self::NoSetting
            | Self::NoMember
            | Self::NoShelfWithoutAMember
            | Self::NotACount
            | Self::TooManyAtOnce
            | Self::NoSuchGroup
            | Self::NoSuchRemoval
            | Self::NoUpdateObject
            | Self::NotALineCount
            | Self::NotAChoice
            | Self::MemberAndDefaults => Remedy::new("Ask again as the sentence says"),
            Self::NoSuchAction => Remedy::new("Ask for one of the actions the contract names"),
            Self::MissingArgument
            | Self::UnrecognisedArgument
            | Self::UnwantedArgument
            | Self::ArgumentsTogether
            | Self::NotArguments => Remedy::new("Ask again with the arguments the action takes"),
            Self::NoSuchJob => Remedy::new("Ask about a job this run started"),
            Self::NotAnAnswer => Remedy::new("Answer the question setup is asking"),
            Self::NotAKeyRequest => {
                Remedy::new("Send the name, scope, purpose and password as the body's four fields")
            }
            Self::WrongMethod => Remedy::new("Ask again with the method the contract names"),
            Self::NotAnIdempotencyKey => {
                Remedy::new("Send the key once, as up to 255 visible characters with no spaces")
            }
            Self::IdempotencyKeyReused => Remedy::new(
                "Send a new key with each new attempt, and the same key only with \
                             the same action sent again",
            ),
            Self::Unrenderable | Self::NoJobName | Self::Unanswered => {
                Remedy::new("Ask again, and send a diagnostic bundle if it keeps happening")
                    .with_detail("lemonfiber support")
            }
        }
    }

    /// The refusal as a problem, saying `summary` as its one line.
    #[must_use]
    pub fn problem(self, summary: impl Into<String>) -> Problem {
        Problem::new(self.code(), summary, self.meaning(), self.remedy())
    }

    /// The refusal as a response, in its own words.
    #[must_use]
    pub fn answered(self) -> Response<Body> {
        self.answer(self.problem(self.said()))
    }

    /// The refusal as a response, saying `summary` instead of its own line.
    ///
    /// For the refusals whose line names what was asked for — an action, an argument,
    /// how long is left — so the particulars reach whoever asked.
    #[must_use]
    pub fn saying(self, summary: impl Into<String>) -> Response<Body> {
        self.answer(self.problem(summary))
    }

    /// A problem this refusal raised, answered at this refusal's status.
    #[must_use]
    pub fn answer(self, problem: Problem) -> Response<Body> {
        enveloped(self.status(), Envelope::new(kind::ERROR, problem).to_json())
    }
}

mod meaning;

#[cfg(test)]
mod tests;
