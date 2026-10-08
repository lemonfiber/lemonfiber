codes! {
    /// Raised when an answer names something nothing is warning about.
    NOT_WARNED = "ACK-1" {
        severity: Error,
        status: 500,
        since: "0.6.0",
        meaning: "The check you named is not something this run is warning about. An answer is \
            only meaningful against something the tool is currently saying.",
        remedy: "Answer one of the warnings this run raised — the message lists them. If it \
            raised none, run the checks first.",
    }
}
