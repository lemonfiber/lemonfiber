//! The `PLUGIN` codes of what an author asks of a plugin's own source.

codes! {
    /// Raised when a key an author names to verify a plugin's images against cannot be
    /// read, or is not a PEM public key on the P-256 curve.
    KEY_UNREADABLE = "PLUGIN-51" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A key named with --key could not be read, or is not a PEM public key on the \
            P-256 curve. Nothing was verified, because an answer checked against fewer keys \
            than were named would call an image somebody signed unproven; the message names \
            the key and what is wrong with it.",
        remedy: "Name a PEM public key on the P-256 curve with --key, or leave the key out.",
    }
}
