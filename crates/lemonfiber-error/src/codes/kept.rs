codes! {
    /// Raised when this run does not know where lemonfiber's own files go.
    NOWHERE_KNOWN = "KEPT-1" {
        severity: Error,
        status: 500,
        since: "0.10.0",
        meaning: "This run cannot say where lemonfiber keeps its own files. The configuration \
            and data directories are worked out from this machine's own conventions, and that \
            did not work here, so there is nothing to list and nothing safe to remove. Guessing \
            at the usual place would risk naming a directory that is somebody else's.",
        remedy: "Run this as the account that installed lemonfiber, on a machine with a home \
            directory it can read.",
    }
}
