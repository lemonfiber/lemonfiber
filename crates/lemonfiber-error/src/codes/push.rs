codes! {
    /// Raised when a push endpoint is registered under a word that cannot name one.
    NOT_A_PUSH_NAME = "PUSH-1" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That word cannot name a push endpoint. Its name travels in an address and the \
            journal, so it is lower-case letters, digits, dots, dashes and underscores, beginning \
            with a letter or a digit, and at most 64 characters.",
        remedy: "Name it after the phone it wakes, as in pixel-8.",
    }
    /// Raised when a push endpoint is registered under a name another holds.
    PUSH_NAME_TAKEN = "PUSH-2" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "A push endpoint by that name is already registered, and the message names it.",
        remedy: "Remove that one, or register this one under another name.",
    }
    /// Raised when a push endpoint's address is not an `https` address.
    PUSH_NOT_HTTPS = "PUSH-3" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A push endpoint is reached over https and nothing else, and the message names \
            the one that is not.",
        remedy: "Register the https address the phone's distributor gives.",
    }
    /// Raised when the keys a push endpoint gave cannot be encrypted to.
    PUSH_KEYS_UNREADABLE = "PUSH-4" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "The message would be encrypted to the keys given, and these are not a P-256 \
            public key and a 16-byte secret, each in base64url. Nothing was registered.",
        remedy: "Make a new WebPush key pair on the phone, and register that.",
    }
    /// Raised when no push endpoint holds the name asked about.
    PUSH_NOT_REGISTERED = "PUSH-5" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "No push endpoint holds that name, so nothing was removed.",
        remedy: "List the push endpoints, and name one of them.",
    }
}
