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
pub const ONLY_AT_HOME: &str =
    "All of this works on your home network and nowhere else. Away from the house, none of \
     these apps will find the server — that is how it is meant to be for now, not a fault.";

/// What lemonfiber will not do, said where somebody might expect otherwise.
pub const NOTHING_IS_INSTALLED: &str =
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
mod tests {
    use super::{
        guidance, Straining, Support, A_LIGHTER_PRESET, DEVICES, NOTHING_IS_INSTALLED,
        ONLY_AT_HOME, PLAYBACK_WILL_STRUGGLE, TROUBLE,
    };
    use crate::platform::Environment;
    use crate::quality::Preset;
    use crate::transcoding::{warn_before_confirming, Playback};
    use crate::wizard::Library;

    /// Every cause says how to tell it from the others under its symptom.
    ///
    /// The requirements ask that the causes be told apart, not listed. A cause with
    /// no way to tell it from its neighbour is a guess offered as an answer, and a
    /// reader given three of those is no better off than with none.
    #[test]
    fn every_cause_says_how_to_tell_it_from_the_others() {
        let silent: Vec<&str> = TROUBLE
            .iter()
            .flat_map(|one| one.causes)
            .filter(|cause| cause.tell.split_whitespace().count() < 4)
            .map(|cause| cause.because)
            .collect();

        assert!(
            silent.is_empty(),
            "these give no way to tell them apart: {silent:?}"
        );
        assert!(
            !TROUBLE.is_empty(),
            "no symptom is answered, so this checked nothing"
        );
    }

    /// Every cause says what is wrong and what to do about it.
    #[test]
    fn every_cause_carries_a_fix() {
        let thin: Vec<&str> = TROUBLE
            .iter()
            .flat_map(|one| one.causes)
            .filter(|cause| cause.fix.is_empty() || cause.because.is_empty())
            .map(|cause| cause.because)
            .collect();

        assert!(thin.is_empty(), "these name no fix: {thin:?}");
    }

    /// A symptom with one cause needs no telling apart, and one with several does.
    ///
    /// Both shapes are here on purpose: a guest network has one cause and an app that
    /// cannot find the server has three. What would be wrong is a symptom with none.
    #[test]
    fn every_symptom_offers_at_least_one_cause() {
        let empty: Vec<&str> = TROUBLE
            .iter()
            .filter(|one| one.causes.is_empty())
            .map(|one| one.symptom)
            .collect();

        assert!(empty.is_empty(), "these answer nothing: {empty:?}");
        assert!(
            TROUBLE.iter().any(|one| one.causes.len() > 1),
            "no symptom has causes to tell apart, so the rule above is checking nothing"
        );
    }

    /// A browser is present, and it is the entry that needs nothing installed.
    #[test]
    fn a_browser_is_offered_and_needs_nothing_installed() {
        let always: Vec<&super::Device> = DEVICES
            .iter()
            .filter(|one| one.support == Support::Fallback)
            .collect();

        assert_eq!(
            always.len(),
            1,
            "one entry always works, and it is the browser"
        );
        let asks_for_an_app: Vec<&str> = always
            .iter()
            .filter(|one| !one.client.contains("no app"))
            .map(|one| one.device)
            .collect();
        assert!(asks_for_an_app.is_empty(), "{asks_for_an_app:?}");
        let not_a_browser: Vec<&str> = always
            .iter()
            .filter(|one| !one.device.to_lowercase().contains("browser"))
            .map(|one| one.device)
            .collect();
        assert!(not_a_browser.is_empty(), "{not_a_browser:?}");
    }

