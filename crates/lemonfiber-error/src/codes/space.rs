codes! {
    /// Raised when the volume is full and new acquisitions are therefore halted.
    HALTED = "SPACE-1" {
        severity: Critical,
        status: 500,
        since: "0.12.0",
        meaning: "There is no room left, so nothing new is being fetched. A service that cannot \
            write its database can take the file with it, which turns a full disk into work that \
            is gone — fetching more onto it is what this prevents.",
        remedy: "Free space, then run this again. `lemonfiber space` names an offer, and \
            answering it takes what it offers.",
    }
    /// Raised when there is no data location to measure.
    NOWHERE_TO_MEASURE = "SPACE-2" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "No data location is configured, so there is no disk to account for.",
        remedy: "Set the data location, with `lemonfiber setup`.",
    }
    /// Raised when the data location is there and could not be read.
    WALK_REFUSED = "SPACE-3" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "The data location is there and could not be read.",
        remedy: "Check that the account lemonfiber runs as can read the data location.",
    }
    /// Raised when there is no torrent client here to be holding a completed download.
    NOTHING_TO_ASK = "SPACE-4" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "There is no torrent client here to be holding a completed download. Seeding is \
            a torrent client's business, and this stack has none lemonfiber can reach and prove \
            itself to.",
        remedy: "Check the download client is running and lemonfiber knows its password. \
            `lemonfiber doctor` says which.",
    }
    /// Raised when the client answers and is holding nothing of the name given.
    NOT_HELD = "SPACE-5" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "The client answered and is holding nothing of the name given. One that has \
            finished seeding, or was removed already, is not there to be removed again.",
        remedy: "Read the account and name one of the completed downloads it lists, with \
            `lemonfiber space`.",
    }
    /// Raised when an agreement names an offer that is not the one standing now.
    ANOTHER_OFFER = "SPACE-6" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "The answer names an offer that is not the one standing now. What an offer \
            covers is in the name it goes by — for one download, what it occupies, where it \
            stands and the ratio it has earned; for a cleanup, every path it would take and what \
            each occupies — so an offer that has moved since it was read is a different offer.",
        remedy: "Read the offer again, and answer the name it prints.",
    }
    /// Raised when the client could not be reached, or would not let a download go.
    STILL_HELD = "SPACE-7" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "The client could not be reached, or would not let the download go. It is still \
            being seeded and the room is still spent, which is the honest reading.",
        remedy: "Check the download client is answering, then answer the offer again.",
    }
}
