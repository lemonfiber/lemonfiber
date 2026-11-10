codes! {
    /// Raised when a back-end is added under a word that cannot name one.
    NOT_A_BACKEND_NAME = "NOTIFY-1" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That word cannot name a back-end. Its name travels in an address and the \
            journal, so it is lower-case letters, digits, dots, dashes and underscores, beginning \
            with a letter or a digit, and at most 64 characters.",
        remedy: "Name it after where it delivers, as in home-ntfy.",
    }
    /// Raised when a back-end is added under a name another holds.
    BACKEND_NAME_TAKEN = "NOTIFY-2" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "A back-end by that name is already added, and the message names it.",
        remedy: "Remove that one, or add this one under another name.",
    }
    /// Raised when no back-end holds the name asked about.
    NO_SUCH_BACKEND = "NOTIFY-3" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "No back-end holds that name, so nothing was changed.",
        remedy: "List the back-ends, and name one of them.",
    }
    /// Raised when a back-end on a host that is not the household's own is added without
    /// saying so.
    HOSTED_UNACKNOWLEDGED = "NOTIFY-4" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That address is a hosted service rather than one the household runs, so what \
            is delivered to it leaves the self-hosted posture. Nothing was added.",
        remedy: "Add it again with --hosted to accept that, or point it at a server the \
            household runs.",
    }
    /// Raised when a back-end's address is not one it can be reached at.
    NOT_A_BACKEND_ADDRESS = "NOTIFY-5" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A back-end is reached at an http or https address with no login, query or \
            fragment in it, and the message names the one that is not. A token is given apart \
            from the address.",
        remedy: "Give the server's address, and its token with --token-env.",
    }
    /// Raised when a back-end is added without what its kind needs.
    BACKEND_INCOMPLETE = "NOTIFY-6" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "An ntfy back-end needs a topic and a gotify back-end needs its application \
            token, and the message names what is missing. Nothing was added.",
        remedy: "Add it again with what the message names.",
    }
    /// Raised when a back-end's attempts and delays do not agree.
    NOT_A_RETRY_SCHEDULE = "NOTIFY-7" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A delivery is given between one and ten attempts, and every attempt after the \
            first waits a number of minutes given in --delays, one for each. Nothing was changed.",
        remedy: "Give as many delays as there are attempts after the first, as in --attempts 3 \
            --delays 1,10.",
    }
    /// Raised when a member chooses a back-end that is not offered to members.
    NOT_OFFERED = "NOTIFY-8" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "Only a way to be told that whoever runs the house offered to everybody can be \
            chosen, and that one was not. Nothing was changed.",
        remedy: "Choose from the ones offered.",
    }
    /// Raised when a phone confirms a wake nobody sent it.
    NO_SUCH_WAKE = "NOTIFY-9" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "No wake by that id was sent to that endpoint, so there was nothing to confirm.",
        remedy: "Confirm the id the wake carried, from the phone it woke.",
    }
    /// Raised when an event arrives without the key the core gave its sender.
    EVENTS_REFUSED = "NOTIFY-10" {
        severity: Error,
        status: 401,
        since: "0.18.0",
        meaning: "Events are taken only from a curator or the media server, with the key the core \
            gave it when it wired the stack. Nothing was taken.",
        remedy: "Wire the stack again, so the service is handed its key.",
    }
}
