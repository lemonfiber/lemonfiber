codes! {
    /// Raised when the program a subprocess needs is missing.
    MISSING_PROGRAM = "PROC-1" {
        severity: Error,
        status: 500,
        leaves: Preflight,
        since: "0.1.0",
        meaning: "The program lemonfiber drives the engine through is not installed, so nothing \
            can be started or stopped.",
        remedy: "Install Docker Desktop, or Docker Engine on Linux.",
    }
    /// Raised when a program exists but will not start.
    UNUSABLE_PROGRAM = "PROC-2" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "The program is installed and would not start. Usually a permission or daemon \
            problem rather than a missing install.",
        remedy: "Check the container engine is running, then try again.",
    }
}
