codes! {
    /// Said where the stack holds no media server: there is nothing to make an account on.
    NO_MEDIA_SERVER = "INVITE-1" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "This stack has no media server, so there is no account to offer. An invitation \
            is an account somebody signs in to; without one there is nothing to invite them to.",
        remedy: "Add a media server to the stack and run setup.",
    }
    /// Said where the admin credential was never recorded: nothing can be asked of the server.
    NO_CREDENTIAL = "INVITE-2" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The media server's own account has not been set up yet. Making somebody else \
            an account is done as the administrator, and this machine has not recorded one.",
        remedy: "Run `lemonfiber setup`, so the media server's account is made and recorded.",
    }
    /// Said where this machine has no address the household could arrive at.
    ///
    /// An invitation is an address somebody else types. Sending one built from a default
    /// would be sending a link that opens nothing, which is worse than saying there is
    /// none: the operator would learn it had failed from whoever they invited.
    NOWHERE_TO_SEND = "INVITE-3" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "This machine has no address the household could arrive at. An invitation is an \
            address somebody else opens, and this machine answers to no name on the network and \
            has none written down.",
        remedy: "Record the address the household should use, with `lemonfiber config set \
            HOUSEHOLD_HOST <address>`.",
    }
    /// Said where the invitation is for nobody: the name is blank, or only spaces.
    ///
    /// The media server refuses this too, in its own words, which are `400` and a link
    /// to the specification of that status. The operator asked for something reasonable
    /// and mistyped it, and is owed a sentence about the name rather than about HTTP.
    NOBODY_NAMED = "INVITE-4" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The invitation is for nobody — the name was blank. The name is what they will \
            sign in as, so a blank one is an account nobody could use.",
        remedy: "Give the name they will sign in as, as in `lemonfiber invite ana`.",
    }
    /// Said where an invitation offered again could not be dated again, so its window is not
    /// real.
    ///
    /// The account is untouched and still theirs — what failed is the write that says when it
    /// was offered. Reported rather than glossed over because the message the operator is
    /// about to send promises a window, and this one would be counted from whenever the
    /// invitation was first made, which has already passed.
    WOULD_NOT_RENEW = "INVITE-5" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "An invitation offered again could not be dated again, so its window is not \
            real. Their account is still there as it was; what could not be written down is when \
            it was offered, which is what the window is counted from.",
        remedy: "Check the configuration directory can be written, then run this again.",
    }
    /// Said where the media server would not say what libraries it holds.
    ///
    /// Refused rather than read as no libraries at all: a name matched against an empty
    /// list is a name that could not be found, and the operator would be told their library
    /// does not exist when what happened is that nobody could ask.
    NO_LIBRARIES_READ = "INVITE-6" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The media server would not say what libraries it holds, so nobody was invited. \
            Choosing what somebody may open starts by finding the libraries, and that read did \
            not answer.",
        remedy: "Check the media server is running, then run this again.",
    }
    /// Said where no library goes by a name that was given.
    ///
    /// The ones there are, named: the fix is one word, and the words are already in hand.
    NO_SUCH_LIBRARY = "INVITE-7" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "No library goes by a name that was given, so nobody was invited. Libraries are \
            named the way the media server's own screens name them, though not necessarily in \
            the same capitalisation.",
        remedy: "Name a library the media server holds. The message lists the ones there are.",
    }
    /// Said where what the account may watch could not be written on it.
    ///
    /// A new account is taken back rather than left open, so the offer is refused whole;
    /// an existing one keeps what it already had. Said as which of the two is now true,
    /// because an operator told only that something failed would not know whether an
    /// open account is standing somewhere.
    WOULD_NOT_ALLOW = "INVITE-8" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The media server would not set what the account may watch. A new account is \
            taken back rather than left open to every library, so there is no invitation to \
            send; an existing one is still there, allowed what it was allowed before.",
        remedy: "Run this again with the same choices, or set them in the media server's own \
            settings.",
    }
    /// Said where the offer could not be written down.
    ///
    /// An invitation runs out a set time after it is offered, and one this machine has no
    /// date for is taken back the next time anybody is invited — so an offer that could
    /// not be dated is not made.
    UNRECORDED = "INVITE-9" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "When they were offered an account could not be written down, so it was not \
            offered. An invitation runs out a set time after it is offered, and one this machine \
            has no date for is taken back the next time anybody is invited.",
        remedy: "Check the configuration directory can be written, then run this again.",
    }
    /// Said where the account could not be made one its person can claim: switched on,
    /// and bounded against guessing at its password.
    UNGUARDED = "INVITE-10" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The media server would not ready the account to be claimed. An account is \
            offered switched on, with a limit on wrong passwords, and the media server would not \
            write either.",
        remedy: "Check the media server is running, then run this again.",
    }
    /// Said where the name given is the account that administers the media server.
    ///
    /// This is the account lemonfiber signs in as, and offering it would put a household
    /// member's limits on it.
    RUNS_THE_SERVER = "INVITE-11" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The name given is the account that administers the media server, so it is not \
            one to offer. This is the account lemonfiber signs in as, and an invitation would \
            put a household member's limits on it.",
        remedy: "Invite the person under a name of their own.",
    }
}
