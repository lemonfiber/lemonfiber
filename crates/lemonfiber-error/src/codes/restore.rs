codes! {
    /// Raised when a backup archive cannot be read to decide a restore.
    CORRUPT = "RESTORE-1" {
        severity: Error,
        status: 500,
        since: "0.3.0",
        meaning: "The archive could not be read. Most often it is truncated, or is not a \
            lemonfiber backup. Nothing was touched.",
        remedy: "Check the archive, or restore from a different backup.",
    }
    /// Raised when an archive was written by a newer lemonfiber than this one.
    TOO_NEW = "RESTORE-2" {
        severity: Error,
        status: 500,
        since: "0.3.0",
        meaning: "The archive was written by a newer lemonfiber than this one, and may hold \
            configuration this version would not restore correctly. Nothing was touched.",
        remedy: "Update lemonfiber to at least the version that made the backup, then restore.",
    }
    /// Raised when an archive's format cannot be restored by this build.
    INCOMPATIBLE = "RESTORE-3" {
        severity: Error,
        status: 500,
        since: "0.3.0",
        meaning: "The archive's format cannot be restored by this build. Restoring it could \
            leave the configuration in a state neither version expects. Nothing was touched.",
        remedy: "Restore it with the lemonfiber version that made it.",
    }
    /// Raised when an archive holds a member that would be written outside its area.
    UNSAFE = "RESTORE-4" {
        severity: Critical,
        status: 500,
        since: "0.3.0",
        meaning: "The archive holds an entry naming a path that leaves the directory it belongs \
            in, which a genuine lemonfiber backup never does. It is refused and nothing was \
            touched.",
        remedy: "Do not restore this archive. It is corrupt, or was tampered with.",
    }
    /// Raised when a restore onto a different data root awaits the operator's consent.
    NEEDS_REPOINT = "RESTORE-5" {
        severity: Warning,
        status: 500,
        since: "0.3.0",
        meaning: "The archive was taken against a different data root. Restoring it unchanged \
            would keep a setting naming a location that is not on this machine.",
        remedy: "Re-run the restore with `--repoint` to accept moving it to this machine's data \
            root.",
    }
    /// Raised when an archive could not be unpacked.
    NOT_RESTORED = "RESTORE-6" {
        severity: Error,
        status: 500,
        since: "0.3.0",
        meaning: "The archive could not be unpacked, and the restore stopped part-way through \
            writing the configuration back.",
        remedy: "Check the configuration location is writable and restore again. A seed \
            afterwards reconciles anything left half-written.",
    }
    /// Raised when a restore could not be shown that nothing is writing to a database.
    STILL_RUNNING = "RESTORE-7" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "The stack was not confirmed stopped, so nothing was touched. A restore writes \
            over the service databases, and an engine that will not say whether the services are \
            running is refused as firmly as one that says they are.",
        remedy: "Stop the stack, check the container engine is answering, then restore again.",
    }
    /// Raised when a name does not name one of the backups this machine kept.
    NOT_KEPT_HERE = "RESTORE-8" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "The name you gave is not one of the backups kept here. A restore by name \
            reaches one of the archives this machine took, which are files in a single \
            directory; a name carrying a path, or climbing out of that directory, is refused \
            rather than followed. Nothing was touched.",
        remedy: "Ask for one of the backups by the name it was written under.",
    }
    /// Raised when this run has nowhere it knows to look for an archive.
    NOWHERE_KEPT = "RESTORE-9" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "This run has nowhere it knows to look for a backup. Archives are kept in \
            lemonfiber's own directory, and this machine would not say where that is. Nothing \
            was touched.",
        remedy: "Set a home directory for this user, then restore again.",
    }
    /// Raised when the restored settings could not be pointed at this machine's data root.
    NOT_REPOINTED = "RESTORE-10" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "The archive was unpacked, and the data root it recorded could not be changed \
            to this machine's — so the settings that landed name a library that is not here.",
        remedy: "Set the data root by hand, then run `lemonfiber seed`.",
    }
    /// Raised when consent was given for a listing that no longer stands.
    MOVED_ON = "RESTORE-11" {
        severity: Warning,
        status: 400,
        since: "0.9.0",
        meaning: "What you agreed to is not what this backup would do now. A fresh look at the \
            archive lists something else, so something changed between reading the listing and \
            answering it. Nothing was overwritten.",
        remedy: "Ask what the backup holds again, and read what it says now.",
    }
    /// Raised when the archive holds trees lemonfiber does not manage.
    NOT_OURS = "RESTORE-12" {
        severity: Error,
        status: 500,
        since: "0.14.0",
        meaning: "The archive holds a setup lemonfiber does not manage. It was captured before \
            lemonfiber took over, so putting it back means writing into directories that are not \
            lemonfiber's to write to. Nothing was touched.",
        remedy: "Unpack it yourself with `tar -xzf`, into the paths the message names.",
    }
}
