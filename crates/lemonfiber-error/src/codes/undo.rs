codes! {
    /// Raised when no run carries the stamp a reversal was asked for.
    NO_SUCH_RUN = "UNDO-1" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "No run in the record carries the stamp you asked for. It may have fallen \
            outside the horizon the record keeps, or the stamp may be mistyped. Nothing was put \
            back.",
        remedy: "Run `lemonfiber history` and take the stamp from the entry you want.",
    }
    /// Raised when a stamp names more than one run, so which to put back is not settled.
    MORE_THAN_ONE_RUN = "UNDO-2" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "The stamp names more than one run, and putting back the wrong one is not \
            something to guess at. Nothing was put back.",
        remedy: "The message names each run the stamp covers; ask for one of them once the \
            surfaces carry it.",
    }
    /// Raised when a run cannot be put back, carrying the reason it cannot.
    CANNOT_SUCCEED = "UNDO-3" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "One change in the run cannot be reversed, and a run goes back whole or not at \
            all — so nothing was put back. The message names the change and why it will not go. \
            Raised by a command that made a change, it means the change was made and could not \
            be written to the change journal: the change stands, and cannot be put back, because \
            a reversal reads the journal.",
        remedy: "Deal with that change first, or restore from a backup. Where a change could not \
            be recorded, check that lemonfiber's configuration directory can be written and has \
            space.",
    }
    /// Raised when a run cannot say where lemonfiber's own files are.
    NOWHERE_TO_LOOK = "UNDO-4" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "This run has nowhere it knows to look for what was changed. What lemonfiber \
            changed is recorded in its own directory, and this machine would not say where that \
            is. Nothing was put back.",
        remedy: "Set a home directory for this user and run it again.",
    }
}
