codes! {
    /// Raised when a run is narrowed to a check nothing in this stack reports.
    NO_SUCH_CHECK = "DIAG-1" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "Nothing on this stack reports under the name you narrowed the run to. A check \
            is named by the identifier its finding carries, and no finding here carries that \
            one. Answering with an empty report would read as nothing being wrong.",
        remedy: "Run the checks with `lemonfiber doctor`, and narrow to a name this stack \
            reports.",
    }
}
