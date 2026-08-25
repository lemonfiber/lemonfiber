//! Why an action was not carried out, and what a caller is told about it.
//!
//! Five refusals and no more. A name nothing answers to, an argument an action
//! needs and was not given, an argument that was given and names nothing, an
//! argument given to an action whose command has nowhere to put it, and two
//! arguments that mean two different requests when they arrive together. Each is a
//! different mistake by whoever asked, and answering all five with one sentence
//! would leave them to work out which they made.
//!
//! The last is the one the command line answers for itself: clap declares the pair
//! in conflict and refuses the run before anything of ours reads it. Nothing does
//! that here, so a surface that dropped one of the two would carry out the request
//! the other names — which is the same mistake as dropping an argument outright,
//! made where it is harder to see.

use axum::http::StatusCode;

/// Why an action was not carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// No action goes by that name.
    Unknown {
        /// The name as it was asked for.
        name: String,
    },
    /// The action needs an argument that was not given.
    Missing {
        /// The action that needs it.
        action: String,
        /// The argument it needs.
        argument: String,
    },
    /// The argument was given and names nothing.
    Unrecognised {
        /// The argument that was given.
        argument: String,
        /// What it said, and what it could have said instead.
        offered: String,
    },
    /// The argument was given to an action whose command has nowhere to put it.
    Unwanted {
        /// The action it was given to.
        action: String,
        /// The argument it does not take.
        argument: String,
    },
    /// Both arguments were given, and each names a different request.
    Together {
        /// The action they were given to.
        action: String,
        /// The argument that was asked for.
        argument: String,
        /// The one it cannot be asked for alongside.
        alongside: String,
    },
}

impl Refused {
    /// The status a refusal answers with.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::Unknown { .. } => StatusCode::NOT_FOUND,
            Self::Missing { .. }
            | Self::Unrecognised { .. }
            | Self::Unwanted { .. }
            | Self::Together { .. } => StatusCode::BAD_REQUEST,
        }
    }

    /// What the refusal says, in the one line a reader gets.
    #[must_use]
    pub fn said(&self) -> String {
        match self {
            Self::Unknown { name } => format!(
                "There is no action named `{name}`. \
                 This surface offers what the command line offers, and nothing else."
            ),
            Self::Missing { action, argument } => {
                format!("The action `{action}` needs `{argument}`, which was not given.")
            }
            Self::Unrecognised { argument, offered } => {
                format!("The `{argument}` given is not one this stack knows: {offered}")
            }
            Self::Unwanted { action, argument } => format!(
                "The action `{action}` takes no `{argument}`. It is refused rather \
                 than dropped, because dropping it would carry out a different \
                 request from the one asked for."
            ),
            Self::Together {
                action,
                argument,
                alongside,
            } => format!(
                "The action `{action}` takes `{argument}` or `{alongside}`, not both: \
                 each names a different request, so a run carrying the two would have \
                 to pick one of them and answer something nobody asked for."
            ),
        }
    }
}
