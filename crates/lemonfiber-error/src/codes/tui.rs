codes! {
    /// A screen that could not be drawn, as a problem rather than a panic.
    DRAWING = "TUI-1" {
        severity: Error,
        status: 500,
        since: "0.6.0",
        meaning: "A screen could not be drawn. The terminal stopped accepting output, which \
            usually means it was closed or resized out from under the process.",
        remedy: "Run it again in a terminal that stays open.",
    }
}
