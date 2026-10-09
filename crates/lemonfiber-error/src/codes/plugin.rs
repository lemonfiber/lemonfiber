codes! {
    /// A check a plugin contributed did not hold.
    ///
    /// One code for all of them rather than one per plugin, because a code is a stable
    /// thing an operator searches for and a plugin's own name is not this build's to mint
    /// one from. Which check and which plugin is on the finding, where it can be read.
    CONTRIBUTED_FAILED = "PLUGIN-1" {
        severity: Error,
        status: 409,
        since: "0.16.0",
        meaning: "A check a plugin contributed to the doctor did not hold. The meaning is the \
            one the plugin declared with its first remedy; the message names the check, the \
            plugin, and each fault found.",
        remedy: "Follow the remedies the plugin declared, in order. Where it declared none, the \
            message says to ask the plugin's author.",
    }
    /// The source names no plugin this build can read.
    UNREADABLE = "PLUGIN-2" {
        severity: Error,
        status: 404,
        since: "0.16.0",
        meaning: "What was named is not a plugin lemonfiber can read. Nothing was installed or \
            written.",
        remedy: "Point at the plugin's directory, or at the `plugin.toml` inside it.",
    }
    /// The manifest is read and this build refuses what it declares.
    REFUSED = "PLUGIN-3" {
        severity: Error,
        status: 400,
        since: "0.16.0",
        meaning: "The plugin declares things lemonfiber will not install. The manifest is \
            refused whole — nothing was written — and the message lists every violation at once.",
        remedy: "Read what each one says, and take it up with whoever published the plugin.",
    }
    /// The record of what is installed cannot be read.
    UNRECORDED = "PLUGIN-4" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "The record of what is installed could not be read, so lemonfiber will not say \
            that nothing is installed. The message names the file.",
        remedy: "Restore the file from a backup, or move it aside if nothing is installed.",
    }
    /// The plugin is installed already.
    ALREADY = "PLUGIN-5" {
        severity: Error,
        status: 400,
        since: "0.16.0",
        meaning: "The plugin is already installed. Installing over it would be an update, and an \
            install is not one. Nothing was changed; the message names the version installed.",
        remedy: "Update it with `lemonfiber plugin update` on the new source, or remove it \
            first.",
    }
    /// There is no stack on this machine to put a plugin's container in.
    NOWHERE = "PLUGIN-6" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "There is nowhere to install the plugin to: no stack directory has been set up.",
        remedy: "Run `lemonfiber setup` first, then install the plugin.",
    }
    /// A directory or a document the install decided on would not land.
    UNWRITABLE = "PLUGIN-7" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "A file the install needed could not be written. The install stopped where it \
            was, and what it had written is on the change record.",
        remedy: "Check the permissions on the stack directory, then install it again. \
            `lemonfiber history` shows what was written.",
    }
    /// The wiring went down and the record of what is installed did not.
    UNRECORDABLE = "PLUGIN-8" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "The plugin held every proof and could not be recorded as installed, so the \
            install was put back.",
        remedy: "Check the permissions on the configuration directory, then install it again.",
    }
    /// The plugin's own service would not start, so nothing about it could be proved.
    UNPROVED = "PLUGIN-9" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "The plugin's service would not start, so nothing about it could be proved, and \
            the install was put back.",
        remedy: "Check that the stack is set up and the container engine is running, then \
            install it again.",
    }
    /// Nothing by that name is installed on this machine.
    NOTHING_TO_REMOVE = "PLUGIN-10" {
        severity: Error,
        status: 404,
        since: "0.16.0",
        meaning: "There is no installed plugin by that name, so nothing was removed. The message \
            lists what is installed.",
        remedy: "See what is on this machine with `lemonfiber plugin installed`, and remove it \
            by that id.",
    }
    /// Nothing by that id is installed, so there is no version to replace.
    NOTHING_TO_UPDATE = "PLUGIN-11" {
        severity: Error,
        status: 404,
        since: "0.16.0",
        meaning: "The plugin is not installed, so there is nothing to update. Nothing was \
            changed.",
        remedy: "Install it instead, with `lemonfiber plugin install` on the same source.",
    }
    /// The version installed would not come off, so nothing else was touched.
    STUCK = "PLUGIN-12" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "The installed version would not stop, so it was not updated. It is still \
            installed, and nothing else changed.",
        remedy: "Check the container engine is running, then try the update again.",
    }
    /// Raised when a plugin's service would answer on a label another plugin's already does.
    ANSWERED = "PLUGIN-13" {
        severity: Error,
        status: 400,
        since: "0.16.0",
        meaning: "The plugin's service would answer on a hostname another plugin already answers \
            on. The proxy will not start with two sites at one address, because that would bring \
            down every route it serves.",
        remedy: "Give the plugin's service another hostname in its manifest, or remove the other \
            plugin first.",
    }
    /// Raised when a plugin is installed from a source other than the one its name is
    /// already installed from.
    TWO_SOURCES = "PLUGIN-14" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The plugin is already installed from another source, and this is a second one \
            for the same name. Two sources for one name are two plugins as far as anybody can \
            tell, and which of them this machine runs is yours to choose. Nothing was written; \
            the message names both sources.",
        remedy: "To run the new source in its place, run `lemonfiber plugin update` on it. To \
            keep the one installed, nothing needs doing.",
    }
    /// Raised when a plugin is named from a git source and fetching from one is
    /// switched off.
    SOURCE_OFF = "PLUGIN-15" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "Fetching a plugin from a git source is switched off on this machine, so the \
            source was not asked. Nothing was fetched or installed.",
        remedy: "Install it from a directory instead, or allow it with `lemonfiber config set \
            LEMONFIBER_REACH_PLUGIN_SOURCE on`.",
    }
    /// Raised when a git source could not be reached or would not hand over a revision.
    UNFETCHED = "PLUGIN-16" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The git source could not be fetched: it did not answer, or would not hand over \
            the revision asked for. Nothing was installed, and the message carries what went \
            wrong.",
        remedy: "Check the address, and that this machine can reach it, then install it again.",
    }
    /// Raised when a git source holds no branch, tag or commit by the name given.
    NO_REVISION = "PLUGIN-17" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "The git source holds no branch, tag or commit by the name given. Nothing was \
            fetched or installed.",
        remedy: "Name a branch, a tag or a whole commit after the last `@`, or leave it off for \
            what the source serves by default.",
    }
    /// Raised when a plugin is installed by name and asking the catalogue is switched
    /// off.
    CATALOGUE_OFF = "PLUGIN-18" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The plugin was named for the catalogue to look up, and asking the catalogue is \
            switched off on this machine. Nothing was fetched or installed.",
        remedy: "Install it from a git source or a directory you name instead, or allow it with \
            `lemonfiber config set LEMONFIBER_REACH_CATALOGUE on`.",
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
    }
    /// Raised when the catalogue holds no plugin by the name given.
    NOT_CATALOGUED = "PLUGIN-22" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "The catalogue holds no plugin by that name. Nothing was fetched or installed.",
        remedy: "Check the name, or write `./<name>` to install the directory of that name.",
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
    }
    /// Raised when a plugin's service would be named, where lemonfiber keeps what a
    /// service holds, as another installed plugin's service already is.
    SPELLED_ALIKE = "PLUGIN-24" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The plugin's service would be named as another installed plugin's service \
            already is, once case and `-` or `_` are set aside. lemonfiber writes a plugin's \
            container under its service's name and keeps what the service holds in settings \
            named after it, so the two would be one container sharing one credential. Nothing \
            was written; the message names both services.",
        remedy: "Remove the other plugin first, or install a version of this one whose service \
            is named otherwise.",
    }
    /// Raised when an install, an update or a removal answers an offer that was read
    /// against a plugin, a stack or a record that has since moved.
    PLUGIN_OFFER_MOVED = "PLUGIN-25" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "That agreement was given for a different reading of the plugin: since it was \
            read, the plugin, the stack or the record of what is installed changed. Agreeing to \
            it now would be agreeing to something nobody saw, so nothing was changed; the \
            message names what moved.",
        remedy: "Read it again, and answer the name it prints.",
    }
    /// Raised when a value a recipe would carry to a destination was not approved as
    /// itself, or an approval names a pair the recipe does not carry.
    UNAPPROVED = "PLUGIN-26" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A value a recipe would send to another host was not approved as itself, or an \
            approval names a value and destination the reading does not list. Each value a \
            recipe carries elsewhere is agreed to on its own, apart from the plugin, so nothing \
            was changed.",
        remedy: "Approve each value the reading lists, by name, with `--approve`, and nothing \
            else.",
    }
    /// Raised when the source an update names holds a different plugin from the one
    /// it was asked to update.
    ANOTHER_PLUGIN = "PLUGIN-27" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The source the update names holds a different plugin from the one it was asked \
            to update. An update puts a new version of the installed plugin in its place, so \
            nothing was changed.",
        remedy: "Name a source that holds the plugin being updated, or install the other one on \
            its own.",
    }
    /// Raised when a plugin's service would take a name, a port or a label something
    /// already on this machine holds: a service of the stack or of the operator's
    /// overlay, another plugin's port, or a site in the proxy's live configuration.
    OCCUPIED = "PLUGIN-28" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The plugin's service would take a name, a port or a label something on this \
            machine already holds: a service of the stack or of your own overlay, another \
            plugin's port, or a site the proxy already serves. Nothing was written; the message \
            names each clash.",
        remedy: "Rename or move what holds them, or install a version of the plugin that \
            declares others.",
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
        remedy: "Check `catalogue.json` in the configuration directory, or install it from its \
            git source.",
    }
    /// Raised when a git source is named over a transport other than https, before
    /// anything is asked of it.
    SCHEME_REFUSED = "PLUGIN-31" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The git source is named over a transport other than https, so nothing was \
            asked of it and nothing was installed. A source fetched over anything else can be \
            read or rewritten on its way here.",
        remedy: "Name the same repository by its https address.",
    }
    /// Raised when a git source's host is, or stands for, an address on this machine
    /// or on a network of its own: loopback, private, link-local or unspecified.
    ADDRESS_REFUSED = "PLUGIN-32" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The git source's host is, or stands for, an address on this machine or on a \
            network of its own — loopback, private, link-local or unspecified. A plugin is \
            fetched only from somewhere every machine could reach, so nothing was fetched and \
            nothing was installed.",
        remedy: "Install it from a directory on this machine, or from a public https address.",
    }
    /// Raised when a recipe substitutes a value into a header's name, which is a fixed
    /// identifier of the protocol and written out; the manifest's every other fault is
    /// listed beside it.
    HEADER_NAMED = "PLUGIN-33" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The plugin's recipe substitutes a value into a header's name, which is a fixed \
            identifier of the protocol and is written out. The manifest is refused whole — \
            nothing was installed or written — and the message lists every other violation \
            beside it.",
        remedy: "Read what each one says, and take it up with whoever published the plugin.",
    }
    /// Raised when a recipe of the act asks the operator for a value that was not
    /// given, or a value was given that no recipe of the act asks for.
    INPUT_UNMATCHED = "PLUGIN-34" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A recipe of the plugin asks the operator for a value that was not given, or a \
            value was given that no recipe of it asks for. Nothing was installed and nothing was \
            written, and the message names each input.",
        remedy: "Give each input the plugin's recipes ask for, by name, and nothing else.",
    }
    /// Raised when a recipe's call was not sent because its host stands for an address
    /// not out on the internet; the install or update was put back.
    CALL_REFUSED = "PLUGIN-35" {
        severity: Error,
        status: 500,
        since: "0.18.0",
        meaning: "A recipe's call names a host that is, or stands for, an address not out on the \
            internet, so it was not sent. The install or update was put back, and the message \
            names the steps that had run before it.",
        remedy: "Read what the message says, and take it up with whoever published the plugin.",
    }
    /// Raised when a recipe's step failed any other way — nothing answered, the answer
    /// was not the one it expects, a capture found nothing, or the answer was larger
    /// than a recipe reads; the install or update was put back.
    STEP_FAILED = "PLUGIN-36" {
        severity: Error,
        status: 500,
        since: "0.18.0",
        meaning: "A step of a recipe failed: nothing answered, the answer was not the one the \
            recipe expects, a capture found nothing, or the answer was larger than any a recipe \
            reads. The install or update was put back, and the message names the steps that had \
            run before it.",
        remedy: "Check the service the recipe calls is running, then try again. If it keeps \
            failing, take it up with whoever published the plugin.",
    }
    /// Raised when a recipe's call path is not a plain absolute path; the manifest's
    /// every other fault is listed beside it.
    PATH_NOT_PLAIN = "PLUGIN-37" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A recipe's call path is not a plain absolute path. The manifest is refused \
            whole — nothing was installed or written — and the message lists every other \
            violation beside it.",
        remedy: "Read what each one says, and take it up with whoever published the plugin.",
    }
    /// Raised when a recipe's call was not sent because a value it carries may not go
    /// where it was going: not where its pairs say, not back to the service a
    /// credential belongs to, or outside without its approval; the install or update
    /// was put back.
    VALUE_WITHHELD = "PLUGIN-38" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A recipe's call would have carried a value somewhere it may not go: not where \
            the plugin's pairs say, not back to the service a credential belongs to, or outside \
            without the operator's approval. It was not sent, and the install or update was put \
            back.",
        remedy: "Read what the message says, and take it up with whoever published the plugin.",
    }
    /// Nothing by that id is installed, so there is nothing to prove again.
    NOTHING_TO_PROVE = "PLUGIN-39" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "The plugin is not installed, so nothing was asked and nothing was cleared. The \
            message lists what is installed.",
        remedy: "See what is on this machine with `lemonfiber plugin installed`, and prove it by \
            that id.",
    }
}
