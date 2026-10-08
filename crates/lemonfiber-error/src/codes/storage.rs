codes! {
    /// Raised when the data root cannot hardlink, so imports must copy.
    COPY_ONLY = "STORAGE-1" {
        severity: Warning,
        status: 500,
        since: "0.2.0",
        meaning: "The data location cannot hardlink, so imports copy: each takes minutes rather \
            than being instant, uses twice the disk while it runs, and torrents cannot seed from \
            the library copy. Where the filesystem type explains it — exFAT, FAT, SMB, NFS, the \
            WSL2 boundary — the message names that.",
        remedy: "Choose a location that hardlinks, or continue in copy mode. The services are \
            configured to copy, so imports still work.",
    }
    /// Raised when the data root exists but cannot be written to.
    ROOT_UNWRITABLE = "STORAGE-2" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "The data location exists and cannot be written to. The services have to own \
            what they import, so every import fails far from where the cause shows.",
        remedy: "Give the account that runs the services write access to the data location.",
    }
    /// Raised when the data root is not there to test.
    ROOT_ABSENT = "STORAGE-3" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "The data location could not be reached. A stack that wrote into a missing \
            mount point would build a phantom library on the system disk.",
        remedy: "Check the location exists and any drive holding it is connected.",
    }
    /// Raised when the volume holding the data root is nearly full.
    SPACE_LOW = "STORAGE-4" {
        severity: Warning,
        status: 500,
        since: "0.2.0",
        meaning: "Free space is low, or is projected to run out against what is already queued. \
            A disk that fills partway through an import leaves half a file behind and stalls the \
            queue.",
        remedy: "Free space on the data location, thin the download queue, or move it to a \
            larger volume.",
    }
    /// Raised when the data root used to hardlink and no longer does.
    DEGRADED = "STORAGE-5" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "The data location used to hardlink and no longer does — usually a drive that \
            came back mounted with different options. Every import since has been copying.",
        remedy: "Check how the data location is mounted, and remount it as it was. A network \
            share remounted without the right options is the common cause.",
    }
    /// Raised when the operator owns the data root but the services cannot write it.
    SERVICE_DENIED = "STORAGE-6" {
        severity: Error,
        status: 500,
        since: "0.2.0",
        meaning: "You own the data location and the containers cannot write it: they run as one \
            user and group, and the directory's ownership and mode do not allow them. Imports \
            fail inside the services.",
        remedy: "Give the service user ownership of the data location, or write access to it.",
    }
    /// Raised where a service would see more than one mount beneath the data location,
    /// so anything imported between them is copied rather than linked.
    SPLIT_MOUNTS = "STORAGE-7" {
        severity: Warning,
        status: 500,
        since: "0.15.0",
        meaning: "A service sees more than one mount beneath the data location, so each is its \
            own filesystem inside the container and a file moved between them is copied rather \
            than linked. Nothing is broken — imports take longer and use twice the space.",
        remedy: "Mount the data location once and keep the downloads and the library as \
            directories beneath it. Where the layout is deliberate, `lemonfiber doctor --accept \
            storage.single-mount` settles it.",
    }
    /// Raised where a location that stopped hardlinking leaves the stack copying in a
    /// mode it was not set up for.
    COPYING_SINCE_DEGRADED = "STORAGE-8" {
        severity: Warning,
        status: 500,
        since: "0.18.0",
        meaning: "The stack is copying imports where it was set up to hardlink them, because the \
            data location stopped hardlinking under it. Imports still work, using twice the \
            disk.",
        remedy: "Restore the location's hardlink support, then run the storage check again.",
    }
}
