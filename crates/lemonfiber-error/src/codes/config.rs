codes! {
    /// Raised when configuration exists and cannot be read.
    CONFIG_UNREADABLE = "CONFIG-1" {
        severity: Error,
        status: 500,
        leaves: Validation,
        since: "0.1.0",
        meaning: "Your settings exist and could not be read. Nothing has been changed — \
            lemonfiber will not guess at settings it cannot read.",
        remedy: "Check the file is readable. The message names its path.",
    }
    /// Raised when configuration cannot be written.
    CONFIG_NOT_WRITTEN = "CONFIG-2" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "Your settings could not be saved. The change was not made and your existing \
            settings are untouched.",
        remedy: "Check the location is writable and has space.",
    }
    /// Raised when there is nowhere to keep configuration.
    CONFIG_NOWHERE = "CONFIG-3" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "There is nowhere to keep settings, because setup has not chosen a location \
            yet.",
        remedy: "Run lemonfiber setup.",
    }
    /// Raised when a file holding a credential can be read by more than its owner.
    CREDENTIALS_EXPOSED = "CONFIG-4" {
        severity: Warning,
        status: 500,
        since: "0.13.0",
        meaning: "A file holding a credential can be read by somebody other than its owner. \
            Nothing has been changed — this is the doctor reporting what it found.",
        remedy: "Take the permissions back to their owner. The message names each file.",
    }
    /// Raised when configuration was written by a newer lemonfiber.
    CONFIG_TOO_NEW = "CONFIG-5" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "Your settings were written by a newer lemonfiber than the one running, so this \
            build will not write over them. Nothing has been changed — an older build writing \
            over a newer one's settings leaves a file neither version can make sense of, and no \
            way back to the one you had.",
        remedy: "Run the newer lemonfiber, or take this one forward with lemonfiber update \
            self. The message names both versions.",
    }
    /// Raised when a setting's key or value spans more than one line.
    CONFIG_SPANS_LINES = "CONFIG-6" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "A value could not be saved because it has a line break in it. Nothing has been \
            changed — the settings file holds one setting per line, so the rest of the value \
            would have been read as settings you never chose.",
        remedy: "Check where the value came from, and set it to a single line. The message names \
            the settings file.",
    }
    /// Raised where the operator's overlay changes services, merged over the project
    /// lemonfiber writes and held to nothing it vets.
    OVERLAY_UNVETTED = "CONFIG-8" {
        severity: Warning,
        status: 500,
        since: "0.18.0",
        meaning: "Your overlay is merged over the project lemonfiber writes, last, and changes \
            the services it names. Nothing in it was held to what lemonfiber checks a \
            service against, so what those services can reach and do is yours to vouch for. \
            Nothing has been changed — this is the doctor reporting what it found.",
        remedy: "Keep the overlay to what the services need, or move a change into the stack's \
            own settings where one says it. The message names the overlay and each service \
            it changes.",
    }
    /// Raised where a path is still recorded under the retired overlay setting, which is no
    /// longer read.
    OVERLAY_RETIRED = "CONFIG-9" {
        severity: Warning,
        status: 500,
        since: "0.18.0",
        meaning: "A Compose file is still named under LEMONFIBER_OVERLAY, which lemonfiber no \
            longer reads: the overlay is the one file overlay.yml in its configuration \
            directory. Whatever the named file changed is not applied.",
        remedy: "Move the named file to overlay.yml in lemonfiber's configuration directory, \
            then remove the LEMONFIBER_OVERLAY line from the settings file. The message names \
            both paths.",
    }
    /// Raised where the project directory still holds a Compose file of the stack's own
    /// that lemonfiber no longer reads.
    COMPOSE_RETIRED = "CONFIG-10" {
        severity: Warning,
        status: 500,
        since: "0.18.0",
        meaning: "lemonfiber runs the one project it generates from the stack's manifest and \
            reads none of the stack's own Compose files. A file named here is one somebody \
            changed, or one a stack of its own still carries, and whatever it says is not \
            applied.",
        remedy: "Move whatever the file changed into overlay.yml in lemonfiber's configuration \
            directory, then delete the file. The message names each one.",
    }
    /// Raised where a part that belongs to whoever claims it first is held by nobody, is
    /// held by somebody other than lemonfiber, or could not be asked who holds it.
    PART_UNCLAIMED = "CONFIG-11" {
        severity: Warning,
        status: 500,
        since: "0.18.0",
        meaning: "A service that belongs to whoever claims it first is not held with the \
            credential lemonfiber minted for it: nobody holds it, so whoever reaches it first can \
            take it; somebody else holds it; or it could not be asked. Nothing has been changed \
            — this is the doctor reporting what it found.",
        remedy: "Run lemonfiber seed to claim a service nobody holds, or lemonfiber reset where \
            lemonfiber claimed it before. Where somebody else holds it and that was not you, \
            reset its configuration and run lemonfiber seed. The message names the service and \
            the secret lemonfiber holds for it.",
    }
}
