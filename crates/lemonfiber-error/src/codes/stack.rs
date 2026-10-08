codes! {
    /// Raised when a stack directory holds no readable manifest.
    STACK_UNREADABLE = "STACK-1" {
        severity: Error,
        status: 500,
        leaves: Validation,
        since: "0.1.0",
        meaning: "No readable manifest was found where a stack was expected. A stack directory \
            holds a `stack.toml` beside its compose files.",
        remedy: "Point at a directory containing `stack.toml`, with `lemonfiber --stack-dir \
            <path>`.",
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
    /// Raised when the embedded stack is not intact.
    STACK_NOT_EMBEDDED = "STACK-3" {
        severity: Critical,
        status: 500,
        since: "0.1.0",
        meaning: "This build of lemonfiber is not intact: the stack that ships inside the binary \
            is missing. The build is supposed to make this impossible.",
        remedy: "Nothing is known to fix this. Send a [support \
            bundle](https://docs.lemonfiber.app/fixing/the-support-bundle/).",
    }
    /// Raised when lemonfiber has nowhere to write the stack.
    STACK_NOT_SET_UP = "STACK-4" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "There is nowhere to write the stack, because no location has been chosen.",
        remedy: "Run `lemonfiber setup`.",
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
        meaning: "The file is not written in the format a `stack.toml` uses, so nothing in it \
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
        remedy: "Update lemonfiber to the version named, with `lemonfiber update self`. Or point \
            at a stack this version runs, with `lemonfiber --stack-dir <path>`.",
    }
    /// Raised when a manifest's files are not laid out as the contract says.
    STACK_UNASSEMBLED = "STACK-10" {
        severity: Error,
        status: 500,
        leaves: Validation,
        since: "0.18.0",
        meaning: "The stack's manifest files are not laid out as a stack is: a `stack.toml` \
            whose `include` list names one file per service under `services/`, each describing \
            one service. Which services the stack holds cannot be told, so nothing in it was \
            started. A stack that keeps every service in `stack.toml` itself is refused this \
            way.",
        remedy: "Fix the files named under the message. All of them were found in one pass.",
    }
}
