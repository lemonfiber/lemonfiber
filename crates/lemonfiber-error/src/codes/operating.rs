//! The codes about lemonfiber itself and the stack it runs: its settings, the
//! engine and the programs it drives, bringing services up, and what it serves.

codes! {
    /// The `ACK` codes.
    ack {
        /// Raised when an answer names something nothing is warning about.
        NOT_WARNED = "ACK-1",
    }
    /// The `BIND` codes.
    bind {
        /// Raised when a service the stack calls admin answers somewhere off this machine.
        BEYOND_LOOPBACK = "BIND-1",
        /// Raised where a published port is reached without the host's own firewall rules
        /// being consulted.
        AROUND_THE_FIREWALL = "BIND-2",
        /// Raised where an admin service answers off this machine and the operator has
        /// written down that they meant it to.
        DELIBERATE = "BIND-3",
    }
    /// The `CONFIG` codes.
    config {
        /// Raised when configuration exists and cannot be read.
        CONFIG_UNREADABLE = "CONFIG-1" => Validation,
        /// Raised when configuration cannot be written.
        CONFIG_NOT_WRITTEN = "CONFIG-2",
        /// Raised when there is nowhere to keep configuration.
        CONFIG_NOWHERE = "CONFIG-3",
        /// Raised when a file holding a credential can be read by more than its owner.
        CREDENTIALS_EXPOSED = "CONFIG-4",
        /// Raised when configuration was written by a newer lemonfiber.
        CONFIG_TOO_NEW = "CONFIG-5",
        /// Raised when a setting's key or value spans more than one line.
        CONFIG_SPANS_LINES = "CONFIG-6",
    }
    /// The `DIAG` codes.
    diag {
        /// Raised when a run is narrowed to a check nothing in this stack reports.
        NO_SUCH_CHECK = "DIAG-1",
    }
    /// The `DOCKER` codes.
    docker {
        /// Raised when the container engine cannot be reached.
        ENGINE_UNREACHABLE = "DOCKER-1" => Preflight,
        /// Raised when a container that should exist does not.
        NO_SUCH_CONTAINER = "DOCKER-2",
        /// Raised when the host an endpoint names cannot be found on the network.
        HOST_UNRESOLVED = "DOCKER-3",
        /// Raised when the host is found and refuses the connection.
        HOST_REFUSED = "DOCKER-4",
        /// Raised when the host is reached and will not accept the SSH login.
        LOGIN_REJECTED = "DOCKER-5",
        /// Raised when an endpoint names a transport this build cannot drive.
        ENDPOINT_UNSUPPORTED = "DOCKER-6",
        /// Raised when a named Docker context is not one this machine records.
        UNKNOWN_CONTEXT = "DOCKER-7",
        /// Raised when a remote host does not answer for a reason nothing here recognises.
        HOST_SILENT = "DOCKER-8",
    }
    /// The `ENV` codes.
    env {
        /// Raised when the Docker client is not installed.
        DOCKER_ABSENT = "ENV-1",
        /// Raised when the Docker client is present but its daemon is not answering.
        DAEMON_DOWN = "ENV-2",
        /// Raised when the Compose plugin is missing or too old to drive.
        COMPOSE_UNUSABLE = "ENV-3",
        /// Raised when the container engine is confirmed not to start with this machine.
        ///
        /// One code for both arrangements it can be. What is not set differs by platform —
        /// Docker Desktop's open-at-login setting on macOS and Windows, the daemon's own unit
        /// on native Linux — and what has gone wrong is the same thing either way: nothing
        /// brings the engine up, so nothing reads the restart policies that would bring the
        /// containers back.
        ENGINE_NOT_AT_BOOT = "ENV-4",
        /// Raised when this machine and the daemon speak different Docker API generations.
        API_MISMATCH = "ENV-5",
    }
    /// The `FORM` codes.
    form {
        /// Raised when no form was named.
        NO_FORM_NAMED = "FORM-1",
        /// Raised when a named form is not declared by the stack.
        NO_SUCH_FORM = "FORM-2",
        /// Raised when forms that cannot be combined are named together.
        FORMS_CONFLICT = "FORM-3",
        /// Raised when narrowing leaves nothing to run.
        NOTHING_TO_RUN = "FORM-4",
    }
    /// The `HOST` codes.
    host {
        /// Raised where the platform has no service manager lemonfiber can configure.
        NOTHING_TO_HOST_WITH = "HOST-1",
        /// Raised where a service definition could not be written.
        DEFINITION_UNWRITABLE = "HOST-2",
        /// Raised where the service manager refused what it was asked.
        MANAGER_REFUSED = "HOST-3",
        /// Raised when this machine will not say where it keeps its own files.
        NOWHERE_TO_WRITE = "HOST-4",
        /// Raised when this run cannot say where its own program is.
        NO_PROGRAM = "HOST-5",
        /// Raised when the guard is to be hosted against nothing.
        NOTHING_NAMED_TO_GUARD = "HOST-6",
    }
    /// The `LIFE` codes.
    life {
        /// Raised when a service never reached a state that starting could accept.
        NEVER_SETTLED = "LIFE-1" => NeverSettled,
        /// Raised when stopping would take a service out from under a form still running.
        STILL_NEEDED = "LIFE-2",
        /// Another run is already working on this stack.
        ALREADY_WORKING = "LIFE-3",
        /// Fetching images is switched off, so there was nothing to fetch with.
        REGISTRY_REFUSED = "LIFE-4",
        /// Raised when a start was asked for over a data location that is not there.
        NO_DATA_LOCATION = "LIFE-5",
        /// Raised when the stack's own location is not on the machine being operated.
        ABSENT_THERE = "LIFE-6",
    }
    /// The `PROC` codes.
    proc {
        /// Raised when the program a subprocess needs is missing.
        MISSING_PROGRAM = "PROC-1" => Preflight,
        /// Raised when a program exists but will not start.
        UNUSABLE_PROGRAM = "PROC-2",
    }
    /// The `READ` codes.
    read {
        /// Raised where a read was given a parameter its answer has nowhere to put.
        UNWANTED = "READ-1",
        /// Raised where a parameter carrying one value was given more than once.
        REPEATED = "READ-2",
    }
    /// The `REHEARSE` codes.
    rehearse {
        /// The flag cannot be honoured by this command, and never will be.
        CANNOT = "REHEARSE-1",
        /// The flag is not honoured by this command yet.
        NOT_YET = "REHEARSE-2",
    }
    /// The `SERVE` codes.
    serve {
        /// Raised when the address the surface was asked to serve on cannot be taken.
        ADDRESS_TAKEN = "SERVE-1",
        /// Raised when this machine will not supply the randomness a token is made of.
        NO_TOKEN = "SERVE-2",
        /// Raised when the network was asked for and nothing here can say who is knocking.
        NO_PASSWORD = "SERVE-3",
    }
    /// The `SETUP` codes.
    setup {
        /// Raised when apply is asked for before the answers have been reviewed.
        NOT_REVIEWED = "SETUP-1",
        /// Raised when the operator's chosen data directory cannot be created.
        DIR_NOT_MADE = "SETUP-2",
        /// Raised when a directory from an interrupted apply could not be removed.
        NOT_REMOVED = "SETUP-3",
        /// Raised when reversing needs the service that made a change.
        NEEDS_SERVICE = "SETUP-4",
        /// Raised when an answer is not meaningful on the platform setup is running on.
        DOES_NOT_APPLY = "SETUP-5",
        /// Raised when setup is asked to gather answers for a wizard already past it.
        ALREADY_UNDERWAY = "SETUP-6",
        /// Raised when setup is answered on a machine that is already set up.
        ALREADY_SET_UP = "SETUP-7",
        /// Raised when a recovery is asked for and no apply stopped part-way.
        NOTHING_TO_RECOVER = "SETUP-8",
        /// Raised when a reversal would write over a setting the operator has since chosen.
        NOT_PUT_BACK = "SETUP-9",
        /// Raised when a reversal meets a credential whose sealed record will not open.
        NOT_OPENED = "SETUP-10",
        /// Raised when a directory a reversal would remove still holds something else's files.
        STILL_HOLDING = "SETUP-11",
        /// Raised when a region a reversal would take out of a stack file cannot be.
        NOT_WITHDRAWN = "SETUP-12",
    }
    /// The `STACK` codes.
    stack {
        /// Raised when a stack directory holds no readable manifest.
        STACK_UNREADABLE = "STACK-1" => Validation,
        /// Raised when a manifest is readable and this build cannot use it.
        STACK_UNUSABLE = "STACK-2",
        /// Raised when the embedded stack is not intact.
        STACK_NOT_EMBEDDED = "STACK-3",
        /// Raised when lemonfiber has nowhere to write the stack.
        STACK_NOT_SET_UP = "STACK-4",
        /// Raised when the stack could not be written to disk.
        STACK_NOT_WRITTEN = "STACK-5",
        /// Raised when a manifest parses and breaks the contract.
        STACK_INVALID = "STACK-6" => Validation,
        /// Raised when a manifest is not TOML at all.
        STACK_MALFORMED = "STACK-7" => Validation,
        /// Raised when a manifest declares names this build does not know.
        STACK_UNRECOGNISED = "STACK-8" => Validation,
    }
    /// The `TELLING` codes.
    telling {
        /// Raised when the household is told about less than lemonfiber now sets out to tell
        /// them, through no choice of the operator's.
        BEHIND = "TELLING-1",
    }
    /// The `TUI` codes.
    tui {
        /// A screen that could not be drawn, as a problem rather than a panic.
        DRAWING = "TUI-1",
    }
    /// The `UPDATE` codes.
    update {
        /// Raised when what this machine has pulled could not be read.
        NOT_CHECKED = "UPDATE-1",
        /// Raised when the service an update was narrowed to is not one the stack declares.
        NO_SUCH_SERVICE = "UPDATE-2",
        /// Raised when transfers are still in flight and the run was not asked to wait.
        STILL_TRANSFERRING = "UPDATE-3",
        /// Raised when the stack came down for the capture and the capture would not write.
        CAPTURE_LEFT_IT_DOWN = "UPDATE-4",
    }
    /// The `WATCH` codes.
    watch {
        /// Raised when a watch is asked for but no data location is configured to watch.
        NOTHING_TO_WATCH = "WATCH-1",
        /// Raised when the data location is already gone when the watch is asked to
        /// start.
        ALREADY_GONE = "WATCH-2",
    }
    /// The `WORD` codes.
    word {
        /// A word this product does not explain, as a refusal that says what it does.
        ///
        /// Through the error model rather than a bare line, so it carries a code and a way
        /// forward like every other refusal — and the way forward is the list itself, which
        /// is short enough to be the answer rather than a pointer at one.
        ///
        /// It lies in the naming: the word is the whole of what was asked for, and there is
        /// no entry for it. A surface that reported this as its own failure would be telling
        /// a caller to try again at something that will never work.
        UNRECOGNISED = "WORD-1",
    }
}
