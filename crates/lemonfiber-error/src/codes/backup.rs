codes! {
    /// Raised when there is not enough room to write a backup.
    NO_ROOM = "BACKUP-1" {
        severity: Error,
        status: 500,
        since: "0.3.0",
        meaning: "There is not enough room. A backup is written to the same disk it protects, \
            and this one would not fit with room to spare. Nothing was captured.",
        remedy: "Free space on the backups volume, or lower how many backups are kept.",
    }
    /// Raised when the backup archive could not be written.
    NOT_WRITTEN = "BACKUP-2" {
        severity: Error,
        status: 500,
        since: "0.3.0",
        meaning: "The archive could not be written and the capture stopped part-way. A \
            configuration backup is what makes the rest recoverable, so it is worth fixing \
            before a risky change.",
        remedy: "Check the backups volume is writable, then try again.",
    }
    /// Raised when the room for a backup could not be measured.
    NOT_MEASURED = "BACKUP-3" {
        severity: Error,
        status: 500,
        since: "0.3.0",
        meaning: "The room for a backup could not be measured. lemonfiber checks a backup will \
            fit before starting one. Nothing was captured.",
        remedy: "Check the backups location is reachable, then try again.",
    }
    /// Raised when a capture could not be shown that nothing is writing to a database.
    STILL_RUNNING = "BACKUP-4" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "The stack was not confirmed stopped, so nothing was captured. Copying a \
            database a service is writing to is the corruption a backup exists to prevent, so a \
            running stack — and an engine that will not say whether it is running — are refused \
            alike.",
        remedy: "Stop the stack, check the container engine is answering, then capture again.",
    }
    /// Raised when this run has nowhere it knows to keep an archive.
    NOWHERE_TO_KEEP = "BACKUP-5" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "This run has nowhere it knows to keep an archive. A backup goes into \
            lemonfiber's own directory, and this machine would not say where that is. Nothing \
            was written.",
        remedy: "Set a home directory for this user, then capture again.",
    }
    /// Raised when this run has nowhere it knows to look for archives.
    NOWHERE_KEPT = "BACKUP-6" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "This run has nowhere it knows to look for backups. They are kept in \
            lemonfiber's own directory, and this machine would not say where that is, so there \
            is nowhere to read a list of them from.",
        remedy: "Set a home directory for this user, then ask again.",
    }
    /// Raised when the directory the archives are kept in could not be read.
    NOT_LISTED = "BACKUP-7" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "The backups kept here could not be listed. The directory lemonfiber keeps them \
            in would not be read, so what is in it is not known. Nothing was touched.",
        remedy: "Check the backups directory is readable, then ask again.",
    }
}
