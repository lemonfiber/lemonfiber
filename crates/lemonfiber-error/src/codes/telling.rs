codes! {
    /// Raised when the household is told about less than lemonfiber now sets out to tell
    /// them, through no choice of the operator's.
    BEHIND = "TELLING-1" {
        severity: Warning,
        status: 500,
        since: "0.11.0",
        meaning: "The household is told about less than lemonfiber now sets out to tell them, \
            through no choice of yours. A newer version sends more than what is currently wired \
            in.",
        remedy: "Bring what the household is told up to what lemonfiber now sends, by running \
            `lemonfiber seed`.",
    }
}
