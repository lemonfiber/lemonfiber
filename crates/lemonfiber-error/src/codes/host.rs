codes! {
    /// Raised where the platform has no service manager lemonfiber can configure.
    NOTHING_TO_HOST_WITH = "HOST-1" {
        severity: Warning,
        status: 500,
        since: "0.12.0",
        meaning: "This machine has no service manager lemonfiber can configure. The command can \
            still be run, and it will still stop when the terminal running it closes.",
        remedy: "Keep the command running yourself, or arrange it with whatever this system uses \
            to start things at login.",
    }
    /// Raised where a service definition could not be written.
    DEFINITION_UNWRITABLE = "HOST-2" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "A service definition could not be written. Nothing was installed, so nothing \
            is running and nothing was left behind.",
        remedy: "Check that the directory exists and belongs to you, then try again.",
    }
    /// Raised where the service manager refused what it was asked.
    MANAGER_REFUSED = "HOST-3" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "The service manager refused what it was asked. The definition that had been \
            written was removed again, so nothing is half-installed.",
        remedy: "Read what it said below, then try again once that is dealt with.",
    }
    /// Raised when this machine will not say where it keeps its own files.
    NOWHERE_TO_WRITE = "HOST-4" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "This machine will not say where lemonfiber keeps its own files.",
        remedy: "Set a home directory for this account, then install it again.",
    }
    /// Raised when this run cannot say where its own program is.
    NO_PROGRAM = "HOST-5" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "This run cannot say where its own program is, so there is nothing to name in a \
            service definition.",
        remedy: "Run this again from an installed copy of lemonfiber rather than a piped one.",
    }
    /// Raised when the guard is to be hosted against nothing.
    NOTHING_NAMED_TO_GUARD = "HOST-6" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "The guard is to be hosted against nothing — it was not told what to guard.",
        remedy: "Name the forms to guard, as you would when running the guard yourself.",
    }
}
