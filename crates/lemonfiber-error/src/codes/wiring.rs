codes! {
    /// Raised when a download client no longer files where lemonfiber wired it.
    DRIFTED = "WIRING-1" {
        severity: Warning,
        status: 500,
        since: "0.7.0",
        meaning: "A service and its download client have drifted apart. Either the client still \
            files under a category lemonfiber has moved on from, so anything filed since is \
            somewhere the rest of the stack no longer looks; or you moved the client off \
            lemonfiber's category and the service can no longer reach it, so the queue fills and \
            never empties.",
        remedy: "Let lemonfiber bring it up to date with `lemonfiber doctor --fix`. To keep your \
            own value instead, adopt it with `lemonfiber adopt`; to discard it and restore \
            lemonfiber's, run `lemonfiber reset --confirm`.",
    }
}
