//! The `PLUGIN` code of a plugin held to the recordings it ships.

codes! {
    /// Raised when a case of a contract a plugin's adapter speaks fails, or no recording
    /// the plugin ships answers it.
    UNCONFORMING = "PLUGIN-50" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "Each case of each contract a plugin's adapter speaks is answered by a \
            recording the plugin ships, and some are not: their recording fails, cannot be \
            judged, or is not there. Nothing was installed and nothing was written; the \
            message names each case.",
        remedy: "Take it up with whoever published the plugin: each case named needs a \
            recording, taken against the upstream the plugin pins, that answers as its \
            contract declares. lemonfiber plugin conform shows every case.",
    }
}
