codes! {
    /// Raised where the media server holds a series' episodes and answers no seasons for it.
    UNSEASONED = "LIBRARY-1" {
        severity: Warning,
        status: 500,
        since: "0.18.0",
        meaning: "The media server holds episodes for a series and answers that it has no \
            seasons, so the household sees the title and nothing to play under it. The \
            server's own records of the series have come apart from its seasons.",
        remedy: "Have the media server read the series afresh, by running \
            `lemonfiber doctor --fix`.",
    }
}
