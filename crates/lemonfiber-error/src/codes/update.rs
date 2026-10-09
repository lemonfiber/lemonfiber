codes! {
    /// Raised when what this machine has pulled could not be read.
    NOT_CHECKED = "UPDATE-1" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "What is running could not be read, so no update was worked out. Which version \
            each service stands on is the container engine's answer, and it did not give one. \
            Nothing was changed.",
        remedy: "Start the container engine, then ask again.",
    }
    /// Raised when the service an update was narrowed to is not one the stack declares.
    NO_SUCH_SERVICE = "UPDATE-2" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "An update was narrowed to one service by a name the stack does not declare. A \
            name matching nothing would otherwise read as a stack already up to date, so it is \
            refused instead. Nothing was changed.",
        remedy: "Name one of the services the stack declares; the message lists them.",
    }
    /// Raised when transfers are still in flight and the run was not asked to wait.
    STILL_TRANSFERRING = "UPDATE-3" {
        severity: Warning,
        status: 500,
        since: "0.14.0",
        meaning: "Something is still coming down, and the run was not asked to wait. Updating \
            stops the download clients, and what is part-way down does not always resume where \
            it left off — so a run that would interrupt one is refused rather than carried out. \
            Nothing was changed.",
        remedy: "Let them finish with `lemonfiber update stack --confirm --wait`, or wait and \
            ask again.",
    }
    /// Raised when the stack came down for the capture and the capture would not write.
    CAPTURE_LEFT_IT_DOWN = "UPDATE-4" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "The stack was stopped so the backup could run against databases nothing was \
            writing to, and the backup would not write. Nothing was updated and nothing opened \
            its state on a newer image, so there is nothing to undo — but the stack is down, \
            because stopping it is what came first.",
        remedy: "Bring the stack back up with `lemonfiber up`, then fix what stopped the capture \
            and ask again.",
    }
    /// Raised where an update names an offer that is not the one a fresh look at the
    /// releases builds.
    UPDATE_MOVED = "UPDATE-5" {
        severity: Warning,
        status: 400,
        since: "0.18.0",
        meaning: "What was agreed to is not what the update would apply now. A newer release \
            arrived, or one was withdrawn, since the update was rehearsed, so nothing was \
            updated.",
        remedy: "Rehearse the update again, and answer the offer it gives now.",
    }
}
