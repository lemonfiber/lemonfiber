codes! {
    /// Raised when the tier that takes the library was confirmed without its own
    /// agreement.
    NEEDS_AGREEING = "GONE-1" {
        severity: Error,
        status: 500,
        since: "0.13.0",
        meaning: "The removal that takes the library was confirmed without its own agreement. \
            Destroying a library takes an answer given to that reading and no other.",
        remedy: "Read what would go, then answer that reading by its own name, as in `lemonfiber \
            uninstall media --agreed <name>`.",
    }
    /// Raised when an agreement names a reading of this machine that is not the one
    /// standing now.
    ANOTHER_READING = "GONE-2" {
        severity: Error,
        status: 400,
        since: "0.13.0",
        meaning: "The agreement names a reading of this machine that is not the one standing \
            now, so acting on it would act on something nobody saw.",
        remedy: "Read it again, and answer the name it prints.",
    }
    /// Raised when the backup a destructive removal takes first could not be taken.
    NOT_BACKED_UP = "GONE-3" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "The backup that comes before a destructive removal could not be taken, so \
            nothing was removed. What these removals destroy cannot be made again, so it is \
            taken behind a backup or not at all.",
        remedy: "Deal with whatever stopped the backup — the message carries its reason — then \
            ask for the removal again.",
    }
}
