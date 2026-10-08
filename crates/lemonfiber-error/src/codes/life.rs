codes! {
    /// Raised when a service never reached a state that starting could accept.
    NEVER_SETTLED = "LIFE-1" {
        severity: Error,
        status: 500,
        leaves: NeverSettled,
        since: "0.1.0",
        meaning: "A service never reached a state that starting could accept, so the run did not \
            finish starting.",
        remedy: "Look at what the service said, then start it again: `lemonfiber logs \
            <service>`.",
    }
    /// Raised when stopping would take a service out from under a form still running.
    STILL_NEEDED = "LIFE-2" {
        severity: Error,
        status: 500,
        since: "0.8.0",
        meaning: "Stopping would take services out from under another form that is still \
            running. Nothing was stopped.",
        remedy: "Stop both if neither is wanted, or leave both up. A service two forms reach \
            belongs to whichever you are using.",
    }
    /// Another run is already working on this stack.
    ALREADY_WORKING = "LIFE-3" {
        severity: Error,
        status: 409,
        since: "0.8.0",
        meaning: "Another lemonfiber run is already working on this stack, so this one stopped \
            before doing anything. Two runs issuing commands about the same containers leave the \
            stack in a state neither asked for.",
        remedy: "Wait for the other run to finish, then run this again. If you are sure that run \
            is gone, `--force` takes the stack from it.",
    }
    /// Fetching images is switched off, so there was nothing to fetch with.
    REGISTRY_REFUSED = "LIFE-4" {
        severity: Error,
        status: 400,
        since: "0.10.0",
        meaning: "Fetching images is switched off, so nothing was fetched. \
            `LEMONFIBER_REACH_REGISTRY` is off, so this machine asks no registry for anything: a \
            start uses the images already here, and a service whose image is missing will not \
            start.",
        remedy: "Turn fetching back on with `lemonfiber config set LEMONFIBER_REACH_REGISTRY \
            on`, then run this again.",
    }
    /// Raised when a start was asked for over a data location that is not there.
    NO_DATA_LOCATION = "LIFE-5" {
        severity: Error,
        status: 500,
        since: "0.15.0",
        meaning: "The data location is not there, so nothing was started. Starting over a \
            location that is not mounted does not fail — the engine makes the directory on \
            whatever is underneath the mount point, usually the system disk, and the stack then \
            files a second library into it while the real one is offline.",
        remedy: "Connect the drive or mount holding the data location, then start again. If it \
            has moved for good, run setup and choose where it is now.",
    }
    /// Raised when the stack's own location is not on the machine being operated.
    ABSENT_THERE = "LIFE-6" {
        severity: Error,
        status: 500,
        since: "0.15.0",
        meaning: "A remote Docker context is in force, and the location the stack mounts is not \
            on that machine. It is on this one, which is why the path looks right. Nothing was \
            started, because Docker would have made an empty directory in its place and the \
            services would have come up with nothing in them.",
        remedy: "Make the location on that machine, or point lemonfiber at one that is there: \
            `lemonfiber config set DATA_ROOT <path on that machine>`.",
    }
    /// Raised when a path the stack mounts is not at the same path on the machine
    /// under the container lemonfiber runs in.
    ELSEWHERE_UNDERNEATH = "LIFE-7" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "lemonfiber runs in a container, and a path the stack mounts is somewhere else \
            on the machine underneath it, or not on that machine at all. Compose resolves every \
            path the stack mounts on the machine, so the engine would mount whatever the machine \
            keeps there, or make an empty directory in its place. Nothing was started; the \
            message names both paths.",
        remedy: "Mount the directory at the same path inside the container as on the machine, \
            and point lemonfiber at that path.",
    }
    /// Raised when lemonfiber runs in a container that cannot reach the engine.
    NO_ENGINE_IN_HERE = "LIFE-8" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "lemonfiber runs in a container and cannot reach Docker from inside it. It \
            drives the stack through the host's Docker socket, which the templates mount at \
            `/var/run/docker.sock`, and nothing answered there. Nothing was started.",
        remedy: "Mount the host's Docker socket into the container at `/var/run/docker.sock`, as \
            the templates do.",
    }
    /// Raised when the engine lemonfiber reaches from a container does not know that
    /// container.
    NOT_ON_THIS_ENGINE = "LIFE-9" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The Docker lemonfiber reaches from its container is not the one running that \
            container: the engine has no container by its name. Which paths on the machine stand \
            behind the paths the container sees cannot be asked, so Compose would resolve them \
            somewhere nobody checked. Nothing was started.",
        remedy: "Mount the socket of the engine running this container at \
            `/var/run/docker.sock`.",
    }
}
