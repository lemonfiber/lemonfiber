codes! {
    /// Raised when apply is asked for before the answers have been reviewed.
    NOT_REVIEWED = "SETUP-1" {
        severity: Error,
        status: 400,
        since: "0.2.0",
        meaning: "Setup was asked to apply before its answers were reviewed. Nothing has been \
            written.",
        remedy: "Answer every question, then confirm the review before applying.",
    }
    /// Raised when the operator's chosen data directory cannot be created.
    DIR_NOT_MADE = "SETUP-2" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "The data directory you chose could not be created. Setup stopped; the next run \
            recovers it.",
        remedy: "Check the location is on a writable disk, then try again.",
    }
    /// Raised when a directory from an interrupted apply could not be removed.
    NOT_REMOVED = "SETUP-3" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "A directory left by an interrupted setup could not be removed. The rest was \
            reversed; this one directory holds nothing.",
        remedy: "Remove it by hand, or leave it where it is.",
    }
    /// Raised when reversing needs the service that made a change.
    NEEDS_SERVICE = "SETUP-4" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "Reversing a change needs the service that made it. Settings and directories \
            were reversed; a resource a service created was not.",
        remedy: "Reverse it from the service itself, once that service is reachable.",
    }
    /// Raised when an answer is not meaningful on the platform setup is running on.
    DOES_NOT_APPLY = "SETUP-5" {
        severity: Error,
        status: 400,
        since: "0.2.0",
        meaning: "An answer is not meaningful on the platform setup is running on. Nothing has \
            been applied.",
        remedy: "Answer with a choice this platform offers.",
    }
    /// Raised when setup is asked to gather answers for a wizard already past it.
    ALREADY_UNDERWAY = "SETUP-6" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "Setup is past the point of gathering answers — it has been reviewed, is \
            applying, or is finished. Nothing has been changed.",
        remedy: "Resume or recover the setup in progress, or reconfigure a finished one.",
    }
    /// Raised when setup is answered on a machine that is already set up.
    ALREADY_SET_UP = "SETUP-7" {
        severity: Error,
        status: 400,
        since: "0.9.0",
        meaning: "This machine already holds configuration, so setup is not what it needs. \
            Answering setup again would walk a working stack back to its first question. Nothing \
            has been changed.",
        remedy: "Change the setting you came to change, with `lemonfiber config set <key> \
            <value>`.",
    }
    /// Raised when a recovery is asked for and no apply stopped part-way.
    NOTHING_TO_RECOVER = "SETUP-8" {
        severity: Error,
        status: 400,
        since: "0.9.0",
        meaning: "A recovery was asked for and no setup here stopped part-way through applying. \
            Recovering chooses what to do about a half-written apply, and there is none to \
            choose about. Nothing has been changed.",
        remedy: "Ask where setup stands, with `lemonfiber setup --status`, before choosing a way \
            out of it.",
    }
    /// Raised when a reversal would write over a setting the operator has since chosen.
    NOT_PUT_BACK = "SETUP-9" {
        severity: Warning,
        status: 500,
        since: "0.9.0",
        meaning: "Reversing a change left some settings exactly as they are. They no longer hold \
            what lemonfiber wrote, so they were changed after it, and putting the old value back \
            would take away a decision you made. Everything else was put back, and the message \
            names the ones it left alone.",
        remedy: "Set them back by hand if the earlier value is the one you want.",
    }
    /// Raised when a reversal meets a credential whose sealed record will not open.
    NOT_OPENED = "SETUP-10" {
        severity: Warning,
        status: 500,
        since: "0.14.0",
        meaning: "Reversing a change met a credential the journal had sealed and this machine \
            can no longer open — the key kept beside the journal is missing, or the record was \
            changed after it was written. What those settings held is not something to put back, \
            so they were left exactly as they are and the message names them. Everything else \
            was put back.",
        remedy: "Set them yourself, from wherever the earlier credential came from.",
    }
    /// Raised when a directory a reversal would remove still holds something else's files.
    STILL_HOLDING = "SETUP-11" {
        severity: Warning,
        status: 500,
        since: "0.16.0",
        meaning: "Reversing a change met directories that hold something this run did not put \
            there. A directory lemonfiber made comes off on the way back only while it is empty, \
            because taking one that holds files would take those files with it. Everything else \
            was put back, and the message names the directories.",
        remedy: "Look at what is inside, then remove them by hand if nothing there is wanted.",
    }
    /// Raised when a region a reversal would take out of a stack file cannot be.
    NOT_WITHDRAWN = "SETUP-12" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "A region lemonfiber wrote into one of the stack's files could not be taken \
            out. Everything before it was put back; the region is still in the file, between the \
            markers that name whose it is.",
        remedy: "Delete the region by hand, markers included, or run the reversal again.",
    }
    /// Raised when a file a reversal would write back to what it held cannot be.
    NOT_REWOUND = "SETUP-13" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "A file lemonfiber wrote over could not be written back to what it held. \
            Everything before it was put back; this file still holds what lemonfiber wrote, and \
            the message names it.",
        remedy: "Run the reversal again once the file can be written.",
    }
}
