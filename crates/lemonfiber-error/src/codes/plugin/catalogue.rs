//! The `PLUGIN` codes of a plugin looked up in the catalogue.

codes! {
    /// Raised when a plugin is installed by name and asking the catalogue is switched
    /// off.
    CATALOGUE_OFF = "PLUGIN-18" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The plugin was named for the catalogue to look up, and asking the catalogue is \
            switched off on this machine. Nothing was fetched or installed.",
        remedy: "Install it from a git source or a directory you name instead, or allow it with \
            lemonfiber config set LEMONFIBER_REACH_CATALOGUE on.",
        household: fault,
    }
    /// Raised when the catalogue's index or its signature could not be fetched.
    CATALOGUE_UNREACHABLE = "PLUGIN-19" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The catalogue's index, or its signature, could not be fetched. Nothing was \
            installed, and what is installed already is unaffected; installing from a git source \
            or a directory does not ask the catalogue.",
        remedy: "Check this machine can reach github.com and install it again, or install it \
            from its git source.",
        household: fault,
    }
    /// Raised when the catalogue's index has no signature, one that does not verify,
    /// or none this build carries a key to check.
    SIGNATURE_UNVERIFIED = "PLUGIN-20" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The catalogue's index is not signed by the key this build carries: it has no \
            signature, one that does not verify, or none this build has a key to check. Nothing \
            was resolved through it, because an index nobody can show the catalogue signed is no \
            evidence that anybody reviewed anything.",
        remedy: "Install it from its git source, where it is installed as unreviewed, or install \
            a lemonfiber that carries the catalogue's key.",
        household: fault,
    }
    /// Raised when the catalogue's index verified and is not one this build reads.
    CATALOGUE_UNREADABLE = "PLUGIN-21" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The catalogue's index is signed and this build cannot read it. Nothing was \
            resolved through it, and nothing was installed.",
        remedy: "Update lemonfiber, whose newer releases read newer indexes, or install it from \
            its git source.",
        household: fault,
    }
    /// Raised when the catalogue holds no plugin by the name given.
    NOT_CATALOGUED = "PLUGIN-22" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "The catalogue holds no plugin by that name. Nothing was fetched or installed.",
        remedy: "Check the name, or write ./<name> to install the directory of that name.",
        household: fault,
    }
    /// Raised when what the catalogue's origin served is not what the catalogue
    /// reviewed.
    NOT_AS_REVIEWED = "PLUGIN-23" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "What the catalogue's origin holds at the commit it named is not what the \
            catalogue reviewed: the manifest there is not the one whose digest it signed. \
            Nothing was installed.",
        remedy: "Install it from its git source if you mean to run it unreviewed, and tell the \
            catalogue's maintainers.",
        household: fault,
    }
    /// Raised when the catalogue's index verifies and is older than the newest one
    /// this machine has verified.
    CATALOGUE_REPLACED = "PLUGIN-29" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The catalogue's index verifies and is older than one this machine has already \
            verified. A release the catalogue has replaced is still signed, and what it reviewed \
            may since have been withdrawn, so nothing was resolved through it and nothing was \
            installed.",
        remedy: "Try again later, when the catalogue serves its newest release, or install it \
            from its git source.",
        household: fault,
    }
    /// Raised when the record of the newest catalogue index this machine verified
    /// cannot be read or written.
    NEWEST_UNKEPT = "PLUGIN-30" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "This machine cannot tell whether the catalogue's index is its newest: the \
            record of the newest index it verified cannot be read or written. Without it, a \
            release the catalogue has replaced would verify as though it were current, so \
            nothing was resolved through it and nothing was installed.",
        remedy: "Check catalogue.json in the configuration directory, or install it from its \
            git source.",
        household: fault,
    }
}
