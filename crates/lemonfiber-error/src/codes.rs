//! Every problem code lemonfiber can raise, declared in one place.
//!
//! A code is a family and a number. It is never renumbered and never recycled, so an
//! operator who searches for one finds the same answer a year later. The families are
//! modules here, and every crate names a code through them, so a code exists exactly
//! where this list says it does and the reference in `reference/error-codes.md` is
//! written from [`EVERY`].
//!
//! `WIRE` and `WIRING` are both the wiring domain's: the two prefixes were published
//! apart, and a published code keeps its spelling.
//!
//! What a run that ends on a code leaves with is declared beside it, with `=>`, where
//! it is not a general failure; [`leaves`] reads it.

use crate::Code;

/// What a run that ends on a problem leaves with, for a script that reads nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaves {
    /// A general failure.
    Failure,
    /// Something outside lemonfiber has to be fixed before it can act.
    Preflight,
    /// Something the operator wrote was refused.
    Validation,
    /// Started, and a service never became usable.
    NeverSettled,
}

/// Declares each family as a module of codes, [`EVERY`] and [`leaves`] from one list.
macro_rules! codes {
    ($(
        $(#[doc = $family_doc:literal])*
        $family:ident {
            $($(#[doc = $doc:literal])* $name:ident = $id:literal $(=> $leaves:ident)?,)*
        }
    )*) => {
        $(
            $(#[doc = $family_doc])*
            pub mod $family {
                use crate::Code;
                $($(#[doc = $doc])* pub const $name: Code = Code::declared($id);)*
            }
        )*

        /// Every code there is, family by family, in number order.
        pub const EVERY: &[Code] = &[$($($family::$name,)*)*];

        /// What a run that ends on `code` leaves with.
        #[must_use]
        pub fn leaves(code: Code) -> Leaves {
            match code.as_str() {
                $($($($id => Leaves::$leaves,)?)*)*
                _ => Leaves::Failure,
            }
        }
    };
}

codes! {
    /// The `ACK` codes.
    ack {
        /// Raised when an answer names something nothing is warning about.
        NOT_WARNED = "ACK-1",
    }
    /// The `ADMIT` codes.
    admit {
        /// Raised when a password is too short to stand in front of this.
        TOO_SHORT = "ADMIT-1",
        /// Raised when this machine will not supply the salt a record is made with.
        NO_SALT = "ADMIT-2",
        /// Raised when the two answers were not the same word.
        MISTYPED = "ADMIT-3",
    }
    /// The `BACKUP` codes.
    backup {
        /// Raised when there is not enough room to write a backup.
        NO_ROOM = "BACKUP-1",
        /// Raised when the backup archive could not be written.
        NOT_WRITTEN = "BACKUP-2",
        /// Raised when the room for a backup could not be measured.
        NOT_MEASURED = "BACKUP-3",
        /// Raised when a capture could not be shown that nothing is writing to a database.
        STILL_RUNNING = "BACKUP-4",
        /// Raised when this run has nowhere it knows to keep an archive.
        NOWHERE_TO_KEEP = "BACKUP-5",
        /// Raised when this run has nowhere it knows to look for archives.
        NOWHERE_KEPT = "BACKUP-6",
        /// Raised when the directory the archives are kept in could not be read.
        NOT_LISTED = "BACKUP-7",
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
    /// The `BUNDLE` codes.
    bundle {
        /// Raised when a bundle would still hold something that reads as a credential.
        BUNDLE_LEAK = "BUNDLE-1",
        /// Raised when there is not enough room to write a bundle.
        BUNDLE_NO_ROOM = "BUNDLE-2",
        /// Raised when the archive could not be written.
        BUNDLE_UNWRITTEN = "BUNDLE-3",
        /// Raised when a setting was asked to be shown as it is without that being confirmed.
        BUNDLE_UNCONFIRMED = "BUNDLE-4",
        /// Raised when the machine can offer no randomness to derive stand-ins from.
        BUNDLE_NO_MARKS = "BUNDLE-5",
        /// Raised when this run has nowhere it knows to keep a bundle.
        NOWHERE_TO_KEEP = "BUNDLE-6",
        /// Raised when this run has nowhere it knows to look for a bundle it kept.
        NOWHERE_HELD = "BUNDLE-7",
        /// Raised when a name does not name one of the bundles this run kept.
        NOT_HELD = "BUNDLE-8",
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
    /// The `CRED` codes.
    cred {
        /// Raised when a service answers and refuses the credential it generated itself.
        CREDENTIAL_REJECTED = "CRED-1",
        /// Raised when the indexer answers and refuses the key it was given.
        INDEXER_REJECTED = "CRED-2",
        /// Raised when the indexer authenticates the key but cannot serve it right now.
        INDEXER_LIMITED = "CRED-3",
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
    /// The `GONE` codes.
    gone {
        /// Raised when the tier that takes the library was confirmed without its own
        /// agreement.
        NEEDS_AGREEING = "GONE-1",
        /// Raised when an agreement names a reading of this machine that is not the one
        /// standing now.
        ANOTHER_READING = "GONE-2",
        /// Raised when the backup a destructive removal takes first could not be taken.
        NOT_BACKED_UP = "GONE-3",
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
    /// The `INVITE` codes.
    invite {
        /// Said where the stack holds no media server: there is nothing to make an account on.
        NO_MEDIA_SERVER = "INVITE-1",
        /// Said where the admin credential was never recorded: nothing can be asked of the server.
        NO_CREDENTIAL = "INVITE-2",
        /// Said where this machine has no address the household could arrive at.
        ///
        /// An invitation is an address somebody else types. Sending one built from a default
        /// would be sending a link that opens nothing, which is worse than saying there is
        /// none: the operator would learn it had failed from whoever they invited.
        NOWHERE_TO_SEND = "INVITE-3",
        /// Said where the invitation is for nobody: the name is blank, or only spaces.
        ///
        /// The media server refuses this too, in its own words, which are `400` and a link
        /// to the specification of that status. The operator asked for something reasonable
        /// and mistyped it, and is owed a sentence about the name rather than about HTTP.
        NOBODY_NAMED = "INVITE-4",
        /// Said where an expired invitation could not be dated again, so its window is not real.
        ///
        /// The account is untouched and still theirs — what failed is the write that says when it
        /// was offered. Reported rather than glossed over because the message the operator is
        /// about to send promises a window, and this one would be counted from whenever the
        /// invitation was first made, which has already passed.
        WOULD_NOT_RENEW = "INVITE-5",
        /// Said where the media server would not say what libraries it holds.
        ///
        /// Refused rather than read as no libraries at all: a name matched against an empty
        /// list is a name that could not be found, and the operator would be told their library
        /// does not exist when what happened is that nobody could ask.
        NO_LIBRARIES_READ = "INVITE-6",
        /// Said where no library goes by a name that was given.
        ///
        /// The ones there are, named: the fix is one word, and the words are already in hand.
        NO_SUCH_LIBRARY = "INVITE-7",
        /// Said where the account was made and what it may watch could not be written on it.
        ///
        /// Said as an account that exists and is open, because that is what is now true. An
        /// operator told only that something failed would not know whether to invite again or
        /// to go and narrow an account that is already there.
        WOULD_NOT_ALLOW = "INVITE-8",
    }
    /// The `KEPT` codes.
    kept {
        /// Raised when this run does not know where lemonfiber's own files go.
        NOWHERE_KNOWN = "KEPT-1",
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
    /// The `PLUGIN` codes.
    plugin {
        /// A check a plugin contributed did not hold.
        ///
        /// One code for all of them rather than one per plugin, because a code is a stable
        /// thing an operator searches for and a plugin's own name is not this build's to mint
        /// one from. Which check and which plugin is on the finding, where it can be read.
        CONTRIBUTED_FAILED = "PLUGIN-1",
        /// The source names no plugin this build can read.
        UNREADABLE = "PLUGIN-2",
        /// The manifest is read and this build refuses what it declares.
        REFUSED = "PLUGIN-3",
        /// The record of what is installed cannot be read.
        UNRECORDED = "PLUGIN-4",
        /// The plugin is installed already.
        ALREADY = "PLUGIN-5",
        /// There is no stack on this machine to put a plugin's container in.
        NOWHERE = "PLUGIN-6",
        /// A directory or a document the install decided on would not land.
        UNWRITABLE = "PLUGIN-7",
        /// The wiring went down and the record of what is installed did not.
        UNRECORDABLE = "PLUGIN-8",
        /// The plugin's own service would not start, so nothing about it could be proved.
        UNPROVED = "PLUGIN-9",
        /// Nothing by that name is installed on this machine.
        NOTHING_TO_REMOVE = "PLUGIN-10",
        /// Nothing by that id is installed, so there is no version to replace.
        NOTHING_TO_UPDATE = "PLUGIN-11",
        /// The version installed would not come off, so nothing else was touched.
        STUCK = "PLUGIN-12",
        /// Raised when a plugin's service would answer on a label another plugin's already does.
        ANSWERED = "PLUGIN-13",
    }
    /// The `PROC` codes.
    proc {
        /// Raised when the program a subprocess needs is missing.
        MISSING_PROGRAM = "PROC-1" => Preflight,
        /// Raised when a program exists but will not start.
        UNUSABLE_PROGRAM = "PROC-2",
    }
    /// The `PROVIDER` codes.
    provider {
        /// Raised when an account has nothing left to serve.
        PROVIDER_EMPTY = "PROVIDER-1",
        /// Raised when an account is running out, with time left to act.
        PROVIDER_LOW = "PROVIDER-2",
        /// Raised when the subscription behind an account ends soon.
        PROVIDER_ENDING = "PROVIDER-3",
        /// Raised when an indexer has been failing and its aggregator has rested it.
        INDEXER_RESTED = "PROVIDER-4",
        /// Raised when every indexer is failing at once.
        INDEXERS_ALL_FAILING = "PROVIDER-5",
        /// Raised when an account refuses the credential the client offers it.
        PROVIDER_REFUSED = "PROVIDER-6",
        /// Raised when an account has stopped answering the client entirely.
        PROVIDER_SILENT = "PROVIDER-7",
        /// Raised when the client is set to open more connections than an account allows.
        PROVIDER_CROWDED = "PROVIDER-8",
        /// Raised when an indexer has spent the allowance recorded against it.
        INDEXER_CAPPED = "PROVIDER-9",
    }
    /// The `QUAL` codes.
    qual {
        /// Raised when the free space holds too little content at the chosen quality.
        HEADROOM_LOW = "QUAL-1",
        /// Raised when releases exist but the profile — the chosen quality included — wants
        /// none of them.
        PRESET_UNMET = "QUAL-2",
        /// Raised when a clean search turns up nothing at all for wanted content.
        NONE_AVAILABLE = "QUAL-3",
    }
    /// The `QUOTA` codes.
    quota {
        /// Raised where the request service would not answer, so nothing was changed.
        UNREACHABLE = "QUOTA-1",
        /// Raised where a policy that lives inside a limit was chosen without one.
        NO_LIMIT = "QUOTA-2",
        /// Raised where no policy goes by the word that was given.
        NO_SUCH_POLICY = "QUOTA-3",
        /// Raised where the request named is not one that is waiting on anybody.
        NOT_WAITING = "QUOTA-4",
        /// Raised where a request was turned down and the reason said nothing.
        NO_REASON = "QUOTA-5",
        /// Raised where nobody in the household goes by the name that was given.
        NOBODY = "QUOTA-6",
        /// Raised where the request service holds no account for somebody who has one here.
        NEVER_HERE = "QUOTA-7",
        /// Raised where a run was asked to close what has waited too long and the household has
        /// never said how long that is.
        NOTHING_AGREED = "QUOTA-8",
        /// Raised where the period named would close a request nobody was ever reminded about.
        TOO_SOON = "QUOTA-9",
    }
    /// The `RATE` codes.
    rate {
        /// Raised when a limit is expressed as a share of a line nothing has measured.
        NOTHING_MEASURED = "RATE-1",
        /// Raised when a schedule is asked for and nothing says which zone the clients
        /// would read it in.
        NO_ZONE = "RATE-2",
        /// Raised when what was asked for could not be read as a limit, a window or a cap.
        UNREADABLE = "RATE-3",
        /// Raised when there is no download client to limit.
        NOTHING_TO_LIMIT = "RATE-4",
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
    /// The `REISSUE` codes.
    reissue {
        /// Said where the media server will not say who holds an account.
        UNREADABLE = "REISSUE-1",
        /// Said where nobody by that name is in the household.
        NOBODY_HERE = "REISSUE-2",
        /// Said where the account named administers the server.
        RUNS_THE_SERVER = "REISSUE-3",
        /// Said where the media server refused to make the account claimable again.
        WOULD_NOT_REISSUE = "REISSUE-4",
    }
    /// The `REMOVE` codes.
    remove {
        /// Said where the removal is for nobody: the name is blank, or only spaces.
        NOBODY_NAMED = "REMOVE-1",
        /// Said where the stack holds no media server: there is no account to remove.
        NO_MEDIA_SERVER = "REMOVE-2",
        /// Said where the media server will not say who it holds.
        UNREADABLE = "REMOVE-3",
        /// Said where nobody by that name is in the household.
        NOBODY_HERE = "REMOVE-4",
        /// Said where the account named administers the server.
        RUNS_THE_SERVER = "REMOVE-5",
        /// Said where the media server refused to remove the account.
        WOULD_NOT_REMOVE = "REMOVE-6",
    }
    /// The `REPAIR` codes.
    repair {
        /// Raised when consent was given for an offer that no longer stands.
        STALE = "REPAIR-1",
        /// Raised when a run cannot say where lemonfiber's own files are.
        NOWHERE_TO_LOOK = "REPAIR-2",
        /// Raised when a run that may not act was asked for the checks that disturb.
        OFFER_CANNOT_DISTURB = "REPAIR-3",
    }
    /// The `RESTORE` codes.
    restore {
        /// Raised when a backup archive cannot be read to decide a restore.
        CORRUPT = "RESTORE-1",
        /// Raised when an archive was written by a newer lemonfiber than this one.
        TOO_NEW = "RESTORE-2",
        /// Raised when an archive's format cannot be restored by this build.
        INCOMPATIBLE = "RESTORE-3",
        /// Raised when an archive holds a member that would be written outside its area.
        UNSAFE = "RESTORE-4",
        /// Raised when a restore onto a different data root awaits the operator's consent.
        NEEDS_REPOINT = "RESTORE-5",
        /// Raised when an archive could not be unpacked.
        NOT_RESTORED = "RESTORE-6",
        /// Raised when a restore could not be shown that nothing is writing to a database.
        STILL_RUNNING = "RESTORE-7",
        /// Raised when a name does not name one of the backups this machine kept.
        NOT_KEPT_HERE = "RESTORE-8",
        /// Raised when this run has nowhere it knows to look for an archive.
        NOWHERE_KEPT = "RESTORE-9",
        /// Raised when the restored settings could not be pointed at this machine's data root.
        NOT_REPOINTED = "RESTORE-10",
        /// Raised when consent was given for a listing that no longer stands.
        MOVED_ON = "RESTORE-11",
        /// Raised when the archive holds trees lemonfiber does not manage.
        NOT_OURS = "RESTORE-12",
    }
    /// The `SEED` codes.
    seed {
        /// Raised when a service is not answering yet.
        SERVICE_UNAVAILABLE = "SEED-1",
        /// Raised when a service rejects the credential lemonfiber holds.
        SERVICE_UNAUTHORISED = "SEED-2",
        /// Raised when a service answers with something unusable.
        SERVICE_REFUSED = "SEED-3",
        /// Raised when a service does not serve the API version this build speaks.
        SERVICE_UNSUPPORTED = "SEED-4",
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
    /// The `SPACE` codes.
    space {
        /// Raised when the volume is full and new acquisitions are therefore halted.
        HALTED = "SPACE-1",
        /// Raised when there is no data location to measure.
        NOWHERE_TO_MEASURE = "SPACE-2",
        /// Raised when the data location is there and could not be read.
        WALK_REFUSED = "SPACE-3",
        /// Raised when there is no torrent client here to be holding a completed download.
        NOTHING_TO_ASK = "SPACE-4",
        /// Raised when the client answers and is holding nothing of the name given.
        NOT_HELD = "SPACE-5",
        /// Raised when an agreement names an offer that is not the one standing now.
        ANOTHER_OFFER = "SPACE-6",
        /// Raised when the client could not be reached, or would not let a download go.
        STILL_HELD = "SPACE-7",
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
    /// The `STORAGE` codes.
    storage {
        /// Raised when the data root cannot hardlink, so imports must copy.
        COPY_ONLY = "STORAGE-1",
        /// Raised when the data root exists but cannot be written to.
        ROOT_UNWRITABLE = "STORAGE-2",
        /// Raised when the data root is not there to test.
        ROOT_ABSENT = "STORAGE-3",
        /// Raised when the volume holding the data root is nearly full.
        SPACE_LOW = "STORAGE-4",
        /// Raised when the data root used to hardlink and no longer does.
        DEGRADED = "STORAGE-5",
        /// Raised when the operator owns the data root but the services cannot write it.
        SERVICE_DENIED = "STORAGE-6",
        /// Raised where a service would see more than one mount beneath the data location,
        /// so anything imported between them is copied rather than linked.
        SPLIT_MOUNTS = "STORAGE-7",
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
    /// The `UNDO` codes.
    undo {
        /// Raised when no run carries the stamp a reversal was asked for.
        NO_SUCH_RUN = "UNDO-1",
        /// Raised when a stamp names more than one run, so which to put back is not settled.
        MORE_THAN_ONE_RUN = "UNDO-2",
        /// Raised when a run cannot be put back, carrying the reason it cannot.
        CANNOT_SUCCEED = "UNDO-3",
        /// Raised when a run cannot say where lemonfiber's own files are.
        NOWHERE_TO_LOOK = "UNDO-4",
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
    /// The `VPN` codes.
    vpn {
        /// Raised when the download client's egress does not match the tunnel.
        LEAKING = "VPN-1",
        /// Raised when the VPN container that should carry traffic is not running.
        VPN_CONTAINER_DOWN = "VPN-2",
        /// Raised when the client cannot reach the internet through the tunnel.
        CLIENT_ISOLATED = "VPN-3",
        /// Raised when port forwarding was asked for but the provider granted no port.
        NO_FORWARDED_PORT = "VPN-4",
        /// The code a stack whose traffic survives its tunnel earns.
        KILLSWITCH_LEAKS = "VPN-5",
        /// The code a stack whose tunnel could not be put back earns.
        TUNNEL_NOT_RESTORED = "VPN-6",
        /// Raised when the client is listening somewhere other than the forwarded port.
        PORT_MISMATCH = "VPN-7",
        /// Raised when torrents are configured with nothing containing them.
        NO_TUNNEL = "VPN-8",
    }
    /// The `WATCH` codes.
    watch {
        /// Raised when a watch is asked for but no data location is configured to watch.
        NOTHING_TO_WATCH = "WATCH-1",
        /// Raised when the data location is already gone when the watch is asked to
        /// start.
        ALREADY_GONE = "WATCH-2",
    }
    /// The `WIRE` codes.
    wire {
        /// A capability was named that no service in this stack provides.
        NO_SUCH_FILLER = "WIRE-1",
        /// The service named cannot do the thing it was asked to fill.
        CANNOT_FILL = "WIRE-2",
        /// Nothing in this stack asks for the capability, so a choice would change nothing.
        NOTHING_ASKS = "WIRE-3",
        /// The setting recording the choice could not be written.
        CHOICE_UNWRITABLE = "WIRE-4",
    }
    /// The `WIRING` codes.
    wiring {
        /// Raised when a download client no longer files where lemonfiber wired it.
        DRIFTED = "WIRING-1",
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{leaves, Leaves, EVERY};

    #[test]
    fn no_two_problems_answer_to_the_same_code() {
        let unique: BTreeSet<&str> = EVERY.iter().map(|code| code.as_str()).collect();
        assert_eq!(unique.len(), EVERY.len());
    }

    #[test]
    fn a_code_leaves_with_a_failure_unless_it_says_otherwise() {
        assert_eq!(leaves(super::life::NEVER_SETTLED), Leaves::NeverSettled);
        assert_eq!(leaves(super::docker::ENGINE_UNREACHABLE), Leaves::Preflight);
        assert_eq!(leaves(super::stack::STACK_INVALID), Leaves::Validation);
        assert_eq!(leaves(super::vpn::LEAKING), Leaves::Failure);
    }
}
