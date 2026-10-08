codes! {
    /// The flag cannot be honoured by this command, and never will be.
    CANNOT = "REHEARSE-1" {
        severity: Error,
        status: 400,
        since: "0.15.0",
        meaning: "This command cannot be rehearsed and never will be: what it would find out is \
            only knowable by doing it, so a rehearsal would be a report with nothing in it. \
            Nothing was done. The message says which command and why.",
        remedy: "Run the command without `--dry-run` when you mean it.",
    }
    /// The flag is not honoured by this command yet.
    NOT_YET = "REHEARSE-2" {
        severity: Error,
        status: 400,
        since: "0.15.0",
        meaning: "This command changes things and has not been taught to say what it would \
            change, so it refuses the flag rather than accepting it and going ahead. Nothing was \
            done.",
        remedy: "Run the command without `--dry-run` when you mean it, or wait for it.",
    }
}
