codes! {
    /// Raised when the request gate refused calls since the last diagnosis: the
    /// request service asked for something it has no use for.
    REFUSED = "GATE-1" {
        severity: Warning,
        status: 500,
        since: "0.17.0",
        meaning: "The request gate refused calls since the last check: something reaching the \
            request service asked Sonarr, Radarr or Jellyfin for more than requests need, and \
            the gate stopped it. The message lists the calls.",
        remedy: "If nobody changed the request service's settings, check what it is running.",
    }
    /// Raised when more entries reached the gate's record between two diagnoses than
    /// it keeps, so some could not be reported.
    LOST = "GATE-2" {
        severity: Warning,
        status: 500,
        since: "0.17.0",
        meaning: "More entries reached the gate's record between two checks than it keeps, so \
            some were dropped before they were read. What those entries were cannot be told now; \
            the message says how many.",
        remedy: "Run the diagnosis more often while this keeps happening.",
    }
}
