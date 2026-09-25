//! Which app to use on which device, and where the answer is to use something else.
//!
//! One entry per device category, each carrying how well served it is, what to use,
//! what is worth knowing before starting, and where support is poor what to do
//! instead. The table is the same on every machine: nothing here is read from disk
//! or asked of the engine.
//!
//! One thing above the table is not the same everywhere. Where the quality preset
//! in force asks for transcoding this platform cannot do in hardware, playback
//! struggles whatever app is installed — so [`Straining`] says so before any device
//! is chosen, and names the transcode as the likely cause of trouble that otherwise
//! reads as a bad app on a bad television. The fact is [`crate::transcoding`]'s and
//! is handed in; this module still reads nothing.
//!
//! A device marked [`Support::Poor`] carries an alternative, and a test refuses one
//! that does not. The browser is present as [`Support::Fallback`] — no installation,
//! any screen — so no device category is without an answer.
//!
//! Beside it, [`TROUBLE`] keys what to do when it does not work by the symptom
//! somebody reports rather than by the cause, because the cause is the thing they
//! cannot yet say. Where a symptom has more than one cause, each carries how to tell
//! it from the others: a cause offered without that is a guess presented as an
//! answer, and the requirements here ask that the causes be told apart.
//!
//! Nothing is installed from here. A link or a store page is named where one exists;
//! the rest is instructions.

use serde::Serialize;

use crate::transcoding::Warning;

/// How well a device is served.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Support {
    /// An official app that works. Most people, most of the time.
    Good,
    /// It works, with something worth knowing before starting.
    Workable,
    /// Poorly served. Said plainly, with somewhere else to go.
    Poor,
    /// No installation, works anywhere, and is never unavailable.
    Fallback,
}

impl Support {
    /// Whether somebody should be told to try something else first.
    #[must_use]
    pub const fn wants_an_alternative(self) -> bool {
        matches!(self, Self::Poor)
    }
}

/// A kind of device somebody in the house might watch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Device {
    /// What somebody would call the device they are holding.
    pub device: &'static str,
    /// How well served it is.
    pub support: Support,
    /// What to use on it.
    pub client: &'static str,
    /// What is worth knowing before starting, where anything is.
    pub caution: Option<&'static str>,
    /// What to do instead where this is a bad device to be stuck with.
    pub instead: Option<&'static str>,
}

/// Every device this product has something to say about.
///
/// Ordered by how common the device is, not alphabetically. Surfaces render this
/// order as given.
pub const DEVICES: &[Device] = &[
    Device {
        device: "Android phone or tablet",
        support: Support::Good,
        client: "the official Jellyfin app, from Google Play",
        caution: None,
        instead: None,
    },
    Device {
        device: "iPhone or iPad",
        support: Support::Good,
        client: "the official Jellyfin app, from the App Store",
        caution: None,
        instead: None,
    },
    Device {
        device: "Android TV or Fire TV",
        support: Support::Good,
        client: "the official Jellyfin app, from the device's own store",
        caution: None,
        instead: None,
    },
    Device {
        device: "Apple TV",
        support: Support::Workable,
        client: "the official Jellyfin app",
        caution: Some(
            "Other apps are widely used here and some people prefer them. The official one \
             is the answer if you do not already have an opinion.",
        ),
        instead: None,
    },
    Device {
        device: "A web browser",
        support: Support::Fallback,
        client: "no app at all — open the address",
        caution: Some(
            "Nothing to install, and it works on anything with a screen. This is the answer \
             whenever an app for the device is missing, broken, or more trouble than it is worth.",
        ),
        instead: None,
    },
    Device {
        device: "Smart TV (LG, Samsung)",
        support: Support::Poor,
        client: "an app exists for some models",
        caution: Some(
            "This is the weakest part of the landscape. Whether there is a working app depends \
             on the make and on how old the television is, and a set that worked last year can \
             stop after an update nobody asked for.",
        ),
        instead: Some(
            "A streaming stick plugged into the television is about the price of a takeaway and \
             turns this into the well-served case above. Casting from a phone works too, and \
             costs nothing to try first.",
        ),
    },
    Device {
        device: "Kodi",
        support: Support::Workable,
        client: "the Jellyfin plugin",
        caution: Some(
            "For somebody who already runs Kodi and wants to keep it. Not worth installing Kodi \
             in order to reach this stack.",
        ),
        instead: None,
    },
];

/// True of every device. Rendered once per report, never per device.
pub(crate) const ONLY_AT_HOME: &str =
    "All of this works on your home network and nowhere else. Away from the house, none of \
     these apps will find the server — that is how it is meant to be for now, not a fault.";

/// What lemonfiber will not do, said where somebody might expect otherwise.
pub(crate) const NOTHING_IS_INSTALLED: &str =
    "lemonfiber does not install anything on your device and cannot. What is here is where to \
     look and what to choose; the installing is yours.";

/// What a preset this machine can only transcode on the processor does to playback,
/// in the terms somebody watching would describe it.
///
/// Names the transcode, because the trouble it causes looks exactly like a bad app:
/// a household told only that a television is poorly served will change the app,
/// then the television, and arrive at the preset last if at all.
pub const PLAYBACK_WILL_STRUGGLE: &str =
    "This preset asks for more than most devices can play as it arrives, and no hardware \
     encoder is reachable from where the media server runs — so anything a device cannot \
     play directly is transcoded by the processor alone. Where a video stutters, takes a \
     long time to start, or stops partway through, that transcode is the likely cause \
     rather than the app, the device or the network.";

/// The two things that stop it, for an operator who would rather playback were
/// smooth than deep.
pub const A_LIGHTER_PRESET: &str =
    "A lighter preset leaves most devices nothing to transcode: `lemonfiber quality set \
     balanced` decides what arrives next and changes nothing already on disk. Running \
     Jellyfin natively, where it can reach the encoder, is the other answer.";

