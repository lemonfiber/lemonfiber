codes! {
    /// Raised when the Docker client is not installed.
    DOCKER_ABSENT = "ENV-1" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "Docker is not installed. lemonfiber runs your stack in containers, so nothing \
            can start without an engine.",
        remedy: "Install Docker Desktop, or Docker Engine on Linux.",
    }
    /// Raised when the Docker client is present but its daemon is not answering.
    DAEMON_DOWN = "ENV-2" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "Docker is installed and its daemon is not answering, or would not start. The \
            client being present usually means this is the daemon stopped, or a permission \
            problem, rather than a missing install.",
        remedy: "Start Docker Desktop, or the `docker` service on Linux. If it is running, check \
            that your account may run `docker`.",
    }
    /// Raised when the Compose plugin is missing or too old to drive.
    COMPOSE_UNUSABLE = "ENV-3" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "The Docker Compose plugin is missing, or is too old. lemonfiber drives the \
            stack through Compose v2.",
        remedy: "Install or update the Docker Compose plugin. The message names the minimum \
            version.",
    }
    /// Raised when the container engine is confirmed not to start with this machine.
    ///
    /// One code for both arrangements it can be. What is not set differs by platform —
    /// Docker Desktop's open-at-login setting on macOS and Windows, the daemon's own unit
    /// on native Linux — and what has gone wrong is the same thing either way: nothing
    /// brings the engine up, so nothing reads the restart policies that would bring the
    /// containers back.
    ENGINE_NOT_AT_BOOT = "ENV-4" {
        severity: Warning,
        status: 500,
        since: "0.15.0",
        meaning: "The container engine is not set to start with this machine. The containers \
            carry a restart policy, and a restart policy only brings a container back once the \
            engine behind it is running — so after the next restart the stack is simply not \
            there, with no error and nothing in any log you would think to look at.",
        remedy: "Turn the engine's own start-at-boot on: Docker Desktop's open-at-login setting \
            on a Mac and on Windows, the daemon's unit on Linux. The message names which.",
    }
    /// Raised when this machine and the daemon speak different Docker API generations.
    API_MISMATCH = "ENV-5" {
        severity: Warning,
        status: 500,
        since: "0.15.0",
        meaning: "This machine and the daemon speak different Docker API generations. Both ends \
            settle on the older of the two, so everything works and anything newer than that \
            generation is not available.",
        remedy: "Bring both to the same Docker release, or carry on with the older set. It is \
            ordinary where the two machines were updated at different times.",
    }
}
