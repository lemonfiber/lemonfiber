codes! {
    /// Raised when a limit is expressed as a share of a line nothing has measured.
    NOTHING_MEASURED = "RATE-1" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "A limit was expressed as a share of a line nothing has measured, so the share \
            is not a limit.",
        remedy: "Say what the line carries, or give a figure instead of a share.",
    }
    /// Raised when a schedule is asked for and nothing says which zone the clients
    /// would read it in.
    NO_ZONE = "RATE-2" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "A schedule was asked for and nothing says which zone the download clients \
            would read it in.",
        remedy: "Set the zone, then ask again.",
    }
    /// Raised when what was asked for could not be read as a limit, a window or a cap.
    UNREADABLE = "RATE-3" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "What was asked for could not be read as a limit, a window or a cap. A cap has \
            to be told what happens when it is reached.",
        remedy: "Say what happens at the cap: `--when-exceeded pause`, `throttle` or `continue`.",
    }
    /// Raised when there is no download client to limit.
    NOTHING_TO_LIMIT = "RATE-4" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "There is no download client on this stack to hold to a limit.",
        remedy: "Start a form that has a download client in it.",
    }
    /// Raised when there is no download client to pause or resume.
    NOTHING_TO_PAUSE = "RATE-5" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "There is no download client on this stack to pause or resume. A pause is asked \
            of the download clients themselves, and this stack declares none, so nothing is \
            taking the line either.",
        remedy: "Start a form that has a download client in it, as in `lemonfiber up tv`.",
    }
    /// Raised where pausing or resuming the download clients names an offer that is not
    /// the one a fresh look at them builds.
    PAUSING_MOVED = "RATE-6" {
        severity: Warning,
        status: 400,
        since: "0.18.0",
        meaning: "What was agreed to is not what pausing or resuming would change now. A \
            download client was added or removed, or one changed by itself, since this was \
            rehearsed, so no client was told anything.",
        remedy: "Rehearse it again, and answer the offer it gives now.",
    }
}