    /// Every device marked [`Support::Poor`] carries an alternative, and at least
    /// one device is so marked — without the second half this passes on an empty
    /// filter.
    #[test]
    fn a_device_that_is_poorly_served_says_what_to_do_instead() {
        let poor: Vec<&str> = DEVICES
            .iter()
            .filter(|one| one.support.wants_an_alternative())
            .filter(|one| one.instead.is_none())
            .map(|one| one.device)
            .collect();

        assert!(
            poor.is_empty(),
            "these are named as poorly served and offer nothing else to try: {poor:?}"
        );
        assert!(
            DEVICES.iter().any(|one| one.support.wants_an_alternative()),
            "no device is named as poorly served, so the rule above checked nothing"
        );
    }

    /// Every device names something to use.
    #[test]
    fn every_device_says_what_to_use_on_it() {
        let silent: Vec<&str> = DEVICES
            .iter()
            .filter(|one| one.client.is_empty())
            .map(|one| one.device)
            .collect();

        assert!(silent.is_empty(), "these name no client: {silent:?}");
        assert!(
            DEVICES.len() > 5,
            "the table is too short to be a landscape"
        );
    }

    /// The caution names the transcode as what playback trouble is likely to be.
    ///
    /// Naming the preset alone would leave a household to work out why a preset has
    /// anything to do with a video that stops, which is the step nobody takes.
    #[test]
    fn a_preset_this_machine_can_only_transcode_in_software_names_the_transcode() {
        let strained = warn_before_confirming(
            Preset::Maximum,
            Playback::of(Environment::MacOs, Library::JellyfinDocker),
        );
        assert!(strained.is_some(), "the fixture must warrant a caution");

        assert_eq!(
            guidance(strained).straining,
            Some(Straining {
                preset: Preset::Maximum.label(),
                caution: PLAYBACK_WILL_STRUGGLE,
                instead: A_LIGHTER_PRESET,
            }),
            "the preset the operator chose is what the caution is about"
        );
        assert!(
            PLAYBACK_WILL_STRUGGLE.contains("transcode"),
            "the cause is not named: {PLAYBACK_WILL_STRUGGLE}"
        );
        assert!(
            PLAYBACK_WILL_STRUGGLE.contains("likely cause"),
            "the transcode is not named as the likely cause: {PLAYBACK_WILL_STRUGGLE}"
        );
        assert!(
            A_LIGHTER_PRESET.contains("lighter preset"),
            "nothing is offered that would stop it: {A_LIGHTER_PRESET}"
        );
    }

    /// Where nothing is strained the guidance gains no sentence.
    ///
    /// A caution shown to everybody says nothing about anybody's machine. The three
    /// ways not to warrant one are all here — a preset that provokes no transcode, a
    /// host that transcodes in hardware, and no media server at all — because each
    /// reaches this through a different arm of the decision.
    #[test]
    fn guidance_that_warrants_no_caution_does_not_gain_one() {
        let software_only = Playback::of(Environment::MacOs, Library::JellyfinDocker);
        let mut asked = 0_usize;
        for (preset, playback) in [
            (Preset::Balanced, software_only),
            (
                Preset::Maximum,
                Playback::of(Environment::LinuxNative, Library::JellyfinDocker),
            ),
            (
                Preset::Maximum,
                Playback::of(Environment::MacOs, Library::None),
            ),
        ] {
            asked += 1;
            let all = guidance(warn_before_confirming(preset, playback));
            assert!(
                all.straining.is_none(),
                "{preset:?} on {playback:?} gained a caution it does not warrant"
            );
            assert_eq!(
                all.devices.len(),
                DEVICES.len(),
                "and still answers in full"
            );
        }
        assert_eq!(asked, 3, "each way of warranting nothing is exercised");
    }

    /// Both statements true of every device are present.
    #[test]
    fn the_limits_that_hold_for_everything_are_stated() {
        let all = guidance(None);

        assert_eq!(all.devices.len(), DEVICES.len());
        assert!(
            all.only_at_home.contains("home network"),
            "the household is told where this works: {ONLY_AT_HOME}"
        );
        assert!(
            all.nothing_is_installed.contains("does not install"),
            "and what lemonfiber will not do: {NOTHING_IS_INSTALLED}"
        );
    }
}
