codes! {
    /// Raised when the decline service's key was used later than anything the
    /// service recorded doing with it.
    UNEXPLAINED = "DECLINE-1" {
        severity: Warning,
        status: 500,
        since: "0.17.0",
        meaning: "The decline service's key was used later than anything the service recorded \
            doing with it. That is either the service doing something it should not, or the key \
            in somebody else's hands. The message gives when it was used, and when the service \
            last recorded something, if it ever did.",
        remedy: "If nobody declined an invitation or had one run out then, rotate the key with \
            `lemonfiber credentials rotate`, and check what the decline service is running.",
    }
}
