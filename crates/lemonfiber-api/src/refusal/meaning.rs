//! What each refusal means for whoever asked, said beside what it is.

use super::Refusal;

impl Refusal {
    /// What the refusal means for whoever asked.
    pub(super) const fn meaning(self) -> &'static str {
        match self {
            Self::NotAdmitted => {
                "This run of lemonfiber does not let in what this request carried, and \
                 nothing was answered."
            }
            Self::Elsewhere => {
                "The address this request named, or the page it came from, is not the one \
                 this server is listening on, and nothing was answered."
            }
            Self::NotYours => {
                "Nothing is wrong with the account. What was asked for belongs to somebody \
                 else, or to whoever looks after this machine."
            }
            Self::Unconfirmed => {
                "Nobody was identified, so nothing was answered. The account has not been \
                 removed and the session has not ended."
            }
            Self::NotThePassword => "Nothing was opened, and no session was begun.",
            Self::KeyInTheClear => {
                "The key crossed a network unencrypted, so it was not looked at and nothing \
                 was answered. Anything between that machine and this one may have read it."
            }
            Self::NotForAKey => "The key is admitted and may not call this, so nothing was done.",
            Self::TooManyAttempts => {
                "This surface has stopped looking at passwords and keys for a while. \
                 Another attempt now makes the wait longer."
            }
            Self::NotAPassword => {
                "The door reads a password, and a household member's name beside it, and \
                 this body carried neither in a form it can read."
            }
            Self::Unwanted => {
                "It is refused rather than dropped, because dropping it would answer a \
                 wider question than the one that was asked — and a wider answer reads \
                 like the answer."
            }
            Self::Repeated => {
                "Which of them was meant is not something this can work out, and answering \
                 for one of them would drop the others without saying so."
            }
            Self::NoSuchRead => {
                "Every read this surface answers is named in the contract, and this name \
                 is not one of them."
            }
            Self::NoTerm
            | Self::NotASeason
            | Self::NoSetting
            | Self::NoMember
            | Self::NoShelfWithoutAMember
            | Self::NotACount
            | Self::NoUpdateObject
            | Self::NotALineCount
            | Self::NotAChoice
            | Self::MemberAndDefaults => {
                "The read cannot be answered as it was asked, and answering a different \
                 question in its place would read like the answer to this one."
            }
            // Refused rather than quietly cut down to the ceiling: a caller that asked for
            // five thousand and was handed five hundred has been told it has the whole
            // shelf, which is the same failure as a wider answer than was asked for.
            Self::TooManyAtOnce => {
                "It is refused rather than cut down, because a shorter answer wearing the \
                 shape of the whole one reads as the whole shelf."
            }
            Self::NoSuchGroup | Self::NoSuchRemoval => {
                "The word names none of the things there are, and reading it as the \
                 nearest one would answer something that was not asked."
            }
            Self::NoSuchAction => {
                "An action here is a command the command line offers, and nothing by this \
                 name is one."
            }
            Self::MissingArgument
            | Self::UnrecognisedArgument
            | Self::UnwantedArgument
            | Self::ArgumentsTogether
            | Self::NotArguments => {
                "The action was not carried out, and nothing was changed. Carrying out \
                 a different request from the one asked for would be worse than none."
            }
            Self::NoSuchJob => {
                "Jobs are named by the run that starts them, and nothing this run started \
                 goes by this name. Work from an earlier run is not tracked here."
            }
            Self::NotAnAnswer => "Setup did not move, and nothing was changed.",
            Self::NotAKeyRequest => "No key was minted.",
            Self::NoEndpoint => {
                "Every endpoint this surface answers is named in the contract, and this \
                 path is not one of them."
            }
            Self::WrongMethod => {
                "The path is one this surface answers, asked in a way it does not answer \
                 it, and nothing was done."
            }
            Self::Unrenderable => {
                "The request was understood and carried out, and what it came to could not \
                 be written down as an answer. Nothing about the request was wrong."
            }
            Self::NoJobName => {
                "A job with no name is work nothing could ever be told about, so it was \
                 not begun, and nothing was changed."
            }
        }
    }
}
