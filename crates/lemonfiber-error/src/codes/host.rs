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
        household: fault,
    }
    /// Raised where a service definition could not be written.
    DEFINITION_UNWRITABLE = "HOST-2" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "A service definition could not be written. Nothing was installed, so nothing \
            is running and nothing was left behind.",
        remedy: "Check that the directory exists and belongs to you, then try again.",
        household: fault,
    }
    /// Raised where the service manager refused what it was asked.
    MANAGER_REFUSED = "HOST-3" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "The service manager refused what it was asked. The definition that had been \
            written was removed again, so nothing is half-installed.",
        remedy: "Read what it said below, then try again once that is dealt with.",
        household: fault,
    }
    /// Raised when this machine will not say where it keeps its own files.
    NOWHERE_TO_WRITE = "HOST-4" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "This machine will not say where lemonfiber keeps its own files.",
        remedy: "Set a home directory for this account, then install it again.",
        household: fault,
    }
    /// Raised when this run cannot say where its own program is.
    NO_PROGRAM = "HOST-5" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "This run cannot say where its own program is, so there is nothing to name in a \
            service definition.",
        remedy: "Run this again from an installed copy of lemonfiber rather than a piped one.",
        household: fault,
    }
    /// Raised when the guard is to be hosted against nothing.
    NOTHING_NAMED_TO_GUARD = "HOST-6" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "The guard is to be hosted against nothing — it was not told what to guard.",
        remedy: "Name the forms to guard, as you would when running the guard yourself.",
        household: fault,
    }
    /// Raised when the service named to run on its schedule is no run-and-exit service.
    NOT_SCHEDULED = "HOST-7" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "The service named is no installed run-and-exit service, so it has no times to \
            run at.",
        remedy: "Name a service an installed plugin declares a schedule for.",
        household: fault,
    }
    /// Raised when a run-and-exit service's last run did not exit zero, or its schedule
    /// names no time.
    SCHEDULED_RUN_FAILED = "HOST-8" {
        severity: Warning,
        status: 500,
        since: "0.18.0",
        meaning: "What a run-and-exit service runs to do was not done: its last run did not exit \
            zero, or its schedule setting names no time it runs at.",
        remedy: "Read what its last run wrote, deal with it, then run it again.",
        household: fault,
    }
}
