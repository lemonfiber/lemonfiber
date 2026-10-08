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
        remedy: "Run `lemonfiber setup`.",
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
        remedy: "Run the newer lemonfiber, or take this one forward with `lemonfiber update \
            self`. The message names both versions.",
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
    /// Raised when the Usenet indexer aggregator answers a read of its whole
    /// configuration to a caller presenting nothing, or will not say whether it does.
    AGGREGATOR_EXPOSED = "CONFIG-7" {
        severity: Warning,
        status: 500,
        since: "0.17.0",
        meaning: "The Usenet indexer aggregator answers its whole configuration — the indexer \
            accounts it holds, and their keys among it — to anything that can reach it, or could \
            not be asked whether it does. Nothing has been changed — this is the doctor \
            reporting what it found.",
        remedy: "Run `lemonfiber seed` to turn its authentication on, or `lemonfiber reset` \
            where you turned it off yourself. Where it could not be asked, check it is up and \
            has finished starting, then run the diagnosis again.",
    }
}
