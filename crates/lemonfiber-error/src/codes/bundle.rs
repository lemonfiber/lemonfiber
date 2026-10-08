codes! {
    /// Raised when a bundle would still hold something that reads as a credential.
    BUNDLE_LEAK = "BUNDLE-1" {
        severity: Critical,
        status: 500,
        since: "0.7.0",
        meaning: "The finished bundle still held something that reads as a credential, so \
            nothing was written. A bundle is a thing people post in public, so anything in one \
            that still looks like a key is treated as one — even where it turns out not to be.",
        remedy: "Report which file the message names, so the value it holds can be added to what \
            a bundle knows how to replace.",
    }
    /// Raised when there is not enough room to write a bundle.
    BUNDLE_NO_ROOM = "BUNDLE-2" {
        severity: Error,
        status: 500,
        since: "0.7.0",
        meaning: "There is not enough room to write the bundle where it was to be written, with \
            space left over for the machine to keep working in.",
        remedy: "Free some space, or write the bundle somewhere with more room using `--out`.",
    }
    /// Raised when the archive could not be written.
    BUNDLE_UNWRITTEN = "BUNDLE-3" {
        severity: Error,
        status: 500,
        since: "0.7.0",
        meaning: "The archive could not be written. Nothing was left behind: a bundle is written \
            whole or not at all, so there is no half-file to mistake for one.",
        remedy: "Check the path is writable, then ask for the bundle again.",
    }
    /// Raised when a setting was asked to be shown as it is without that being confirmed.
    BUNDLE_UNCONFIRMED = "BUNDLE-4" {
        severity: Error,
        status: 500,
        since: "0.7.0",
        meaning: "A setting was asked to be shown as it is, without that being confirmed on the \
            same run. Showing one puts the real value in a file people post in public, so it \
            takes saying twice.",
        remedy: "Run it again with `--confirm` if you meant it.",
    }
    /// Raised when the machine can offer no randomness to derive stand-ins from.
    BUNDLE_NO_MARKS = "BUNDLE-5" {
        severity: Error,
        status: 500,
        since: "0.7.0",
        meaning: "The machine could offer no randomness to derive the stand-ins from, so nothing \
            was written. A stand-in anyone can reproduce is a way back to the value it stands \
            for.",
        remedy: "Report this. A machine that cannot produce random bytes is a fault in its own \
            right.",
    }
    /// Raised when this run has nowhere it knows to keep a bundle.
    NOWHERE_TO_KEEP = "BUNDLE-6" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "This run can write no archive at all: it holds neither a directory of its own \
            to keep one in nor anything to pack one with, which is what a machine that will not \
            say where its own files go leaves behind. A bundle asked for at a named path is \
            refused here too, and nothing was written.",
        remedy: "Set a home directory for this user, then ask for the bundle again.",
    }
    /// Raised when this run has nowhere it knows to look for a bundle it kept.
    NOWHERE_HELD = "BUNDLE-7" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "This run has nowhere it knows to look for a bundle. Bundles asked for by name \
            are kept with lemonfiber's own files, and this machine would not say where those \
            are, so there is nowhere to read one back from.",
        remedy: "Set a home directory for this user, then ask for the bundle again.",
    }
    /// Raised when a name does not name one of the bundles this run kept.
    NOT_HELD = "BUNDLE-8" {
        severity: Error,
        status: 404,
        since: "0.9.0",
        meaning: "The name you gave is not one of the bundles kept here. A bundle asked for by \
            name is one of the files this run wrote into lemonfiber's own directory; a name \
            holding a path, or climbing out of that directory, is refused rather than followed.",
        remedy: "Ask for a bundle by the name the run that produced it reported.",
    }
}
