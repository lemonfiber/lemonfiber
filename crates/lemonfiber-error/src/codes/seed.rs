codes! {
    /// Raised when a service is not answering yet.
    SERVICE_UNAVAILABLE = "SEED-1" {
        severity: Warning,
        status: 500,
        since: "0.1.0",
        meaning: "A service was not answering yet, so it was skipped. Nothing was changed for \
            it.",
        remedy: "Wait for it to finish starting, then run `lemonfiber seed` again.",
    }
    /// Raised when a service rejects the credential lemonfiber holds.
    SERVICE_UNAUTHORISED = "SEED-2" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "A service rejected the credential lemonfiber holds, usually because it was \
            changed in the service's own interface.",
        remedy: "Have lemonfiber re-read the service's credential with `lemonfiber doctor \
            --fix`.",
    }
    /// Raised when a service answers with something unusable.
    SERVICE_REFUSED = "SEED-3" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "A service answered in a way lemonfiber does not recognise, so it will not \
            guess at what would fix it.",
        remedy: "Nothing is known to fix this. Send a [support \
            bundle](https://docs.lemonfiber.app/fixing/the-support-bundle/); the service's own \
            words are attached to the message.",
    }
    /// Raised when a service does not serve the API version this build speaks.
    SERVICE_UNSUPPORTED = "SEED-4" {
        severity: Error,
        status: 500,
        since: "0.4.0",
        meaning: "A service is past — or stands before — the API version this build speaks, so \
            writing to it would mean writing something malformed. Nothing was changed for it.",
        remedy: "Match the service to the version lemonfiber supports, or update lemonfiber, \
            then run `lemonfiber seed` again.",
    }
}