/// Why playback here is likely to struggle, whatever app the household installs.
///
/// Present only where the preset in force asks for transcoding this platform cannot
/// do in hardware. It belongs to the guidance rather than to any one device: the
/// preset and the platform decide it between them, and every device in the table
/// meets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Straining {
    /// The preset in force, under the name it was chosen by.
    pub preset: &'static str,
    /// What that preset asks of this machine, and what playback does where this
    /// machine cannot give it.
    pub caution: &'static str,
    /// What makes it stop.
    pub instead: &'static str,
}

impl Straining {
    /// The caution a [`Warning`] comes to, said for a household rather than for the
    /// operator about to confirm a preset.
    ///
    /// The same fact reaching a second surface: [`crate::transcoding`] decides
    /// whether there is one, and each surface says it in the words its reader needs.
    #[must_use]
    pub const fn of(warning: Warning) -> Self {
        Self {
            preset: warning.preset.label(),
            caution: PLAYBACK_WILL_STRUGGLE,
            instead: A_LIGHTER_PRESET,
        }
    }
}

/// One thing that could be behind a symptom, and how to tell it from the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Cause {
    /// What is wrong.
    pub because: &'static str,
    /// How to tell this cause from the others under the same symptom.
    pub tell: &'static str,
    /// What to do about it.
    pub fix: &'static str,
}

/// Something somebody reports, and what is likely behind it.
///
/// Keyed by the symptom rather than the cause: the person asking has the symptom,
/// and which cause it is is the thing they cannot yet say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Trouble {
    /// What somebody says is happening, in their words.
    pub symptom: &'static str,
    /// What is likely behind it, most likely first.
    pub causes: &'static [Cause],
}

/// Every symptom this product has an answer for.
pub const TROUBLE: &[Trouble] = &[
    Trouble {
        symptom: "The app cannot find the server",
        causes: &[
            Cause {
                because: "The address is wrong, or it has changed.",
                tell: "Every device fails the same way, including one that worked yesterday.",
                fix: "Check it against what `lemonfiber front-door` says now. That address is \
                      read from this machine at the moment of asking, so a machine that was \
                      renamed answers differently from the note somebody wrote down.",
            },
            Cause {
                because: "The device is on a different network from the server.",
                tell: "The device has working internet, and another device in the house can \
                       reach the server.",
                fix: "Put it back on the home Wi-Fi. A phone that fell back to mobile data is \
                      the common one, and it looks like a broken address rather than a network \
                      it is on.",
            },
            Cause {
                because: "The server is not running.",
                tell: "No device can reach it, and this is the one you can answer without \
                       leaving your chair.",
                fix: "`lemonfiber status` says whether it is, and `lemonfiber up` starts it.",
            },
        ],
    },
    Trouble {
        symptom: "The device is on the guest Wi-Fi",
        causes: &[Cause {
            because: "Guest networks keep devices from reaching each other, which is what \
                      they are for.",
            tell: "The device has working internet and the address is right, and nothing \
                   answers — which looks exactly like a wrong address.",
            fix: "Join the ordinary home network. There is nothing to change on the server: a \
                  guest network is doing its job.",
        }],
    },
    Trouble {
        symptom: "The library is empty after signing in",
        causes: &[
            Cause {
                because: "Nothing has been scanned yet.",
                tell: "Nobody sees anything, including you.",
                fix: "A new library, or one whose files moved, has nothing until it is \
                      scanned. Jellyfin scans on a schedule and can be told to now.",
            },
            Cause {
                because: "The account has been given access to no library.",
                tell: "Somebody else sees content and this person does not.",
                fix: "The account exists and can sign in — what it lacks is permission. Give \
                      it the libraries it should see in Jellyfin's user settings.",
            },
        ],
    },
    Trouble {
        symptom: "It worked and now the app cannot connect",
        causes: &[Cause {
            because: "The address changed and the app remembers the old one.",
            tell: "It stopped for everybody at once, and a browser opened at the current \
                   address still works.",
            fix: "Change the saved server in the app to what `lemonfiber front-door` says \
                  now. Where an app offers no way to edit it, remove the server and add it \
                  again.",
        }],
    },
];

/// The guidance in full, for a surface that shows all of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Guidance {
    /// Why playback here is likely to struggle before any app is chosen, or `None`
    /// where the preset in force asks for nothing this platform cannot serve.
    ///
    /// Absent far more often than present, and it must be: a caution shown to
    /// everybody says nothing about anybody's machine, and a reader who meets one
    /// every time stops reading it.
    pub straining: Option<Straining>,
    /// Every device, in the order somebody is likely to be holding one.
    pub devices: Vec<Device>,
    /// What to do when it does not work, keyed by the symptom.
    pub trouble: Vec<Trouble>,
    /// True of every device, said once.
    pub only_at_home: &'static str,
    /// What this will not do for them.
    pub nothing_is_installed: &'static str,
}

/// What to use, for every device this product has an answer for — carrying the
/// caution where playback here will struggle whatever is installed.
///
/// `strained` is the surface's answer to a question this module cannot ask: what the
/// preset in force is, and what the platform can transcode. Handed in rather than
/// read here, and optional, so guidance still answers on a machine with no stack set
/// up at all — where nothing can be read, there is nothing to caution about.
#[must_use]
pub fn guidance(strained: Option<Warning>) -> Guidance {
    Guidance {
        straining: strained.map(Straining::of),
        devices: DEVICES.to_vec(),
        trouble: TROUBLE.to_vec(),
        only_at_home: ONLY_AT_HOME,
        nothing_is_installed: NOTHING_IS_INSTALLED,
    }
}

#[cfg(test)]
mod tests;
