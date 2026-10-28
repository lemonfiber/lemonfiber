codes! {
    /// Raised when a service answers and refuses the credential it generated itself.
    CREDENTIAL_REJECTED = "CRED-1" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "A service answered and refused the credential it generated itself. The key in \
            its configuration no longer matches the one the running service expects, usually \
            because the configuration was regenerated after the service last started.",
        remedy: "Restart the service so it reloads its configuration, then check again: \
            lemonfiber restart.",
    }
    /// Raised when the indexer answers and refuses the key it was given.
    INDEXER_REJECTED = "CRED-2" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "The indexer answered and rejected the API key configured for it. The key is \
            wrong, expired, or for a different indexer — searches through it come back empty.",
        remedy: "Correct the indexer's API key in configuration, then check again.",
    }
    /// Raised when the indexer authenticates the key but cannot serve it right now.
    INDEXER_LIMITED = "CRED-3" {
        severity: Warning,
        status: 500,
        since: "0.2.0",
        meaning: "The indexer accepted the key and would not serve the request — usually a rate \
            or quota limit that lifts on its own. The key is not wrong.",
        remedy: "Leave it a while and check again.",
    }
    /// Raised where a replacement of the media server's internal certificate answers an
    /// offer that is not the one a fresh reading builds.
    REPLACEMENT_MOVED = "CRED-4" {
        severity: Warning,
        status: 400,
        since: "0.18.0",
        meaning: "What was agreed to is not what replacing the certificate would do now: the \
            certificate in force, who is handed the new one, or the media server has changed \
            since it was read, so nothing was replaced.",
        remedy: "Read the replacement again, and answer the offer it gives now.",
    }
}
