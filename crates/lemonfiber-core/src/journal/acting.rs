//! What undoing a change does.

use serde::Serialize;

/// A single reversal, for the surface to carry out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Undo {
    /// The service or file to reverse it against.
    pub target: String,
    /// What reversing it does.
    pub action: Action,
}

/// What an undo does.
///
/// Tagged by what it does rather than by the field it sits in, so a reader parsing
/// one branches on a word rather than on which keys are present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "does", rename_all = "kebab-case")]
pub enum Action {
    /// Remove the resource that was created.
    Remove {
        /// The kind of resource.
        resource: String,
        /// The identifier to remove.
        id: String,
    },
    /// Restore a value, or remove it where there was none before (`None`).
    Restore {
        /// The setting to restore.
        key: String,
        /// What to restore it to, or `None` to remove it.
        value: Option<String>,
        /// What lemonfiber put there, which has to still be there for putting the
        /// old value back to be putting anything back.
        ///
        /// Carried so that a reversal can ask whether it is undoing its own work.
        /// Without it a reversal knows only what it would like the setting to say,
        /// and a setting the operator has since chosen for themselves reads exactly
        /// like one nobody has touched.
        wrote: String,
    },
    /// Remove a path that was created.
    Delete {
        /// The path to remove.
        path: String,
    },
    /// Take a region lemonfiber wrote back out of the file it was written into.
    ///
    /// Only where the region is still what was written. One that was edited since, or
    /// whose markers were, is somebody else's work now, and is left exactly as it is.
    Withdraw {
        /// The file the region is in.
        path: String,
        /// The same file beneath the stack directory, as the record of what lemonfiber
        /// materialised names it.
        key: String,
        /// Whose region it is, as its markers name it.
        owner: String,
        /// The checksum of what was written between the markers, which has to still be
        /// what is there for taking it out to be taking out lemonfiber's own work.
        written: u32,
    },
    /// Write a file back to what it held before lemonfiber wrote over it.
    ///
    /// Only where it still holds what was written. One written since is somebody
    /// else's work now, and is left exactly as it is.
    Rewind {
        /// The file to write back.
        path: String,
        /// What to write back into it.
        previous: String,
        /// The checksum of what lemonfiber wrote, which has to still be what is there
        /// for writing the old text back to be undoing lemonfiber's own work.
        written: u32,
    },
    /// Move a service's state back to where it was.
    ///
    /// Only where nothing has taken its old place, and only from where it was moved to.
    MoveBack {
        /// Where it goes back to.
        from: String,
        /// Where it is now.
        to: String,
    },
    /// Move a value in the environment file back to the key it was under.
    ///
    /// Only where the key it is under now holds a value and the one it goes back to holds
    /// none.
    RenameBack {
        /// The key it goes back to.
        from: String,
        /// The key it is under now.
        to: String,
    },
    /// Keep the copy of a volume where it is: the volume was never changed, so nothing
    /// is put back.
    KeepCopy {
        /// The volume it was copied from.
        volume: String,
        /// Where the copy is.
        to: String,
    },
    /// Pin a service back to the version it was standing on.
    ///
    /// The one reversal nothing in this product carries out. Which version runs is
    /// decided by the materialised stack and by what Compose was told to start, and
    /// a reversal of settings and files reaches neither — so this is worked out,
    /// reported, and left for the operator rather than attempted.
    Repin {
        /// The version to put back.
        previous: String,
        /// The version this run moved it to, which has to still be the one running
        /// for putting the old one back to be putting anything back.
        current: String,
    },
    /// Put one field of a service's own resource back to what it held.
    ///
    /// The only reversal that needs the service itself: the value lives inside it,
    /// and nothing on the host can write it. A reversal that cannot reach the
    /// service says so rather than reporting the field restored.
    Reconfigure {
        /// The kind of resource.
        resource: String,
        /// The identifier to change.
        id: String,
        /// The field to put back.
        field: String,
        /// What to put back, or `None` where it held nothing.
        value: Option<String>,
    },
    /// Revoke the key a mint made.
    Revoke {
        /// The key's name.
        name: String,
    },
    /// Make a revoked key good again.
    ///
    /// Worked out so the record of a revoke has a reversal to name, and never carried
    /// out: a key is revoked because something should stop holding it, and reinstating
    /// it would hand back what the revoke took away. Whatever still needs a key is
    /// minted a new one.
    Reinstate {
        /// The key's name.
        name: String,
    },
    /// Revoke the device key a registration made.
    RevokeDevice {
        /// The device key's name.
        name: String,
    },
    /// Make a revoked device key good again.
    ///
    /// Worked out so the record of a revoke has a reversal to name, and never carried
    /// out, for the reason [`Action::Reinstate`] never is.
    ReinstateDevice {
        /// The device key's name.
        name: String,
    },
    /// Put a setting in the settings store back to what it held.
    Resettle {
        /// The setting's name.
        name: String,
        /// What to put back, or `None` to clear it.
        value: Option<String>,
        /// What lemonfiber recorded, which has to still be there.
        recorded: Option<String>,
        /// The environment file's setting it goes back to, where it was moved out of one.
        filed: Option<String>,
    },
    /// Give a grant back to a plugin, or end one it was given.
    Regrant {
        /// The plugin the grant is of.
        plugin: String,
        /// The grant, by the setting the credential is held under.
        grant: crate::plugin::Grant,
        /// Whether to give it, rather than end it.
        give: bool,
    },
    /// Put a secret in the secret store back to the value it held.
    Rehold {
        /// The secret's name.
        name: String,
        /// The fingerprint of the value to put back, or `None` to forget it.
        earlier: Option<String>,
        /// The fingerprint of what lemonfiber kept, which has to still be there.
        kept: Option<String>,
        /// The environment file's setting it goes back to, where it was moved out of one.
        filed: Option<String>,
    },
    /// Leave the media server's internal certificate as it is.
    ///
    /// Worked out so the record of a replacement has a reversal to name, and never carried
    /// out: the certificate is put back only by replacing it again.
    Unreplaced {
        /// The media server's service.
        service: String,
    },
    /// Start a service again, or stop it again.
    Run {
        /// The service.
        service: String,
        /// Whether it is left running.
        running: bool,
    },
    /// Leave the operator's overlay as it is.
    ///
    /// Worked out so the record of a change to it has a reversal to name, and never
    /// carried out: lemonfiber does not write the operator's own file.
    Leave {
        /// The overlay.
        path: String,
    },
}
