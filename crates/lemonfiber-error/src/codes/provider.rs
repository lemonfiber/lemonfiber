codes! {
    /// Raised when an account has nothing left to serve.
    PROVIDER_EMPTY = "PROVIDER-1" {
        severity: Error,
        status: 500,
        since: "0.7.0",
        meaning: "A Usenet account has nothing left. It authenticates perfectly and can download \
            nothing, which looks exactly like a broken stack from the outside. A block account \
            does not refill on its own.",
        remedy: "Top the account up, or point the client at one that has data left.",
    }
    /// Raised when an account is running out, with time left to act.
    PROVIDER_LOW = "PROVIDER-2" {
        severity: Warning,
        status: 500,
        since: "0.7.0",
        meaning: "A Usenet account is running out, with time left to act. At the rate it is \
            being used it runs out shortly, and downloads will stop with nothing else having \
            changed.",
        remedy: "Top the account up before it runs out.",
    }
    /// Raised when the subscription behind an account ends soon.
    PROVIDER_ENDING = "PROVIDER-3" {
        severity: Warning,
        status: 500,
        since: "0.7.0",
        meaning: "The subscription behind a Usenet account ends on the date recorded for it in \
            the download client. When it lapses the account stops serving.",
        remedy: "Renew the subscription, or clear its date in the client if it renews itself.",
    }
    /// Raised when an indexer has been failing and its aggregator has rested it.
    INDEXER_RESTED = "PROVIDER-4" {
        severity: Warning,
        status: 500,
        since: "0.7.0",
        meaning: "An indexer has been failing and its aggregator has rested it. Searches through \
            it are not coming back; the others still are, so releases are found from a smaller \
            pool.",
        remedy: "Check the indexer's subscription and its status page, then test it in the \
            aggregator.",
    }
    /// Raised when every indexer is failing at once.
    INDEXERS_ALL_FAILING = "PROVIDER-5" {
        severity: Error,
        status: 500,
        since: "0.7.0",
        meaning: "Every indexer is failing at once. Indexers do not all fail on the same \
            afternoon, so the cause is almost always on this side of the connection.",
        remedy: "Check this machine's network and DNS, and the tunnel if searches run through \
            one.",
    }
    /// Raised when an account refuses the credential the client offers it.
    PROVIDER_REFUSED = "PROVIDER-6" {
        severity: Error,
        status: 500,
        since: "0.7.0",
        meaning: "A Usenet account is refusing the login. The provider answered the download \
            client and rejected the credentials it offered. Every service stays green while \
            nothing downloads.",
        remedy: "Check the account's username and password in the download client, and that the \
            subscription behind it is still active.",
    }
    /// Raised when an account has stopped answering the client entirely.
    PROVIDER_SILENT = "PROVIDER-7" {
        severity: Warning,
        status: 500,
        since: "0.7.0",
        meaning: "A Usenet account has stopped answering the client entirely. That is the \
            provider being down or the connection to it failing, rather than anything about the \
            account — which is worth telling apart from a rejected login before changing \
            anything.",
        remedy: "Check the provider's status page and this machine's connection. The client \
            picks the account up again on its own once it answers.",
    }
    /// Raised when the client is set to open more connections than an account allows.
    PROVIDER_CROWDED = "PROVIDER-8" {
        severity: Warning,
        status: 500,
        since: "0.7.0",
        meaning: "The download client is set to open more connections than the account allows, \
            and the provider refuses the ones beyond the plan. Downloads still run on the rest, \
            and the refusals read as an unreliable provider rather than as one setting too high.",
        remedy: "Lower the connection count for that account in the download client to what the \
            plan includes.",
    }
    /// Raised when an indexer has spent the allowance recorded against it.
    INDEXER_CAPPED = "PROVIDER-9" {
        severity: Warning,
        status: 500,
        since: "0.7.0",
        meaning: "An indexer has spent the allowance recorded against it. Searches through it \
            come back empty until it resets, and neither it nor the aggregator says so anywhere.",
        remedy: "Wait for the allowance to reset, or raise the limit recorded for that indexer \
            in the aggregator if the subscription allows more.",
    }
}
