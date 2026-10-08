codes! {
    /// Raised when the container engine cannot be reached.
    ENGINE_UNREACHABLE = "DOCKER-1" {
        severity: Error,
        status: 500,
        leaves: Preflight,
        since: "0.1.0",
        meaning: "The container engine is not running. Nothing about your stack can be read or \
            changed while it is down.",
        remedy: "Start Docker Desktop, or the `docker` service on Linux. This is the first thing \
            to fix.",
    }
    /// Raised when a container that should exist does not.
    NO_SUCH_CONTAINER = "DOCKER-2" {
        severity: Warning,
        status: 500,
        since: "0.1.0",
        meaning: "A container that should be up is not. It may have stopped on its own, or never \
            been started.",
        remedy: "Start the form that includes it. `lemonfiber ps` shows what is running.",
    }
    /// Raised when the host an endpoint names cannot be found on the network.
    HOST_UNRESOLVED = "DOCKER-3" {
        severity: Error,
        status: 500,
        since: "0.15.0",
        meaning: "The host an endpoint names could not be found on the network. The name was \
            looked up and nothing answered to it, so no connection was attempted — the daemon \
            may be running perfectly on a machine this one cannot name.",
        remedy: "Check the host name, and that this machine can resolve it. Trying the host's \
            address in place of its name tells the two apart.",
    }
    /// Raised when the host is found and refuses the connection.
    HOST_REFUSED = "DOCKER-4" {
        severity: Error,
        status: 500,
        since: "0.15.0",
        meaning: "The host was found and refused the connection. The name is right and something \
            about the endpoint is not: either Docker is not running over there, or it is not \
            listening where the endpoint says.",
        remedy: "Check Docker is running on that machine, and on the port the endpoint names.",
    }
    /// Raised when the host is reached and will not accept the SSH login.
    LOGIN_REJECTED = "DOCKER-5" {
        severity: Error,
        status: 500,
        since: "0.15.0",
        meaning: "The host was reached and would not accept the SSH login. This is about keys \
            and accounts rather than about Docker — lemonfiber uses the SSH configuration you \
            already have and makes no keys of its own.",
        remedy: "Connect to the host with `ssh` and read what it says, then try again.",
    }
    /// Raised when an endpoint names a transport this build cannot drive.
    ENDPOINT_UNSUPPORTED = "DOCKER-6" {
        severity: Error,
        status: 500,
        since: "0.15.0",
        meaning: "The endpoint names a transport lemonfiber cannot drive. Nothing was read and \
            nothing was changed, because reading one machine while writing to another is worse \
            than reaching neither.",
        remedy: "Point `DOCKER_HOST` at an `ssh://` or `tcp://` endpoint, or at a local socket.",
    }
    /// Raised when a named Docker context is not one this machine records.
    UNKNOWN_CONTEXT = "DOCKER-7" {
        severity: Error,
        status: 500,
        since: "0.15.0",
        meaning: "A Docker context was named and this machine records no endpoint under that \
            name. Nothing was read and nothing was changed, because falling back to the local \
            daemon would answer about this machine while you were asking about another.",
        remedy: "List the contexts this machine has with `docker context ls`, and name one of \
            those.",
    }
    /// Raised when a remote host does not answer for a reason nothing here recognises.
    HOST_SILENT = "DOCKER-8" {
        severity: Error,
        status: 500,
        since: "0.15.0",
        meaning: "A remote host did not answer, for a reason nothing here recognises. What the \
            transport said is quoted beside it; it names that machine rather than this one.",
        remedy: "Read what the transport said. It is the most specific thing known about this.",
    }
}
