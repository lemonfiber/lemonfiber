codes! {
    /// Raised when a watch is asked for but no data location is configured to watch.
    NOTHING_TO_WATCH = "WATCH-1" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "No data location is configured, so there is nothing for a watch to guard.",
        remedy: "Run `lemonfiber setup` to choose a data location, then start the watch again.",
    }
    /// Raised when the data location is already gone when the watch is asked to
    /// start.
    ALREADY_GONE = "WATCH-2" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "The data location is already gone when the watch was asked to start. A watch \
            can only guard a location that is present when it begins.",
        remedy: "Connect the drive or mount holding the data location, then start the watch.",
    }
}
