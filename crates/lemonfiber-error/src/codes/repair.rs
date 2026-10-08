codes! {
    /// Raised when consent was given for an offer that no longer stands.
    STALE = "REPAIR-1" {
        severity: Warning,
        status: 400,
        since: "0.9.0",
        meaning: "What you agreed to is not what is offered now. A fresh look offers something \
            else, so something changed between reading the offer and answering it. Nothing was \
            carried out.",
        remedy: "Ask what could be put right again, and read what it says now.",
    }
    /// Raised when a run cannot say where lemonfiber's own files are.
    NOWHERE_TO_LOOK = "REPAIR-2" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "This run has nowhere it knows to look for what a repair changed. What each \
            repair changed is recorded in lemonfiber's own directory, and this machine would not \
            say where that is. Nothing was put back.",
        remedy: "Set a home directory for this user, then ask again.",
    }
    /// Raised when a run that may not act was asked for the checks that disturb.
    OFFER_CANNOT_DISTURB = "REPAIR-3" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "Saying what could be put right does not include the checks that disturb. Those \
            checks prove themselves by disturbing — the killswitch test takes the tunnel away \
            from the download client, and the release check spends one of the indexers' daily \
            searches — and a run that only says what it would put right has agreed to neither. \
            Nothing was disturbed.",
        remedy: "Ask for the diagnosis with those checks in it, with `lemonfiber doctor \
            --disruptive`. Or agree to the repairs first, and the checks that disturb run with \
            them.",
    }
}
