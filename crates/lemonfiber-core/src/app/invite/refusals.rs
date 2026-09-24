//! How an invitation is refused, and what each refusal tells the operator to do.
//!
//! Apart from the errand because they are about the operator rather than about the
//! account: which refusal they meet, the words they read it in, and what they can do next.
//! Together because that last part is the half worth reading side by side — a refusal an
//! operator cannot act on has only told them to give up.

/// Said where the stack holds no media server: there is nothing to make an account on.
pub(crate) fn no_media_server() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-1"),
        crate::error::Severity::Error,
        "this stack has no media server, so there is no account to offer",
        "An invitation is an account on the media server; without one there is nothing \
         for somebody to sign in to",
        crate::error::Remedy::new("Add a media server to the stack and run setup"),
    )
}

/// Said where the invitation is for nobody: the name is blank, or only spaces.
///
/// The media server refuses this too, in its own words, which are `400` and a link
/// to the specification of that status. The operator asked for something reasonable
/// and mistyped it, and is owed a sentence about the name rather than about HTTP.
pub(crate) fn nobody_named() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-4"),
        crate::error::Severity::Error,
        "an invitation needs somebody to be for",
        "The name is what they will sign in as, so a blank one is an account nobody \
         could use",
        crate::error::Remedy::new("Give the name they will sign in as")
            .with_detail("lemonfiber invite ana"),
    )
}

/// Said where this machine has no address the household could arrive at.
///
/// An invitation is an address somebody else types. Sending one built from a default
/// would be sending a link that opens nothing, which is worse than saying there is
/// none: the operator would learn it had failed from whoever they invited.
pub(crate) fn nowhere_to_send() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-3"),
        crate::error::Severity::Error,
        "this machine has no address the household could arrive at",
        "An invitation is an address somebody else opens, and this machine answers to \
         no name on the network and has none written down",
        crate::error::Remedy::new("Record the address the household should use")
            .with_detail("lemonfiber config set HOUSEHOLD_HOST <address>"),
    )
}

/// Said where an expired invitation could not be dated again, so its window is not real.
///
/// The account is untouched and still theirs — what failed is the write that says when it
/// was offered. Reported rather than glossed over because the message the operator is
/// about to send promises a window, and this one would be counted from whenever the
/// invitation was first made, which has already passed.
pub(crate) fn would_not_renew(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-5"),
        crate::error::Severity::Error,
        format!("the media server would not offer {name}'s invitation again"),
        "Their account is still there and still has no password on it; what could not be \
         written is when it was offered, which is what the window is counted from",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}

/// Said where the admin credential was never recorded: nothing can be asked of the server.
pub(crate) fn no_credential() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-2"),
        crate::error::Severity::Error,
        "the media server's own account has not been set up yet",
        "Making somebody else an account is done as the administrator, and this machine \
         has not recorded one",
        crate::error::Remedy::new("Run setup so the media server's account is made and recorded")
            .with_detail("lemonfiber setup"),
    )
}
