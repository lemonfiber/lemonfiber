codes! {
    /// Raised when a replacement was agreed to for an offer that is not the one
    /// standing now.
    OFFER_MOVED = "MIGRATE-1" {
        severity: Warning,
        status: 400,
        since: "0.17.0",
        meaning: "What you agreed to is not what standing in place of this setup would stop now. \
            A fresh look offers something else — something started, stopped or changed since you \
            read it — so nothing was stopped. The message names what replacing would stop now.",
        remedy: "Ask what replacing would stop again, and read what it says now.",
    }
}
