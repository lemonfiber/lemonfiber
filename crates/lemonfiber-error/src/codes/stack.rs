codes! {
    /// Raised when a stack directory holds no readable manifest.
    STACK_UNREADABLE = "STACK-1" {
        severity: Error,
        status: 500,
        leaves: Validation,
        since: "0.1.0",
        meaning: "No readable manifest was found where a stack was expected. A stack directory \
            holds a stack.toml describing its services.",
        remedy: "Point at a directory containing stack.toml, with lemonfiber --stack-dir \
            <path>.",
    }
    /// Raised when a manifest is readable and this build cannot use it.
    STACK_UNUSABLE = "STACK-2" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "The manifest is readable and was written for a different version of \
            lemonfiber.",
        remedy: "Update lemonfiber, or point at a stack this version reads.",
    }
    /// Raised when lemonfiber has nowhere to write the stack.
    STACK_NOT_SET_UP = "STACK-4" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "There is nowhere to write the stack, because no location has been chosen.",
        remedy: "Run lemonfiber setup.",
    }
    /// Raised when the stack could not be written to disk.
    STACK_NOT_WRITTEN = "STACK-5" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "The stack could not be written to disk, so nothing can start. Usually a \
            permission problem or a full disk.",
        remedy: "Check the location is writable and has space.",
    }
    /// Raised when a manifest parses and breaks the contract.
    STACK_INVALID = "STACK-6" {
        severity: Error,
        status: 500,
        leaves: Validation,
        since: "0.1.0",
        meaning: "The manifest parses and contradicts itself: it says things about itself that \
            cannot all be true.",
        remedy: "Fix the faults listed under the message. All of them were found in one pass.",
    }
    /// Raised when a manifest is not TOML at all.
    STACK_MALFORMED = "STACK-7" {
        severity: Error,
        status: 500,
        leaves: Validation,
        since: "0.14.0",
        meaning: "The file is not written in the format a stack.toml uses, so nothing in it \
            has been read. The message names the line the reader stopped on.",
        remedy: "Fix the file at the line the message names.",
    }
    /// Raised when a manifest declares names this build does not know.
    STACK_UNRECOGNISED = "STACK-8" {
        severity: Error,
        status: 500,
        leaves: Validation,
        since: "0.14.0",
        meaning: "The file is well formed and declares things in words this version of \
            lemonfiber has no meaning for — usually a stack from a newer lemonfiber, or one with \
            something of its own added. Starting it would quietly leave out whatever was named.",
        remedy: "Update lemonfiber, or change the named declarations to ones it knows. Every one \
            of them is listed, found in a single pass.",
    }
    /// Raised when a stack names a newer lemonfiber than the one running.
    STACK_NEEDS_NEWER = "STACK-9" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The stack names the oldest lemonfiber it works with, and the one running is \
            older. It relies on something this version cannot do, so nothing in it is started \
            rather than started in part. The message names both versions.",
        remedy: "Update lemonfiber to the version named, with lemonfiber update self. Or point \
            at a stack this version runs, with lemonfiber --stack-dir <path>.",
    }
    /// Raised when a manifest's files are not laid out as the contract says.
    STACK_UNASSEMBLED = "STACK-10" {
        severity: Error,
        status: 500,
        leaves: Validation,
        since: "0.18.0",
        meaning: "The stack's manifest files are not laid out as a stack is: a stack.toml \
            whose include list names one file per service under services/, each describing \
            one service. Which services the stack holds cannot be told, so nothing in it was \
            started. A stack that keeps every service in stack.toml itself is refused this \
            way.",
        remedy: "Fix the files named under the message. All of them were found in one pass.",
    }
    /// Raised when a change would write over a generated project the operator has edited.
    PROJECT_EDITED = "STACK-11" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "The project lemonfiber generates no longer holds what lemonfiber last wrote \
            there: somebody edited it. A change that has to write the project again would \
            discard that edit, so nothing was changed. The message names the file.",
        remedy: "Move the edit into overlay.yml in lemonfiber's configuration directory, which \
            is merged over the project and kept, then run lemonfiber reset to put the project \
            back and make the change again.",
    }
    /// Raised when a service's state kept in a volume could not be copied into its mount
    /// tree before the service first starts from there.
    VOLUME_NOT_COPIED = "STACK-12" {
        severity: Error,
        status: 500,
        since: "0.18.0",
        meaning: "A service kept its state in a volume the container engine holds, and from \
            now on it keeps it in its own directory. The state could not be copied there, so \
            nothing was started: starting it now would start it on nothing and its old state \
            would never be read again. The volume is as it was.",
        remedy: "Check the container engine is running and the project directory can be \
            written, then start again. The message names the volume and the directory.",
    }
    /// Raised where the offer a reset answers is not the one a reading of it makes now.
    RESET_MOVED = "STACK-13" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That agreement was given for a different reading of what a reset would \
            revert: since it was read, an edit or a connection changed. Agreeing to it now \
            would be agreeing to lose something nobody read, so nothing was reverted; the \
            message names what moved.",
        remedy: "Read what a reset would revert again, and answer the name it prints.",
    }
    /// Raised where the certificate a service declares could not be made or read before it
    /// starts.
    CERTIFICATE_NOT_MADE = "STACK-14" {
        severity: Error,
        status: 500,
        since: "0.18.0",
        meaning: "A service declares a certificate lemonfiber makes for it before it starts, \
            and that certificate could not be made, or what is kept in its place is not a \
            certificate and its key, or only one of the two is there. Starting the service \
            would start it without the certificate it serves with, so nothing was started.",
        remedy: "The message names the service and the file. Where only one of the two is \
            there, or either is not what it should be, move both aside and start again, and \
            lemonfiber makes a new pair.",
    }
    /// Raised where a start names a fenced service no fence can be written for.
    SERVICE_UNFENCED = "STACK-15" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "The service declares itself fenced: it runs inside a fence lemonfiber \
            writes beside it, which refuses every connection it opens to a private address or \
            to the host. No fence can be written for it here, so it was not started: starting \
            it unfenced would let whatever page it fetches reach every private address.",
        remedy: "The message names why. Where the engine's host was not asked its addresses, \
            check the container engine is running and the fence's image can be fetched, then \
            start again.",
    }
}
